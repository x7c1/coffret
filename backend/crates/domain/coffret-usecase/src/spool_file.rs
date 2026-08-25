use std::path::{Path, PathBuf};

use coffret_model::ContentHash;
use md5::{Digest, Md5};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::local_error::LocalError;
use crate::local_operation::LocalOperation;

/// How much of a Container is hashed and written at a time.
///
/// The two digests are folded in as the bytes go to disk rather than by reading
/// the spool back, which is what makes the size of one write a question at all.
/// 64 KiB is large enough that the syscall cost disappears against the bytes it
/// moves, and small enough that a Pack of any size still costs one of these.
pub(crate) const WRITE_CHUNK: usize = 64 * 1024;

/// One Container's ciphertext on its way to disk, digested as it passes.
///
/// Both digests are folded in as the bytes are written. Reading the file back
/// to hash it would double the I/O of every upload and would answer a different
/// question anyway — what is on disk now, rather than what was written.
pub(crate) struct SpoolFile {
    file: fs::File,
    path: PathBuf,
    blake3: blake3::Hasher,
    md5: Md5,
    len: u64,
}

impl SpoolFile {
    /// Opens a spool file for one Container, replacing anything at that path.
    pub(crate) async fn create(path: impl Into<PathBuf>) -> Result<Self, LocalError> {
        let path = path.into();
        let file = fs::File::create(&path)
            .await
            .map_err(|cause| LocalError::io(LocalOperation::Creating, &path, cause))?;
        Ok(Self {
            file,
            path,
            blake3: blake3::Hasher::new(),
            md5: Md5::new(),
            len: 0,
        })
    }

    /// Writes the next stretch of ciphertext, digesting it on the way past.
    pub(crate) async fn write(&mut self, bytes: &[u8]) -> Result<(), LocalError> {
        for chunk in bytes.chunks(WRITE_CHUNK) {
            self.blake3.update(chunk);
            self.md5.update(chunk);
            self.file
                .write_all(chunk)
                .await
                .map_err(|cause| LocalError::io(LocalOperation::Writing, &self.path, cause))?;
            self.len += chunk.len() as u64;
        }
        Ok(())
    }

    /// Flushes the file and answers with what was written.
    ///
    /// Flushed to the device, because the point of a spool is to be there after
    /// the run that wrote it is not.
    pub(crate) async fn finish(self) -> Result<Digests, LocalError> {
        self.file
            .sync_all()
            .await
            .map_err(|cause| LocalError::io(LocalOperation::Flushing, &self.path, cause))?;
        Ok(Digests {
            blake3: ContentHash::from_bytes(*self.blake3.finalize().as_bytes()),
            md5: self
                .md5
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            len: self.len,
        })
    }
}

/// What writing a spool file produced besides the file.
///
/// What each of the two digests is good for is documented where they land, on
/// `SpooledContainer`.
pub(crate) struct Digests {
    pub(crate) blake3: ContentHash,
    pub(crate) md5: String,
    pub(crate) len: u64,
}

/// Removes one spool file, its Container having been committed or abandoned.
///
/// A file that is already gone is the same outcome as one this call removed, so
/// an interrupted cleanup is simply run again (spec: OC-6).
pub(crate) async fn discard(spool_path: &Path) -> Result<(), LocalError> {
    match fs::remove_file(spool_path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(cause) => Err(LocalError::io(LocalOperation::Removing, spool_path, cause)),
    }
}
