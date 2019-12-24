//! This module manages connections to the underlying sqlite database used for caching.
//!

use std::path::Path;
use std::sync::Mutex;

use sqlite::Connection;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        SqliteError(::sqlite::Error);
    }
}

/// Represent a database connection to the sqlite database.
pub struct SqliteConnection {
    /// The raw connection protected by a `Mutex`.
    raw: Mutex<Connection>,
}

impl SqliteConnection {
    /// Create a new `SqliteConnection` instance connecting to a sqlite database instance stored
    /// in the specified file.
    pub fn new<P>(path: P) -> Result<Self>
        where P: AsRef<Path> {
        let raw = Connection::open(path)?;
        Ok(SqliteConnection { raw: Mutex::new(raw) })
    }

    /// Execute the given callback on the underlying raw connection.
    pub fn execute<F, R>(&self, callback: F) -> R
        where F: FnOnce(&Connection) -> R {
        let lock = self.raw.lock().expect("failed to lock mutex of the sqlite connection.");
        callback(&*lock)
    }

    /// Get names of all tables contained in the database instance.
    pub fn get_table_names(&self) -> Result<Vec<String>> {
        self.execute(|conn| {
            let mut names: Vec<String> = Vec::new();
            conn.iterate("SELECT name FROM sqlite_master WHERE type='table'", |pairs| {
                for (_, value) in pairs.iter() {
                    if value.is_none() {
                        continue;
                    }
                    names.push(String::from(value.unwrap()));
                }
                true
            })?;
            Ok(names)
        })
    }
}
