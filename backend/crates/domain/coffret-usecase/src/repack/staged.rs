use std::ops::Range;

use coffret_format::{unwrap_container_key, ChunkRunReader, ContainerOutline, Header};
use coffret_model::{ContainerKey, Redacted};
use tokio::io::{AsyncRead, AsyncReadExt};
use tracing::warn;

use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::pack_spool::PackSpool;
use crate::repack::repack_error::RepackResult;
use crate::spool::Spool;
use crate::spooled_container::SpooledContainer;

/// How much of a staged copy is read at a time — a transfer buffer, for the
/// reason a rebuild holds one: the bytes go to the chunk reader as they come.
const READ_BUFFER: usize = 128 * 1024;

/// One Pack to be rewritten, read and verified whole and carried, every Entry
/// of it in its own order, into a copy on this device's spool (spec: PK-10).
///
/// The copy is what the new Packs are written from. It is a Container like any
/// other — read-modify-replace's own replacement, keeping every row — so it is
/// ciphertext on the device, under a Container Key that never leaves the run,
/// and a pending row names it from before its first byte (spec: OC-2): a run
/// that stops anywhere leaves nothing the next sync's settlement does not
/// dispose of. It is never uploaded and never committed.
pub(super) struct Staged {
    /// The copy, as the rebuild handed it back.
    pub(super) copy: SpooledContainer,
    /// The key that opens it.
    key: ContainerKey,
}

impl Staged {
    /// Holds on to a copy the rebuild spooled, opening its envelope so the
    /// copy can be read back.
    pub(super) fn new(copy: SpooledContainer, keys: &LibraryKeys) -> RepackResult<Self> {
        let key = unwrap_container_key(keys.container_wrap(), &copy.container_id, &copy.envelope)?;
        Ok(Self { copy, key })
    }
}

/// Removes the staged copies and the rows naming them (spec: OC-2, OC-8).
///
/// Nothing of them was uploaded and no commit was attempted for them, so both
/// are this device's own and removing them is safe. A row goes only once its
/// file has: a copy that would not go stays named, and the next sync's
/// settlement disposes of it as it would any abandoned spool. Nothing here
/// fails the run — the copies were only ever the run's scratch.
pub(super) async fn dispose(index: &dyn Index, spool: &dyn Spool, staged: &[Staged]) {
    for one in staged {
        let container_id = one.copy.container_id;
        match spool.discard(&one.copy.spool_path).await {
            Ok(()) => {
                if let Err(error) = index.clear_pending_row(container_id).await {
                    warn!(
                        container = %container_id,
                        reason = %error.redacted(),
                        "a staged copy's row could not be cleared; the next sync settles it",
                    );
                }
            }
            Err(error) => warn!(
                container = %container_id,
                reason = %error.redacted(),
                "a staged copy could not be removed; its row stays for the next sync",
            ),
        }
    }
}

/// A staged copy being read forward, a chunk of plaintext at a time.
///
/// The members a new Pack carries come in Entry Path order, which takes them
/// from several copies in turn and — where a rename moved an Entry — can take
/// one copy's Entries out of their stream order. So a reader keeps its place
/// in its copy between members and hands over only what it is asked for: an
/// Entry at or after the place it stands is reached by reading on, discarding
/// what lies between, and one behind it by opening the copy again from the
/// start. What it holds between calls is the plaintext of the chunk it stopped
/// in, which is where the next Entry of an ordered Pack begins.
///
/// Every chunk authenticates as it is opened (spec: FM-5, FM-8), and the new
/// Pack's encoder holds each Entry it is fed to the length and hash its row
/// records — the copy was verified three ways as it was written, and this is
/// verified again as it is read.
pub(super) struct Cursor<'a> {
    staged: &'a Staged,
    open: Option<Open>,
}

