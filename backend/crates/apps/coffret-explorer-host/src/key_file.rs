use std::io;
use std::path::PathBuf;

use coffret_device::ServerKey;

use crate::KeySource;

/// The file a server published its key into (spec: LA-3), read afresh each
/// time it is asked for, because a server that was started again drew a new
/// key (spec: LA-4) and this host is how it finds out.
#[derive(Debug, Clone)]
pub struct KeyFile(pub PathBuf);

impl KeySource for KeyFile {
    fn read(&self) -> io::Result<String> {
        ServerKey::read_published(&self.0)
    }
}
