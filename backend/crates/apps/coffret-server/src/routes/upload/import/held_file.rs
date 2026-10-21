use axum::body::Bytes;
use coffret_device::{DropReader, DroppedFile};

use crate::allowance::Allowance;
use crate::api_error::ApiError;

use super::super::receive::{Incoming, Times};
use super::super::refusal::Refusal;

/// How much of a held file is read at a time.
const CHUNK: usize = 256 * 1024;

/// One held file, read off this device for [`receive`](super::super::receive).
///
/// It is kept with the times the walk read off the person's file — its
/// modification time and, where the platform reported one, its birth time
/// (spec: EP-11, FM-9) — and it is held to the length the walk stated: a file
/// that grew since it was dropped is refused part way through rather than
/// taken past what this device was asked to have room for, and the scratch it
/// was going to goes with it (spec: EP-11, LA-28).
pub(super) struct HeldFile<'a> {
    file: &'a DroppedFile,
    reader: DropReader,
    read: u64,
}

impl<'a> HeldFile<'a> {
    /// Opens `file` again from where it was dropped, through every component
    /// below that folder without following a link (spec: EP-8).
    pub(super) async fn open(file: &'a DroppedFile) -> Result<Self, ApiError> {
        let reader = file.open().await?;
        Ok(Self {
            file,
            reader,
            read: 0,
        })
    }
}

impl Incoming for HeldFile<'_> {
    const OPERATION: &'static str = super::OPERATION;

    fn times(&self) -> Times {
        Times {
            modified: Some(self.file.mtime()),
            born: self.file.btime(),
        }
    }

    /// No ceiling: what bounds a file imported by path is the room this device
    /// has for it and the length the walk stated, not a budget on what a socket
    /// carries (spec: LA-28).
    fn ceiling(&self, _: &Allowance) -> Option<u64> {
        None
    }

    async fn next_chunk(&mut self) -> Result<Option<Bytes>, Refusal> {
        let mut buffer = vec![0u8; CHUNK];
        let read = self.reader.read(&mut buffer).await?;
        if read == 0 {
            return Ok(None);
        }
        self.read += read as u64;
        if self.read > self.file.size() {
            return Err(Refusal::Part(ApiError::changed_since_dropped()));
        }
        buffer.truncate(read);
        Ok(Some(Bytes::from(buffer)))
    }
}
