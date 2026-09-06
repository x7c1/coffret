use std::path::Path;

use coffret_model::ContentHash;
use md5::{Digest, Md5};

use crate::local_error::LocalError;
use crate::spool::Spool;
use crate::spool_writer::SpoolWriter;

/// How much of a Container is hashed and written at a time.
///
/// The two digests are folded in as the bytes go to disk rather than by reading
/// the spool back, which is what makes the size of one write a question at all.
/// 64 KiB is large enough that the syscall cost disappears against the bytes it
/// moves, and small enough that a Pack of any size still costs one of these.
pub(crate) const WRITE_CHUNK: usize = 64 * 1024;

/// One Container's ciphertext on its way to the spool, digested as it passes.
///
/// Both digests are folded in as the bytes are written. Reading the file back
/// to hash it would double the I/O of every upload and would answer a different
/// question anyway — what is on disk now, rather than what was written.
///
/// The bytes themselves go through [`Spool`], which is where the filesystem is.
/// What stays here is what the digests are for, and neither of them is a promise
/// about a disk: the BLAKE3 is what the Journal record carries about the
/// Container (spec: FM-15, CP-11), and the MD5 is what the provider's own report
/// of what it stored is compared against.
pub(crate) struct SpoolFile {
    writer: Box<dyn SpoolWriter>,
    blake3: blake3::Hasher,
    md5: Md5,
    len: u64,
}

impl SpoolFile {
    /// Opens a spool file for one Container, replacing anything at that path.
    ///
    /// The pending row naming this path is already written when this is called,
    /// so a failure here — a full disk, a directory that went away — leaves a
    /// row naming a file that never came to exist. That is a state disposal
    /// tolerates rather than one it has to be spared: see [`Spool::discard`].
    pub(crate) async fn create(spool: &dyn Spool, path: &Path) -> Result<Self, LocalError> {
        Ok(Self {
            writer: spool.create(path).await?,
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
            self.writer.write(chunk).await?;
            self.len += chunk.len() as u64;
        }
        Ok(())
    }

    /// Flushes the file to the device and answers with what was written.
    ///
    /// Flushed, because the point of a spool is to be there after the run that
    /// wrote it is not — and the writer is spent in the flushing, so what a
    /// caller holds afterwards are the digests of a Container that is on the
    /// device (spec: OC-2).
    pub(crate) async fn finish(self) -> Result<Digests, LocalError> {
        self.writer.finish().await?;
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
