use std::ops::Range;

use coffret_format::{
    unwrap_container_key, ChunkRun, ChunkRunReader, ContainerOutline, Error as FormatError, Header,
};
use coffret_model::{ContainerKey, ContainerSummary, EntryMetadata, KeyEnvelope, ObjectRef};
use tokio::io::AsyncReadExt;
use tracing::debug;

use crate::byte_stream::ByteStream;
use crate::commit::ControlListing;
use crate::error::{Error, Result};
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::placement::{discard_all, Placement};
use crate::fetch::target::Target;
use crate::fetch::TRANSFER_BUFFER;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// Reads one Entry out of a Container without pulling the rest of it.
///
/// A Container says where everything in it is before any of it arrives, so this
/// is three reads and never the object: the header, which says how long the meta
/// section is; the meta section, which places every Entry in the plaintext
/// stream; and the chunks covering exactly the Entry that was asked for
/// (spec: FM-2, FM-5, FM-9). A Pack is sized in gigabytes, and a reader wanting
/// one page out of one waits for a page (spec: PK-5, PK-16).
///
/// What it cannot do is check the object's own hash. That hash is a claim about
/// bytes this deliberately did not ask for, and asking for them would be the
/// whole-Container fetch. The gates for the bytes that do arrive are the ones
/// that hold over a range: every chunk authenticates on its own, under a nonce
/// carrying the position it holds in this object and this Container's header as
/// associated data (spec: FM-5, FM-7, FM-8), and the Entry's plaintext is then
/// held against what the current catalog records for it before the file becomes
/// visible (spec: CP-11, EP-11).
///
/// Those gates say what the bytes are, once they are here. What decides how many
/// bytes are worth asking for is in front of them: the first read is 32 fixed
/// bytes, and the length that read hands back has already been held against the
/// ceiling a Container's meta section may be (spec: FM-2), so the second request
/// is aimed at a bounded extent rather than at whatever four unauthenticated
/// bytes claimed. A Container whose header lies about it is declined here, along
/// with the Entry that named it, and no other Entry's fetch is touched.
pub(super) async fn read_entry<'a>(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    keys: &LibraryKeys,
    listing: &ControlListing,
    summary: &ContainerSummary,
    envelope: &KeyEnvelope,
    target: &'a Target,
) -> FetchResult<Placement<'a>> {
    let container_id = summary.id;
    let object: &ObjectRef = summary
        .object_ref
        .as_ref()
        .or_else(|| listing.container(container_id))
        .ok_or(FetchError::ContainerUnreachable { container_id })?;
    let key = unwrap_container_key(keys.container_wrap(), &container_id, envelope)?;

    let outline = front(store, retry, object, &key).await?;
    let entry = outline
        .entry_at(target.path())
        .ok_or_else(|| FetchError::EntryMissing {
            container_id,
            path: target.path().clone(),
        })?
        .clone();

    // A chunk is the smallest thing that authenticates, so the Entry's own
    // extent is rounded out to the chunks covering it, and the bytes in front of
    // it inside the first chunk are stepped over (spec: FM-5).
    let run = outline.chunks_covering(entry.extent.range())?;
    let asked = run.ciphertext();

    // Every attempt opens a fresh stream and writes a fresh temporary file, the
    // same contract the whole-Container fetch keeps.
    let placement = retry
        .run("get", || {
            let asked = asked.clone();
            let entry = entry.clone();
            let outline = &outline;
            let key = &key;
            async move {
                let stream = store.get(object, Some(asked)).await?;
                write_entry(stream, outline, key, run, target, entry).await
            }
        })
        .await??;

    debug!(
        container = %container_id,
        object = %container_id.object_name(),
        chunks = run.count(),
        of = outline.chunk_count(),
        bytes = asked.end - asked.start,
        "range-read one Entry out of a Container",
    );
    Ok(placement)
}

/// The header and the meta section, read off the front of the object.
///
/// Two reads rather than one guess: the header's 32 plaintext bytes say how long
/// the meta section behind them is, so the second read asks for exactly that
/// (spec: FM-2). Neither read grows with the Container behind it, and neither
/// grows with what the header claims either: the claim is refused here if it is
/// past what a meta section may be, which is before the second read is issued
/// and before anything is sized by it.
async fn front(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    object: &ObjectRef,
    key: &ContainerKey,
) -> FetchResult<ContainerOutline> {
    let mut front = ranged(store, retry, object, 0..Header::LEN as u64).await?;
    let front_len = ContainerOutline::prefix_len(&front)?;
    let meta = ranged(store, retry, object, Header::LEN as u64..front_len).await?;
    front.extend_from_slice(&meta);
    Ok(ContainerOutline::open(&front, key)?)
}

