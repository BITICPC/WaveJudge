//! This module implements some synchronization primitives that are used in this crate.
//!

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

/// Provide a lock associated with unique keys.
pub struct KeyLock<K>
    where K: Hash + Eq + Clone {
    /// Keys and locks.
    keys: Mutex<HashMap<K, KeyLockEntry>>,
}

impl<K> KeyLock<K>
    where K: Hash + Eq + Clone {
    /// Create a new `KeyLock` object.
    pub fn new() -> Self {
        KeyLock {
            keys: Mutex::new(HashMap::new()),
        }
    }

    /// Acquire a lock on the specified key.
    pub fn lock_and_execute<'s, F, R>(&'s self, key: K, action: F) -> R
        where F: FnOnce(&K) -> R {
        let mtx = {
            let mut keys_lock = self.keys.lock().expect("failed to lock mutex");

            // If the key is not active, add it to the hash map.
            if !keys_lock.contains_key(&key) {
                keys_lock.insert(key.clone(), KeyLockEntry::new());
            }

            // Increase the waiting thread count on the key lock.
            let mut entry = keys_lock.get_mut(&key).unwrap();
            entry.count += 1;

            entry.mtx.clone()
        };

        // Lock the internal mutex and execute the action.
        let ret = {
            let _lock = mtx.lock().expect("failed to lock mutex");
            action(&key)
        };

        // Decrease the waiting thread count on the key lock and remove the corresponding hash map
        // entry on necessary.
        {
            let mut keys_lock = self.keys.lock().expect("failed to lock mutex");
            let mut entry = keys_lock.get_mut(&key).unwrap();
            entry.count -= 1;
            if entry.count == 0 {
                keys_lock.remove(&key);
            }
        }

        ret
    }
}

/// Entry in a key lock.
struct KeyLockEntry {
    /// Number of threads waiting on the lock.
    count: u32,

    /// The mutex used for lock.
    mtx: Arc<Mutex<()>>,
}

impl KeyLockEntry {
    /// Create a new `KeyLockEntry` instance.
    fn new() -> Self {
        KeyLockEntry {
            count: 0,
            mtx: Arc::new(Mutex::new(())),
        }
    }
}
