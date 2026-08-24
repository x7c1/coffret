use coffret_format::{decode_control_object, ControlHeader, DecodedControlObject};
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
pub(super) async fn read(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    keys: &ControlKeys,
    name: &ControlObjectName,
    object: &ObjectRef,
) -> CommitResult<DecodedControlObject> {
    let bytes = fetch(store, retry, object).await?;
    let header = ControlHeader::parse(&bytes)?;
    Ok(decode_control_object(
        &bytes,
        &name.to_string(),
        keys.of_kind(header.kind),
    )?)
}

/// Drains one object into memory.
///
/// Control objects are the only things this crate reads whole: a Container goes
/// through [`ByteStream::into_reader`](crate::ByteStream::into_reader) instead,
/// because it is as large as the files it carries.
pub(super) async fn fetch(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    object: &ObjectRef,
) -> Result<Vec<u8>> {
    let stream: ByteStream = retry.run("get", || store.get(object, None)).await?;
    stream.into_bytes().await
}

/// Reads back only the plaintext header of a control object.
///
/// An Index Snapshot is a multi-megabyte object and the two questions asked of
/// one before a commit — is the head still there, and is it still the head it
/// was — are both answered by the first 44 bytes (spec: FM-11, CP-16). A
/// provider that serves more of the object than the range asked for is harmless
/// here: the header is parsed off the front either way.
pub(super) async fn fetch_header(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    object: &ObjectRef,
) -> CommitResult<ControlHeader> {
    let range = 0..ControlHeader::LEN as u64;
    let stream: ByteStream = retry
        .run("get", || store.get(object, Some(range.clone())))
        .await?;
    Ok(ControlHeader::parse(&stream.into_bytes().await?)?)
}
