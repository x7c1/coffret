use coffret_usecase::SourceReader;

use crate::error::{Error, Result};

/// One dropped file, open for reading.
pub struct DropReader {
    reader: Box<dyn SourceReader>,
}

impl DropReader {
    pub(super) fn new(reader: Box<dyn SourceReader>) -> Self {
        Self { reader }
    }

    /// Fills `buffer` with the next stretch of the file; zero is its end.
    pub async fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
        self.reader.read(buffer).await.map_err(Error::Local)
    }
}