/// One attempt: open the run's chunks as they arrive and write the Entry out.
///
/// The two error channels are the two answers the whole-Container fetch draws
/// too. The outer one is Storage's — a transfer that failed or came up short,
/// which the policy may attempt again — and the inner one is a verdict about the
/// Library, which no later attempt would change. Either way the temporary file
/// this attempt made is gone before it returns.
async fn write_entry<'a>(
    stream: ByteStream,
    outline: &ContainerOutline,
    key: &ContainerKey,
    run: ChunkRun,
    target: &'a Target,
    entry: EntryMetadata,
) -> Result<FetchResult<Placement<'a>>> {
    let wanted = entry.extent.range();
    let mut placement = match Placement::open(target, entry).await {
        Ok(placement) => placement,
        Err(error) => return Ok(Err(error)),
    };

    let expected = stream.len();
    let mut reader = stream.into_reader();
    let mut buffer = vec![0u8; TRANSFER_BUFFER];
    let mut chunks = ChunkRunReader::begin(outline, key, &run);
    let mut plaintext = Vec::new();
    // Where in the Container's plaintext stream the next opened byte stands.
    let mut position = run.plaintext_start();
    let mut received = 0u64;

    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(read) => read,
            Err(cause) => {
                discard_all(vec![placement]).await;
                return Err(Error::from(cause));
            }
        };
        if read == 0 {
            break;
        }
        received += read as u64;

        plaintext.clear();
        let opened = chunks.read(&buffer[..read], &mut plaintext);
        if let Err(error) = opened {
            discard_all(vec![placement]).await;
            return Ok(Err(FetchError::Format(error)));
        }

        let piece = match OpenedPiece::at(position, &plaintext) {
            Ok(piece) => piece,
            Err(error) => {
                discard_all(vec![placement]).await;
                return Ok(Err(error));
            }
        };
        position = piece.end();
        if let Some(bytes) = piece.overlapping(&wanted) {
            let written = placement.write(bytes).await;
            if let Err(error) = written {
                discard_all(vec![placement]).await;
                return Ok(Err(error));
            }
        }
    }

    if received != expected {
        discard_all(vec![placement]).await;
        // Told apart the way the drains tell them apart: a short answer is known
        // exactly, and a long one is only known to be long, because the reader
        // stopped one byte past the declaration rather than following it.
        return Err(if received > expected {
            Error::LengthOverrun { expected }
        } else {
            Error::LengthMismatch {
                expected,
                actual: received,
            }
        });
    }
    let finished = chunks.finish();
    if let Err(error) = finished {
        discard_all(vec![placement]).await;
        return Ok(Err(FetchError::Format(error)));
    }
    let verified = placement.verify().await;
    if let Err(error) = verified {
        discard_all(vec![placement]).await;
        return Ok(Err(error));
    }
    Ok(Ok(placement))
}

/// One piece of a Container's plaintext stream as the chunk decoder handed it
/// over, and where that piece stands in the stream.
///
/// It exists so that the arithmetic between a stream position and an index into
/// a buffer is done once. A position is a `u64` because a Container's stream is
/// addressed in 64 bits (spec: FM-4, FM-9) and an index is a `usize` because a
/// buffer is as long as this machine can address, and a cast between the two
/// made in passing is where a 32-bit reader would quietly deliver the wrong
/// bytes. Every conversion here is of a distance from this piece's own start to
/// a position inside it, which is at most the piece's own length — so the
/// conversion cannot fail, and it says so rather than truncating.
struct OpenedPiece<'a> {
    /// Where this piece's first byte stands in the plaintext stream.
    start: u64,
    /// The bytes themselves.
    bytes: &'a [u8],
}

impl<'a> OpenedPiece<'a> {
    /// The piece standing at `start`, or the refusal a stream reaching past
    /// what 64 bits can address earns.
    ///
    /// No writer produces such a Container — the layout one is written from
    /// refuses an entry table that would need it (spec: FM-9) — so this is the
    /// walk saying so instead of wrapping round and writing bytes from the
    /// wrong place into a file.
    fn at(start: u64, bytes: &'a [u8]) -> FetchResult<Self> {
        match start.checked_add(bytes.len() as u64) {
            Some(_) => Ok(Self { start, bytes }),
            None => Err(FetchError::Format(FormatError::StreamTooLong)),
        }
    }

    /// The first stream position past this piece.
    fn end(&self) -> u64 {
        self.start + self.bytes.len() as u64
    }

    /// The bytes of this piece that belong to `wanted`, where any of them do.
    fn overlapping(&self, wanted: &Range<u64>) -> Option<&'a [u8]> {
        let from = wanted.start.max(self.start);
        let to = wanted.end.min(self.end());
        (from < to).then(|| &self.bytes[self.index_of(from)..self.index_of(to)])
    }

    /// How far into this piece a stream position inside it stands.
    fn index_of(&self, position: u64) -> usize {
        usize::try_from(position - self.start)
            .expect("a position inside one opened piece is no further in than its own length")
    }
}

/// One short ranged answer, drained into memory.
///
/// Only for the front of an object: a header is 32 bytes and a meta section is
/// bounded by the ceiling a Container's declared meta length is held against,
/// neither of which grows with the Container behind it. The drain is held to the
/// extent that was asked for either way, so a provider answering with more of
/// the object than the range named is stopped at the bound rather than buffered.
/// Everything else a fetch reads goes past the chunk decoder without ever being
/// held.
async fn ranged(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    object: &ObjectRef,
    range: Range<u64>,
) -> Result<Vec<u8>> {
    let asked = range.end - range.start;
    retry
        .run("get", || {
            let range = range.clone();
            async move {
                store
                    .get(object, Some(range))
                    .await?
                    .collect_exact(asked)
                    .await
            }
        })
        .await
}
