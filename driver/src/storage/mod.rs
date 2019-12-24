pub mod archives;
mod db;
pub mod problems;

use std::sync::Arc;

use archives::ArchiveStore;
use db::SqliteConnection;
use problems::ProblemStore;

use crate::config::AppConfig;
use crate::forkserver::ForkServerClient;
use crate::restful::RestfulClient;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    links {
        ArchivesError(archives::Error, archives::ErrorKind);
        DbError(db::Error, db::ErrorKind);
        ProblemsError(problems::Error, problems::ErrorKind);
    }
}

/// Provide a facade of the storage subsystem used in WaveJudge.
pub struct AppStorageFacade {
    db: Arc<SqliteConnection>,

    /// The RESTful client.
    rest: Arc<RestfulClient>,

    /// The archive store.
    archive_store: ArchiveStore,

    /// The problem store.
    problem_store: ProblemStore,
}

impl AppStorageFacade {
    /// Create a new `AppStorage` object.
    pub fn new(
        config: &AppConfig,
        rest: Arc<RestfulClient>,
        fork_server: Arc<ForkServerClient>) -> Result<Self> {
        let db_conn = db::SqliteConnection::new(&config.storage.db_file)?;

        let arc_db = Arc::new(db_conn);
        let problem_db = arc_db.clone();

        let archive_rest = rest.clone();
        let problem_rest = rest.clone();

        Ok(AppStorageFacade {
            db: arc_db,
            rest,
            archive_store: ArchiveStore::new(&config.storage.archive_dir, archive_rest),
            problem_store: ProblemStore::new(
                problem_db, problem_rest, fork_server, &config.storage.jury_dir)?,
        })
    }

    /// Get the archive store.
    pub fn archives(&self) -> &ArchiveStore {
        &self.archive_store
    }

    /// Get the problem store.
    pub fn problems(&self) -> &ProblemStore {
        &self.problem_store
    }
}
