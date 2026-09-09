use coffret_usecase::SourceReader;

use crate::error::Result;

/// A confined local regular file held open for streaming.
pub struct LocalFile {
    reader: Box<dyn SourceReader>,
}

impl LocalFile {
    pub(crate) fn new(reader: Box<dyn SourceReader>) -> Self {
        Self { reader }
    }

    /// The length obtained from this same open handle.
    pub fn len(&self) -> u64 {
        self.reader.len()
    }

    /// Whether this file is empty.
    pub fn is_empty(&self) -> bool {
        self.reader.is_empty()
    }

    /// Reads the next stretch of the retained file handle.
    pub async fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
        Ok(self.reader.read(buffer).await?)
    }
}
