use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::in_memory_fs::state::{lock, State};
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::source_reader::SourceReader;

/// One source file of [`InMemoryFs`](super::InMemoryFs), open for reading.
///
/// The bytes are taken at the open, the way a handle on a real filesystem keeps
/// hold of the file it was given whatever later happens to the name. What it
/// still needs the fake for is the fault script: a read that refuses partway
/// through a Pack's member stream is one of the things this fake exists to
/// arrange, and only the shared state knows how many reads have gone by.
pub(super) struct InMemorySourceReader {
    state: Arc<Mutex<State>>,
    path: PathBuf,
    content: Vec<u8>,
    offset: usize,
}

impl InMemorySourceReader {
    /// A reader over `content`, which the fake held at `path` when it was
    /// opened.
    pub(super) fn new(state: Arc<Mutex<State>>, path: PathBuf, content: Vec<u8>) -> Self {
        Self {
            state,
            path,
            content,
            offset: 0,
        }
    }
}

#[async_trait]
impl SourceReader for InMemorySourceReader {
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, LocalIoError> {
        lock(&self.state).attempt(LocalOperation::Reading, &self.path)?;
        let taken = (self.content.len() - self.offset).min(buffer.len());
        buffer[..taken].copy_from_slice(&self.content[self.offset..self.offset + taken]);
        self.offset += taken;
        Ok(taken)
    }
}
