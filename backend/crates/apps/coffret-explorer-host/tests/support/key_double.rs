use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use coffret_explorer_host::KeySource;

use super::SERVER_KEY;

/// A key file the case writes into, and that counts how often it is read.
pub struct KeyDouble {
    key: Mutex<io::Result<String>>,
    reads: AtomicUsize,
}

impl KeyDouble {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            key: Mutex::new(Ok(SERVER_KEY.to_owned())),
            reads: AtomicUsize::new(0),
        })
    }

    /// What the next read answers.
    pub fn set(&self, key: io::Result<String>) {
        *self.key.lock().unwrap() = key;
    }

    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

impl KeySource for KeyDouble {
    fn read(&self) -> io::Result<String> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        match &*self.key.lock().unwrap() {
            Ok(key) => Ok(key.clone()),
            Err(cause) => Err(io::Error::new(cause.kind(), "a read the case refused")),
        }
    }
}