/// The copy, opened.
struct Open {
    reader: Box<dyn AsyncRead + Send + Unpin>,
    chunks: ChunkRunReader,
    /// Plaintext decoded and not yet passed over.
    plaintext: Vec<u8>,
    /// Where `plaintext` begins in the copy's plaintext stream.
    at: u64,
    /// What the next read of the copy lands in, kept between members.
    buffer: Vec<u8>,
}

impl<'a> Cursor<'a> {
    /// A reader over `staged` that has not opened it yet.
    pub(super) fn new(staged: &'a Staged) -> Self {
        Self { staged, open: None }
    }

    /// Feeds the copy's plaintext over `extent` into `pack`.
    pub(super) async fn carry(
        &mut self,
        spool: &dyn Spool,
        extent: Range<u64>,
        pack: &mut PackSpool,
    ) -> RepackResult<()> {
        if extent.is_empty() {
            // An Entry of no bytes is fed nothing: the encoder closes it as
            // the next one begins (spec: FM-9).
            return Ok(());
        }
        let behind = self.open.as_ref().is_none_or(|open| open.at > extent.start);
        if behind {
            self.open = Some(self.reopen(spool).await?);
        }
        let open = self.open.as_mut().expect("opened just above");

        let mut next = extent.start;
        loop {
            // What lies between where the reader stands and the Entry is
            // another Entry's, or padding, and is passed over.
            let skip = usize::try_from(next - open.at)
                .unwrap_or(usize::MAX)
                .min(open.plaintext.len());
            open.plaintext.drain(..skip);
            open.at += skip as u64;
            if open.at == next && !open.plaintext.is_empty() {
                let take = usize::try_from(extent.end - next)
                    .unwrap_or(usize::MAX)
                    .min(open.plaintext.len());
                pack.write(&open.plaintext[..take]).await?;
                open.plaintext.drain(..take);
                open.at += take as u64;
                next += take as u64;
                if next == extent.end {
                    return Ok(());
                }
            }
            let read = open
                .reader
                .read(&mut open.buffer)
                .await
                .map_err(|cause| unread(&self.staged.copy.spool_path, cause))?;
            if read == 0 {
                // The copy ended before an Entry its table places: this
                // device's own file is not what it wrote.
                return Err(coffret_format::Error::Truncated.into());
            }
            open.chunks
                .read(&open.buffer[..read], &mut open.plaintext)?;
        }
    }

    /// Opens the copy from its first byte: the header and the meta section,
    /// then a chunk reader over the whole chunk sequence (spec: FM-2, FM-5).
    async fn reopen(&self, spool: &dyn Spool) -> RepackResult<Open> {
        let path = &self.staged.copy.spool_path;
        let mut reader = spool.reader(path).await?;
        let mut front = vec![0u8; Header::LEN];
        reader
            .read_exact(&mut front)
            .await
            .map_err(|cause| unread(path, cause))?;
        let declared = ContainerOutline::prefix_len(&front)?;
        let front_len = usize::try_from(declared).map_err(|_| {
            coffret_format::Error::UnaddressableOnThisBuild {
                what: "header and meta section",
                declared,
            }
        })?;
        front.resize(front_len, 0);
        reader
            .read_exact(&mut front[Header::LEN..])
            .await
            .map_err(|cause| unread(path, cause))?;
        let outline = ContainerOutline::open(&front, &self.staged.key)?;
        let chunks = ChunkRunReader::begin(&outline, &self.staged.key, &outline.all_chunks());
        Ok(Open {
            reader,
            chunks,
            plaintext: Vec::new(),
            at: 0,
            buffer: vec![0u8; READ_BUFFER],
        })
    }
}

/// A staged copy that could not be read back, named by its path in the value
/// and never in the message, as every spool failure is.
fn unread(path: &std::path::Path, cause: std::io::Error) -> LocalIoError {
    LocalIoError {
        operation: LocalOperation::Reading,
        path: path.to_path_buf(),
        cause,
    }
}
