//! This module manages problem metadata.
//!

use std::path::PathBuf;
use std::str::FromStr;
use std::string::ToString;
use std::sync::Arc;

use crate::forkserver::{ForkServerClient, ForkServerClientExt};
use crate::restful::RestfulClient;
use crate::restful::entities::{ObjectId, LanguageTriple, ProblemInfo, JudgeMode};
use crate::sync::KeyLock;

use super::db::SqliteConnection;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    links {
        DbError(super::db::Error, super::db::ErrorKind);
        RestfulError(crate::restful::Error, crate::restful::ErrorKind);
        ForkServerError(crate::forkserver::Error, crate::forkserver::ErrorKind);
    }

    foreign_links {
        IoError(::std::io::Error);
        SqliteError(::sqlite::Error);
    }
}

/// Provide metadata about a problem.
#[derive(Debug, Clone)]
pub struct ProblemMetadata {
    /// The ID of the problem.
    pub id: ObjectId,

    /// The judge mode of the problem.
    pub judge_mode: JudgeMode,

    /// The time limit of the problem, in milliseconds.
    pub time_limit: u64,

    /// The memory limit of the problem, in megabytes.
    pub memory_limit: u64,

    /// The source code of the jury, if the `judge_mode` is `JudgeMode::SpecialJudge` or
    /// `JudgeMode::Interactive`.
    pub jury_src: Option<String>,

    /// The language of the jury program.
    pub jury_lang: Option<LanguageTriple>,

    /// Path to the jury's executable, if the `judge_mode` is `JudgeMode::SpecialJudge` or
    /// `JudgeMode::Interactive`.
    pub jury_exec_path: Option<PathBuf>,

    /// The ID of the test archive.
    pub archive_id: ObjectId,

