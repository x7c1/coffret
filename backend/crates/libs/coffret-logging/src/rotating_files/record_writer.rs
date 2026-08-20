use std::io::{self, Write};

use tracing_subscriber::fmt::MakeWriter;

use super::RotatingFiles;

impl<'writer> MakeWriter<'writer> for RotatingFiles {
    type Writer = RecordWriter<'writer>;

    fn make_writer(&'writer self) -> Self::Writer {
        RecordWriter { files: self }
    }
}

/// One event on its way to the file.
///
/// The formatting layer writes each event with a single call, so one write is
/// one record — which is what lets rotation happen on record boundaries rather
/// than in the middle of a line.
pub struct RecordWriter<'writer> {
    files: &'writer RotatingFiles,
}

impl Write for RecordWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.files.lock().write_record(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.files.lock().flush()
    }
}
