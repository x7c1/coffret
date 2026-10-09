use std::ops::Range;

use coffret_format::{ContainerOutline, Error as FormatError, Header};
use coffret_model::{ContainerKey, ObjectRef};

use crate::error::Result;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// The header and the meta section, read off the front of the object.
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
    key: &ContainerKey,
) -> FetchResult<ContainerOutline> {
    let mut front = ranged(store, retry, object, 0..Header::LEN as u64).await?;
    let front_len = ContainerOutline::prefix_len(&front)?;
    within_object(&(0..front_len), object_len)?;
    let meta = ranged(store, retry, object, Header::LEN as u64..front_len).await?;
    front.extend_from_slice(&meta);
    Ok(ContainerOutline::open(&front, key)?)
}

/// Whether an extent of the object lies inside what the catalog records the
/// object's length to be, or the refusal a header lying about its own lengths
/// earns.
///
/// `Truncated` is exactly that claim: the object ends before the lengths its
/// header declares. It goes to the inner channel, the Library's, because the
/// length it is held against is the committed one (spec: FM-15) and no second
/// request would make the object any longer.
pub(super) fn within_object(extent: &Range<u64>, object_len: u64) -> FetchResult<()> {
    if extent.end > object_len {
        return Err(FetchError::Format(FormatError::Truncated));
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
