pub mod archives;
mod db;
pub mod problems;

use std::sync::Arc;

use archives::ArchiveStore;
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
    /// The archive store.
    pub archives: ArchiveStore,

    /// The problem store.
    pub problems: ProblemStore,
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
            archives: ArchiveStore::new(&config.storage.archive_dir, archive_rest),
            problems: ProblemStore::new(
                problem_db, problem_rest, fork_server, &config.storage.jury_dir)?,
        })
    }
}
