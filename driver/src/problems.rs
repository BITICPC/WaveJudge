//! This module manages problem metadata.
//!

use std::path::PathBuf;
use std::str::FromStr;
use std::string::ToString;
use std::sync::Arc;

use serde::{Serialize, Deserialize};

use crate::common::{ObjectId, LanguageTriple};
use crate::db::SqliteConnection;
use crate::restful::RestfulClient;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    links {
        DbError(crate::db::Error, crate::db::ErrorKind);
        RestfulError(crate::restful::Error, crate::restful::ErrorKind);
    }

    foreign_links {
        SqliteError(::sqlite::Error);
    }
}

/// Represent the kind of judge mode.
#[repr(C)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum JudgeModeKind {
    /// Standard judge mode.
    Standard,

    /// Special judge.
    SpecialJudge,

    /// Interactive judge mode.
    Interactive,
}

/// Provide metadata about a problem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemMetadata {
    /// The ID of the problem.
    #[serde(rename = "id")]
    pub id: ObjectId,

    /// The judge mode of the problem.
    #[serde(rename = "judgeMode")]
    pub judge_mode: JudgeModeKind,

    /// The time limit of the problem, in milliseconds.
    #[serde(rename = "timeLimit")]
    pub time_limit: u64,

    /// The memory limit of the problem, in megabytes.
    #[serde(rename = "memoryLimit")]
    pub memory_limit: u64,

    /// The source code of the jury, if the `judge_mode` is `JudgeMode::SpecialJudge` or
    /// `JudgeMode::Interactive`.
    #[serde(rename = "jurySource")]
    #[serde(default)]
    pub jury_src: Option<String>,

    /// The language of the jury program.
    #[serde(rename = "juryLanguage")]
    #[serde(default)]
    pub jury_lang: Option<LanguageTriple>,

    /// Path to the jury's executable, if the `judge_mode` is `JudgeMode::SpecialJudge` or
    /// `JudgeMode::Interactive`.
    #[serde(skip_deserializing)]
    pub jury_exec_path: Option<PathBuf>,

    /// The ID of the test archive.
    #[serde(rename = "archiveId")]
    pub archive_id: ObjectId,

    /// Timestamp of the last update time of this metadata.
    #[serde(rename = "timestamp")]
    pub timestamp: u64,
}

impl ProblemMetadata {
    /// Deserialize a new `ProblemMetadata` value from the given sqlite database row.
    fn from_db_row(row: &[sqlite::Value]) -> Option<Self> {
        let id = match row[0].as_string() {
            Some(s) => match ObjectId::from_str(&s) {
                Ok(i) => i,
                Err(..) => return None
            },
            None => return None
        };

        let judge_mode = match row[1].as_integer() {
            Some(0) => JudgeModeKind::Standard,
            Some(1) => JudgeModeKind::SpecialJudge,
            Some(2) => JudgeModeKind::Interactive,
            _ => return None
        };

        let time_limit = match row[2].as_integer() {
            Some(v) => crate::utils::bitcast::<i64, u64>(v),
            None => return None
        };

        let memory_limit = match row[3].as_integer() {
            Some(v) => crate::utils::bitcast::<i64, u64>(v),
            None => return None
        };

        let jury_src = row[4].as_string().map(String::from);

        let jury_lang_id = row[5].as_string().map(String::from);
        let jury_lang_dialect = row[6].as_string().map(String::from);
        let jury_lang_version = row[7].as_string().map(String::from);
        let jury_lang = match (jury_lang_id, jury_lang_dialect, jury_lang_version) {
            (Some(id), Some(dialect), Some(version)) =>
                Some(LanguageTriple::new(id, dialect, version)),
            _ => None
        };

        let jury_exec_path = match &row[8] {
            sqlite::Value::Null => None,
            sqlite::Value::String(s) => Some(PathBuf::from(s)),
            _ => return None
        };

        let archive_id = match row[9].as_string() {
            Some(s) => match ObjectId::from_str(&s) {
                Ok(i) => i,
                Err(..) => return None
            },
            None => return None
        };

        let timestamp = match row[10].as_integer() {
            Some(v) => crate::utils::bitcast::<i64, u64>(v),
            None => return None
        };

        Some(ProblemMetadata {
            id,
            judge_mode,
            time_limit,
            memory_limit,
            jury_src,
            jury_lang,
            jury_exec_path,
            archive_id,
            timestamp
        })
    }

    /// Determine whether the problem has a jury program, i.e. whether the judge mode of this
    /// problem is either `JudgeMode::SpecialJudge` or `JudgeMode::Interactive`.
    pub fn has_jury(&self) -> bool {
        match self.judge_mode {
            JudgeModeKind::SpecialJudge | JudgeModeKind::Interactive => true,
            _ => false
        }
    }

    /// Determine whether the jury program has been compiled.
    pub fn jury_compiled(&self) -> bool {
        self.jury_exec_path.is_some()
    }

