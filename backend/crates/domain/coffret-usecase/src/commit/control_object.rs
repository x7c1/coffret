use coffret_format::{
    decode_control_object, max_control_object_len_at, ControlHeader, DecodedControlObject,
};
use coffret_model::{ControlObjectName, ObjectRef};

use crate::byte_stream::ByteStream;
use crate::commit::commit_error::CommitResult;
use crate::commit::control_keys::ControlKeys;
use crate::error::Result;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// Reads one control object back and opens it.
///
/// The kind is read off the plaintext header first, and only then is a key
/// chosen: one name form covers the whole control-head chain, so what is at
/// `head-<generation>` may be a Journal record or the Index Snapshot that
/// activated an epoch, and the two are sealed under different purpose keys
/// (spec: FM-11, FM-12, KD-4). Every check the framing makes — the name
/// admitting the kind, the generation and replica position agreeing with it —
/// happens inside [`decode_control_object`] on those same plaintext bytes.
///
/// How many bytes are taken in before any of that is decided by the name and the
/// format's ceilings, not by what Storage says the object weighs — see
/// [`fetch`].
pub(super) async fn read(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    keys: &ControlKeys,
    name: &ControlObjectName,
    object: &ObjectRef,
) -> CommitResult<DecodedControlObject> {
    let bytes = fetch(store, retry, name, object).await?;
    open(keys, name, &bytes)
}

/// Opens the bytes of one control object that has already arrived.
///
/// Split from [`read`] so that a caller who has to tell a fetch that failed from
/// an object that came back and was rejected can drive the two halves itself —
/// [`keyring`](super::keyring) does. The checks are the same ones [`read`]
/// makes, because they are the same call.
pub(super) fn open(
    keys: &ControlKeys,
    name: &ControlObjectName,
    bytes: &[u8],
) -> CommitResult<DecodedControlObject> {
    let header = ControlHeader::parse(bytes)?;
    Ok(decode_control_object(
        bytes,
        &name.to_string(),
        keys.of_kind(header.kind),
    )?)
}

/// Drains one object into memory, up to what an object at that name may be.
///
/// Control objects are the only things this crate reads whole: a Container goes
/// through [`ByteStream::into_reader`](crate::ByteStream::into_reader) instead,
/// because it is as large as the files it carries. Reading something whole means
/// spending memory on a size nothing has authenticated yet, so the spending is
/// bounded first.
///
/// The bound comes from the name, because that is all there is to go on before
/// the answer arrives: the kind rides in the object's header. A name admits one
/// or two kinds (spec: FM-12) and each kind's payload has a size its schema can
/// account for, so `max_control_object_len_at` is the largest object that name
/// could legitimately lead to. Anything past it is refused before a byte is
/// read, and an answer that runs past its own declared length stops at that
/// length rather than growing.
pub(super) async fn fetch(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    name: &ControlObjectName,
    object: &ObjectRef,
) -> Result<Vec<u8>> {
    let stream: ByteStream = retry.run("get", || store.get(object, None)).await?;
    stream
        .into_bytes_within(max_control_object_len_at(name))
        .await
}

/// Reads back only the plaintext header of a control object.
///
/// An Index Snapshot is a multi-megabyte object and the two questions asked of
/// one before a commit — is the head still there, and is it still the head it
/// was — are both answered by the first 44 bytes (spec: FM-11, CP-16). A
/// provider that serves more of the object than the range asked for is harmless
/// here, and is *kept* harmless: the header is parsed off the front either way,
/// and only the front is ever taken in, so ignoring the range costs this device
/// 44 bytes rather than the Snapshot.
pub(super) async fn fetch_header(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    object: &ObjectRef,
) -> CommitResult<ControlHeader> {
    let front = ControlHeader::LEN as u64;
    let stream: ByteStream = retry
        .run("get", || store.get(object, Some(0..front)))
        .await?;
    Ok(ControlHeader::parse(&stream.collect_front(front).await?)?)
}
