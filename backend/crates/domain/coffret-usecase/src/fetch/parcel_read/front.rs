use std::ops::Range;
use std::sync::Arc;

use coffret_format::{ContainerOutline, Error as FormatError, Header};
use coffret_model::{ContainerId, ContainerKey, ObjectRef};
use tracing::debug;

use crate::error::{Error, Result};
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::fetch_memory::{FetchMemory, Turn};
use crate::fetch::storage_reads::Origin;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// Why a front could not be read: the two refusals reading one can meet, in a
/// form every reader waiting on that read can be handed as the reader met it.
#[derive(Debug, Clone)]
pub(crate) enum FrontRefusal {
    /// Storage failed, or answered short.
    Storage(Error),
    /// The header is not one, or declares lengths the object cannot hold.
    Format(FormatError),
}

impl From<Error> for FrontRefusal {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

impl From<FormatError> for FrontRefusal {
    fn from(error: FormatError) -> Self {
        Self::Format(error)
    }
}

impl From<FrontRefusal> for FetchError {
    fn from(refusal: FrontRefusal) -> Self {
        match refusal {
            FrontRefusal::Storage(error) => Self::Storage(error),
            FrontRefusal::Format(error) => Self::Format(error),
        }
    }
}

/// The header and the meta section, read off the front of the object as it
/// arrived, for [`outline`] to open.
///
/// Two reads rather than one guess: the header's 32 plaintext bytes say how long
/// the meta section behind them is, so the second read asks for exactly that
/// (spec: FM-2). Neither read grows with the Container behind it, and neither
/// grows with what the header claims either: the claim is refused here if it is
/// past what a meta section may be, which is before the second read is issued
/// and before anything is sized by it.
///
/// The front belongs to no parcel. It is the same read whichever Entry is
/// wanted, so it shows the provider only that the Container was opened
/// (spec: PK-16, PK-20).
///
/// The meta section is held against the object's recorded length before it is
/// asked for: a header that declares more meta section than the object holds is
/// lying about its own lengths, and asking Storage for bytes that are not there
/// would only have the retry policy ask again for a short answer no attempt can
/// lengthen.
pub(super) async fn front(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    object: &ObjectRef,
    object_len: u64,
) -> std::result::Result<Vec<u8>, FrontRefusal> {
    let mut front = ranged(store, retry, object, 0..Header::LEN as u64).await?;
    let front_len = ContainerOutline::prefix_len(&front)?;
    within_object(&(0..front_len), object_len)?;
    let meta = ranged(store, retry, object, Header::LEN as u64..front_len).await?;
    front.extend_from_slice(&meta);
    Ok(front)
}

/// The same front, out of `memory` where it keeps this Container's, and read
/// and kept there where it does not — or, where another run is reading it
/// already, out of that read once it lands. Answers with where it came from.
///
/// A Container object never changes once committed (spec: FM-2, CP-11), so the
/// bytes kept are the object's own; they are opened under the key each time, as
/// bytes read from Storage are. Kept only once they have opened, so a front
/// that would not authenticate is read again rather than remembered.
///
/// A read under way is joined whoever made it — a page fetch waits on the read
/// ahead a listing set going, and a read ahead on the page fetch — and what it
/// came to is what the waiter gets: the bytes, which it opens under its own
/// key, or the same refusal the reader met. A reader dropped before it landed
/// anything leaves the read to the waiter, which then makes it itself.
pub(in crate::fetch) async fn outline(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    memory: Option<&FetchMemory>,
    container_id: ContainerId,
    object: &ObjectRef,
    object_len: u64,
    key: &ContainerKey,
) -> FetchResult<(ContainerOutline, Origin)> {
    let Some(memory) = memory else {
        let read = front(store, retry, object, object_len).await?;
        return Ok((ContainerOutline::open(&read, key)?, Origin::Read));
    };
    loop {
        let flight = match memory.front_of(container_id) {
            Turn::Kept(kept) => {
                debug!(
                    container = %container_id,
                    "the Container's front is kept from an earlier fetch, so it is not read again",
                );
                return Ok((ContainerOutline::open(&kept, key)?, Origin::Kept));
            }
            Turn::Join(waiter) => match waiter.landed().await {
                Some(landed) => {
                    debug!(
                        container = %container_id,
                        read = landed.is_ok(),
                        "the Container's front was being read already, so this fetch waited for that read",
                    );
                    let landed = landed?;
                    return Ok((ContainerOutline::open(&landed, key)?, Origin::Joined));
                }
                None => {
                    debug!(
                        container = %container_id,
                        "the read of the Container's front this fetch waited for was dropped, so it reads the front itself",
                    );
                    continue;
                }
            },
            Turn::Read(flight) => flight,
        };
        let read = match front(store, retry, object, object_len).await {
            Ok(read) => Arc::<[u8]>::from(read),
            Err(refusal) => {
                flight.land(Err(refusal.clone()));
                return Err(refusal.into());
            }
        };
        let outline = ContainerOutline::open(&read, key);
        if outline.is_ok() {
            memory.remember_front(container_id, Arc::clone(&read));
        }
        // The bytes as they arrived, opened or not: a waiter opens them under
        // its own key, and meets the same refusal this did if they do not.
        flight.land(Ok(read));
        return Ok((outline?, Origin::Read));
    }
}

/// Whether an extent of the object lies inside what the catalog records the
/// object's length to be, or the refusal a header lying about its own lengths
/// earns.
///
/// `Truncated` is exactly that claim: the object ends before the lengths its
/// header declares. It goes to the inner channel, the Library's, because the
/// length it is held against is the committed one (spec: FM-15) and no second
/// request would make the object any longer.
pub(super) fn within_object(
    extent: &Range<u64>,
    object_len: u64,
) -> std::result::Result<(), FormatError> {
    if extent.end > object_len {
        return Err(FormatError::Truncated);
    }
    Ok(())
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
