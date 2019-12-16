//! This module manages connections to the underlying sqlite database used for caching.
//!

use std::sync::Mutex;

use sqlite::Connection;

use crate::utils::Once;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        SqliteError(::sqlite::Error);
    }
}

/// The singleton connection to the sqlite database.
static mut CONNECTION: Option<Mutex<Connection>> = None;
/// The `Once` guard for the underlying sqlite connection.
static CONNECTION_ONCE: Once = Once::new();

/// Initialize the underlying singleton connection to the sqlite database.
pub fn init() -> Result<()> {
    let once_ret = CONNECTION_ONCE.call_once(|| {
        let app_config = crate::config::app_config();
        log::info!("Initializing sqlite connection from db file: {}",
            app_config.storage.db_file.display());

        let conn = Connection::open(&app_config.storage.db_file)?;
        unsafe { CONNECTION.replace(Mutex::new(conn)) };

        Ok(())
    });
    match once_ret {
        Some(Err(e)) => Err(e),
        _ => Ok(())
    }
}

/// Get names of all tables contained in the sqlite database.
pub fn get_table_names() -> Result<Vec<String>> {
    execute(|conn| {
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

/// Execute actions on the underlying sqlite connection. This function panics if the underlying
/// sqlite connection has not been initialized.
pub fn execute<F, R>(callback: F) -> R
    where F: for<'conn> FnOnce(&'conn Connection) -> R {
    let lock = unsafe {
        CONNECTION.as_ref().expect("sqlite connection has not been initialized.")
                  .lock().expect("failed to lock mutex of the sqlite connection.")
    };

    callback(&*lock)
}
