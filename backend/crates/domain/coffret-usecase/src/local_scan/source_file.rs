use std::path::PathBuf;

use coffret_model::{Btime, EntryPath, Mtime};

use crate::local_error::LocalError;
use crate::mapped_roots::MappedRoots;
use crate::source_reader::SourceReader;

/// How much of a source file is taken at a time by the step that reads the whole
/// of it.
///
/// The same size the spool writes in, and for the same reason: large enough that
/// the syscall cost disappears against the bytes it moves, small enough that one
/// of these is all a read of any file costs.
const READ_CHUNK: usize = 64 * 1024;

/// One local file the scan found, at the Library position it stands for.
///
/// The observed values are what a filesystem answers cheaply, and the three a
/// scan compares are the whole of what it holds against
/// [`LocalObservation`](crate::device_state::LocalObservation) before deciding
/// to read a file at all: a file whose length and modification time are what
/// this device last saw is not opened (spec: EP-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceFile {
    /// The Library position the file stands at, derived from the mapping it was
    /// found under (spec: EP-9).
    pub(crate) path: EntryPath,
    /// Where the file is on this device.
    ///
    /// Device state and nothing else: it never travels into a Container, a
    /// Journal record, or a log line.
    pub(crate) local_path: PathBuf,
    /// The file's length in bytes when the scan looked.
    pub(crate) size: u64,
    /// The file's modification time when the scan looked, which is the value
    /// the Entry carries (spec: FM-9).
    pub(crate) mtime: Mtime,
    /// The file's birth time, where the platform reports one (spec: FM-9).
    ///
    /// Not compared against anything: it is captured because this is the one
    /// moment it can be, and a platform that reports none leaves it absent.
    pub(crate) btime: Option<Btime>,
}

impl SourceFile {
    /// The file's whole plaintext.
    ///
    /// For the steps that can afford it: a sync's scan hashes a candidate to
    /// settle whether it really changed, and its spool encodes one file into a
    /// Container of its own. A Pack cannot be read this way, which is what
    /// [`open`](Self::open) is for.
    ///
    /// Built out of the same reader rather than out of a whole-file call of its
    /// own, so that a capability answering both would have one behaviour to get
    /// right instead of two — and so that the length is the read's answer, not
    /// a stat's.
    pub(crate) async fn read(&self, roots: &dyn MappedRoots) -> Result<Vec<u8>, LocalError> {
        let mut reader = self.open(roots).await?;
        let mut content = Vec::new();
        let mut buffer = vec![0u8; READ_CHUNK];
        loop {
            let filled = reader.read(&mut buffer).await?;
            if filled == 0 {
                return Ok(content);
            }
            content.extend_from_slice(&buffer[..filled]);
        }
    }

    /// Opens the file to be walked a buffer at a time.
    ///
    /// What a Pack does with every file it holds — hashing it before the entry
    /// table is written, and feeding it through the encoder afterwards — so
    /// neither step is bounded by what fits in memory (spec: FM-2, FM-5, FM-9).
    pub(crate) async fn open(
        &self,
        roots: &dyn MappedRoots,
    ) -> Result<Box<dyn SourceReader>, LocalError> {
        Ok(roots.open_source(&self.local_path).await?)
    }
}
