pub mod archives;
mod db;
pub mod problems;

use std::path::Path;
use std::sync::Arc;

use archives::ArchiveStore;
use db::SqliteConnection;
use problems::ProblemStore;

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
    pub fn new<P1, P2>(db_file: &P1, archive_dir: &P2, rest: Arc<RestfulClient>) -> Result<Self>
        where P1: ?Sized + AsRef<Path>,
              P2: ?Sized + AsRef<Path> {
        let db_conn = db::SqliteConnection::new(db_file)?;

        let arc_db = Arc::new(db_conn);
        let problem_db = arc_db.clone();

        let archive_rest = rest.clone();
        let problem_rest = rest.clone();

        Ok(AppStorageFacade {
            db: arc_db,
            rest,
            archive_store: ArchiveStore::new(archive_dir, archive_rest),
            problem_store: ProblemStore::new(problem_db, problem_rest)?,
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