    /// Timestamp of the last update time of this metadata.
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
            Some(0) => JudgeMode::Standard,
            Some(1) => JudgeMode::SpecialJudge,
            Some(2) => JudgeMode::Interactive,
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
            JudgeMode::SpecialJudge | JudgeMode::Interactive => true,
            _ => false
        }
    }

    /// Determine whether the jury program has been compiled successfully.
    pub fn jury_compile_succeeded(&self) -> bool {
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

impl From<ProblemInfo> for ProblemMetadata {
    fn from(pi: ProblemInfo) -> Self {
        let jury_src = match pi.judge_mode {
            JudgeMode::Standard => None,
            _ => Some(pi.jury_src)
        };
        let jury_lang = match pi.judge_mode {
            JudgeMode::Standard => None,
            _ => Some(pi.jury_lang)
        };

        ProblemMetadata {
            id: pi.id,
            judge_mode: pi.judge_mode,
            time_limit: pi.time_limit,
            memory_limit: pi.memory_limit,
            jury_src,
            jury_lang,
            jury_exec_path: None,
            archive_id: pi.archive_id,
            timestamp: pi.timestamp,
        }
    }
}

/// Provide access to the problem metadata store.
pub struct ProblemStore {
    /// Lock for accessing specific problem.
    lock: KeyLock<ObjectId>,

    /// Connection to the sqlite database containing problem metadata.
    db: Arc<SqliteConnection>,

    /// RESTful client connected to the judge board server.
    rest: Arc<RestfulClient>,

    /// Fork server client connected to the fork server.
    fork_server: Arc<ForkServerClient>,

    /// Path to the directory containing compiled jury programs.
    jury_dir: PathBuf,
}

impl ProblemStore {
    /// Create a new `ProblemStore` instance.
    pub(super) fn new<P>(
        db: Arc<SqliteConnection>,
        rest: Arc<RestfulClient>,
        fork_server: Arc<ForkServerClient>,
        jury_dir: P) -> Result<Self>
        where P: Into<PathBuf> {
        let store = ProblemStore {
            lock: KeyLock::new(),
            db,
            rest,
            fork_server,
            jury_dir: jury_dir.into()
        };
        store.init_db()?;

        // Create jury_dir if it does not exist.
        std::fs::create_dir_all(&store.jury_dir)?;

        Ok(store)
    }

    fn init_db(&self) -> Result<()> {
        if self.db.get_table_names()?.contains(&String::from("problems")) {
            log::debug!("Table `problems` already exists in the sqlite database.");
            return Ok(());
        }

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

    /// Get the last update timestamp of the specified problem's metadata.
    fn get_timestamp(&self, id: ObjectId) -> Result<Option<u64>> {
        self.db.execute(move |conn| {
            let mut cursor = conn.prepare("SELECT timestamp FROM problems WHERE id = ?")?
                                .cursor();
            cursor.bind(&[sqlite::Value::String(id.to_string())])?;
            match cursor.next()? {
                Some(v) => match v[0].as_integer() {
                    Some(i) => Ok(Some(crate::utils::bitcast::<i64, u64>(i))),
                    None => Ok(None)
                },
                None => Ok(None)
            }
        })
    }

    /// Get the latest timestamp of the specified problem.
    fn get_remote_timestamp(&self, id: ObjectId) -> Result<u64> {
        Ok(self.rest.get_problem_timestamp(id)?)
    }

    /// Compile the jury program. This function returns `Err` to indicate judge errors occured to
    /// compile the jury program, returns `Ok(None)` to indicate the jury program cannot be compiled
    /// due to compilation errors.
    fn compile_jury(&self, jury_src: &str, jury_lang: &LanguageTriple, judge_mode: JudgeMode)
        -> Result<Option<PathBuf>> {
        let kind = match judge_mode {
            JudgeMode::SpecialJudge => judge::ProgramKind::Checker,
            JudgeMode::Interactive => judge::ProgramKind::Interactor,
            _ => unreachable!()
        };
        let result = self.fork_server.compile_source(
            jury_src,
            jury_lang.to_judge_language(),
            kind)?;

        if !result.succeeded {
            log::error!("failed to compile jury: {}", result.compiler_out.unwrap_or_default());
            return Ok(None);
        }

        if result.compiler_out.is_none() {
            log::error!("failed to compile jury: judge returned ok but no output file.");
            return Ok(None);
        }

        Ok(Some(result.output_file.unwrap()))
    }

    /// Get the cached version of the metadata of the specified problem. The returned metadata
    /// might be out of date.
    fn get_cached(&self, id: ObjectId) -> Result<Option<ProblemMetadata>> {
        self.db.execute(|conn| -> Result<Option<ProblemMetadata>> {
            let mut cursor = conn
                .prepare("SELECT * FROM problems WHERE id = ?")?
                .cursor();
            cursor.bind(&[sqlite::Value::String(id.to_string())])?;

            if let Some(row) = cursor.next()? {
                Ok(ProblemMetadata::from_db_row(row))
            } else {
                Ok(None)
            }
        })
    }

    /// Get the problem metadata of the specified problem. The returned metadata is guaranteed to be
    /// the latest version. This function will send a request to the judge board server if the
    /// cached metadata is out of date.
    pub fn get(&self, id: ObjectId) -> Result<ProblemMetadata> {
        self.lock.lock_and_execute(id, |_| {
            if let Some(timestamp) = self.get_timestamp(id)? {
                let remote_ts = self.get_remote_timestamp(id)?;
                if timestamp >= remote_ts {
                    if let Some(metadata) = self.get_cached(id)? {
                        return Ok(metadata);
                    }
                }
            }

            let mut metadata: ProblemMetadata = self.rest.get_problem_info(id)?.into();
            if metadata.has_jury() {
                // Compile jury program.
                log::info!("Compiling jury program for problem \"{}\"", metadata.id);

                // Note that if has_jury function returns true then jury_src and jury_lang used below
                // must be `Some`.
                let jury_exec_temp_path = self.compile_jury(
                    metadata.jury_src.as_ref().expect("failed to get source code of jury"),
                    metadata.jury_lang.as_ref().expect("failed to get language of jury"),
                    metadata.judge_mode)?;

                if jury_exec_temp_path.is_some() {
                    // Copy the jury executable to the jury directory.
                    let jury_exec_temp_path = jury_exec_temp_path.unwrap();
                    let jury_exec_ext = jury_exec_temp_path.extension();

                    // The file name of the jury executable should be {problemId}.{extension} under the
                    // jury executable directory. Build the jury executable's file name now.
                    let mut jury_exec_path = self.jury_dir.clone();
                    jury_exec_path.push(id.to_string());
                    if jury_exec_ext.is_some() {
                        jury_exec_path.set_extension(jury_exec_ext.unwrap());
                    }

                    // And do the copy.
                    std::fs::copy(&jury_exec_temp_path, &jury_exec_path)?;

                    metadata.jury_exec_path = Some(jury_exec_path);
                }
            }

            metadata.save(self.db.as_ref())?;

            Ok(metadata)
        })
    }
}

/// Provide extension functions for `JudgeMode`.
trait JudgeModeExt {
    /// Determine whether jury program is needed for this judge mode.
    fn need_jury(&self) -> bool;
}

impl JudgeModeExt for JudgeMode {
    fn need_jury(&self) -> bool {
        use JudgeMode::*;
        match self {
            SpecialJudge | Interactive => true,
            _ => false,
        }
    }
}