    /// Save the metadata into the sqlite database through the given database connection.
    fn save(&self, conn: &SqliteConnection) -> Result<()> {
        let id = format!("'{}'", self.id.to_string());
        let judge_mode = self.judge_mode as i32;
        let time_limit = self.time_limit;
        let memory_limit = self.memory_limit;
        let jury_src = match &self.jury_src {
            Some(s) => format!("'{}'", s),
            None => String::from("NULL")
        };
        let (jury_lang_id, jury_lang_dialect, jury_lang_version) = match &self.jury_lang {
            Some(lang) => (
                format!("'{}'", lang.identifier),
                format!("'{}'", lang.dialect),
                format!("'{}'", lang.version)
            ),
            None => (String::from("NULL"), String::from("NULL"), String::from("NULL"))
        };
        let jury_exec_path = match &self.jury_exec_path {
            Some(p) => format!("'{}'", p.display()),
            None => String::from("NULL")
        };
        let archive_id = format!("'{}'", self.archive_id.to_string());
        let timestamp = self.timestamp;

        let stmt = format!(r#"
            INSERT OR REPLACE INTO problems(
                id,
                judge_mode,
                time_limit,
                memory_limit,
                jury_src,
                jury_lang_id,
                jury_lang_dialect,
                jury_lang_version,
                jury_exec_path,
                archive_id,
                timestamp
            ) VALUES (
                {}, /* id */
                {}, /* judge_mode */
                {}, /* time_limit */
                {}, /* memory_limit */
                {}, /* jury_src */
                {}, /* jury_lang_id */
                {}, /* jury_lang_dialect */
                {}, /* jury_lang_version */
                {}, /* jury_exec_path */
                {}, /* archive_id */
                {}  /* timestamp */
            )
        "#, id, judge_mode, time_limit, memory_limit, jury_src,
            jury_lang_id, jury_lang_dialect, jury_lang_version, jury_exec_path,
            archive_id, timestamp);

        conn.execute(|sqlite| {
            sqlite.execute(stmt)
        })?;

        Ok(())
    }
}

impl crate::restful::ProblemInfo for ProblemMetadata { }

/// Provide access to the problem metadata store.
pub struct ProblemStore {
    /// Connection to the sqlite database containing problem metadata.
    db: Arc<SqliteConnection>,

    /// RESTful client connected to the judge board server.
    rest: Arc<RestfulClient>,
}

impl ProblemStore {
    /// Create a new `ProblemStore` instance.
    pub fn new(db: Arc<SqliteConnection>, rest: Arc<RestfulClient>) -> Self {
        ProblemStore { db, rest }
    }

    pub fn init_db(&self) -> Result<()> {
        log::info!("Creating table `problems` on sqlite database");
        self.db.execute(|conn| {
            conn.execute(r#"
                CREATE TABLE problems(
                    id                  TEXT PRIMARY KEY,
                    judge_mode          INTEGER,
                    time_limit          INTEGER,
                    memory_limit        INTEGER,
                    jury_src            TEXT,
                    jury_lang_id        TEXT,
                    jury_lang_dialect   TEXT,
                    jury_lang_version   TEXT,
                    jury_exec_path      TEXT,
                    archive_id          TEXT,
                    timestamp           INTEGER
                );
            "#)
        })?;
        log::info!("Successfully created table `problems`");

        Ok(())
    }

    /// Get the problem metadata of the specified problem.
    pub fn get_problem_metadata(&self, id: ObjectId) -> Result<ProblemMetadata> {
        let cached_metadata = self.db.execute(|conn| -> Result<Option<ProblemMetadata>> {
            let mut cursor = conn
                .prepare("SELECT * FROM problems WHERE id = ?")?
                .cursor();
            cursor.bind(&[sqlite::Value::String(id.to_string())])?;

            if let Some(row) = cursor.next()? {
                Ok(ProblemMetadata::from_db_row(row))
            } else {
                Ok(None)
            }
        })?;
        if cached_metadata.is_some() {
            return Ok(cached_metadata.unwrap());
        }

        let metadata: ProblemMetadata = self.rest.get_problem_info(id)?;
        metadata.save(self.db.as_ref())?;

        Ok(metadata)
    }

    /// Get the last update timestamp of the specified problem's metadata.
    pub fn get_problem_timestamp(&self, id: ObjectId) -> Result<Option<u64>> {
        let id_str = id.to_string();
        self.db.execute(move |conn| {
            let mut cursor = conn.prepare("SELECT timestamp FROM problems WHERE id = ?")?
                                .cursor();
            cursor.bind(&[sqlite::Value::String(id_str)])?;
            match cursor.next()? {
                Some(v) => match v[0].as_integer() {
                    Some(i) => Ok(Some(crate::utils::bitcast::<i64, u64>(i))),
                    None => Ok(None)
                },
                None => Ok(None)
            }
        })
    }

    /// Update the problem metadata for the specified problem if the timestamp of the cached problem
    /// metadata is before the given one. This function returns the updated problem metadata.
    pub fn update_problem_metadata(&self, id: ObjectId, timestamp: u64) -> Result<ProblemMetadata> {
        let cached_timestamp = self.get_problem_timestamp(id)?;
        if cached_timestamp.is_some() && cached_timestamp.unwrap() >= timestamp {
            self.get_problem_metadata(id)
        } else {
            self.update_problem_metadata_force(id)
        }
    }

    /// Update the problem metadata for the specified problem. This function returns the updated problem
    /// metadata.
    pub fn update_problem_metadata_force(&self, id: ObjectId) -> Result<ProblemMetadata> {
        let metadata: ProblemMetadata = self.rest.get_problem_info(id)?;
        metadata.save(self.db.as_ref())?;
        Ok(metadata)
    }
}
