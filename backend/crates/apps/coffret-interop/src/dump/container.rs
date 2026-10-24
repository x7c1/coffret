//! A Container's meta section, opened with the key a Keyring replica wraps for
//! it.

use std::path::Path;

use coffret_format::{
    decode_control_object, decode_keyring, unwrap_container_key, ContainerOutline, Header, Purpose,
    PurposeKey,
};
use coffret_model::{
    ContainerId, ContainerKey, ContainerKeyStatus, ControlObjectKind, EntryMetadata, MasterKey,
};
use serde_json::json;

use crate::{hex, Error, Result, Step};

/// The Container ID the object's plaintext header names, where the object is
/// stored under that Container's own name (spec: FM-2, FM-3).
pub(super) fn container_named(bytes: &[u8], object_name: &str) -> Option<ContainerId> {
    let header = Header::parse(bytes).ok()?;
    (header.container_id.object_name() == object_name).then_some(header.container_id)
}

/// The key `container_id` is opened with, unwrapped from the envelope the
/// Keyring replica at `keyring` holds for it (spec: FM-14, FM-17).
///
/// The replica is opened under the name it is saved as, which is the name it
/// is stored under: the name is part of what authenticates it (spec: FM-12).
pub(super) fn container_key(
    keyring: &Path,
    master_key: &MasterKey,
    container_id: ContainerId,
) -> Result<ContainerKey> {
    let replica = std::fs::read(keyring).map_err(Error::read(keyring))?;
    let name = keyring
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::KeyringNameNotUnicode {
            path: keyring.to_path_buf(),
        })?;
    let key = PurposeKey::derive(
        master_key,
        Purpose::of_control_object(ControlObjectKind::Keyring),
    );
    let opened =
        decode_control_object(&replica, name, &key).map_err(|cause| Error::KeyringUnopened {
            name: name.to_owned(),
            cause,
        })?;
    let key_table = decode_keyring(&opened.payload).map_err(Error::format(Step::ReadingKeyring))?;
    let element = key_table
        .elements()
        .iter()
        .find(|element| element.container_id == container_id)
        .ok_or(Error::NothingForContainer { container_id })?;
    match &element.key {
        ContainerKeyStatus::Envelope(envelope) => unwrap_container_key(
            &PurposeKey::derive(master_key, Purpose::ContainerWrap),
            &container_id,
            envelope,
        )
        .map_err(Error::format(Step::UnwrappingContainerKey)),
        ContainerKeyStatus::KeyLost => Err(Error::ContainerKeyLost { container_id }),
    }
}

/// A Container's meta section: its kind, its padding and its entry table, read
/// through the outline a reader opens before any chunk (spec: FM-2, FM-9).
pub(super) fn container(bytes: &[u8], key: &ContainerKey) -> Result<serde_json::Value> {
    let header = bytes
        .get(..Header::LEN)
        .ok_or(Error::ShorterThanHeader { len: bytes.len() })?;
    let prefix_len = ContainerOutline::prefix_len(header)
        .map_err(Error::format(Step::ReadingContainerHeader))?;
    let prefix = usize::try_from(prefix_len)
        .ok()
        .and_then(|len| bytes.get(..len))
        .ok_or(Error::ShorterThanMetaSection {
            len: bytes.len(),
            declared: prefix_len,
        })?;
    let outline =
        ContainerOutline::open(prefix, key).map_err(Error::format(Step::OpeningMetaSection))?;
    Ok(json!({
        "container_id": outline.container_id().to_hex(),
        "kind": format!("{:?}", outline.kind()),
        "chunk_size": outline.chunk_size().get(),
        "pad_len": outline.pad_len(),
        "entries": outline.entries().iter().map(entry).collect::<Vec<_>>(),
    }))
}

/// One row of the entry table.
fn entry(metadata: &EntryMetadata) -> serde_json::Value {
    json!({
        "path": metadata.path.as_str(),
        "captured_path": metadata.captured_path.as_str(),
        "offset": metadata.extent.offset(),
        "size": metadata.extent.size(),
        "mtime": metadata.mtime.as_unix_seconds(),
        "btime": metadata.btime.map(|btime| btime.as_unix_seconds()),
        "hash": hex::encode(metadata.hash.as_bytes()),
        "derived_from": metadata.derived_from.as_ref().map(|derived| json!({
            "container_id": derived.container_id.to_hex(),
            "path": derived.path.as_str(),
        })),
        "mime": metadata.mime,
    })
}
