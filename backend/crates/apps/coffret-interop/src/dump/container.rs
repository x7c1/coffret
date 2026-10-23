//! A Container's meta section, opened with the key a Keyring replica wraps for
//! it.

use std::path::Path;

use anyhow::{bail, Context, Result};
use coffret_format::{
    decode_control_object, decode_keyring, unwrap_container_key, ContainerOutline, Header, Purpose,
    PurposeKey,
};
use coffret_model::{
    ContainerId, ContainerKey, ContainerKeyStatus, ControlObjectKind, EntryMetadata, MasterKey,
};
use serde_json::json;

use crate::hex;

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
    let replica = std::fs::read(keyring).context("reading the Keyring replica")?;
    let name = keyring
        .file_name()
        .and_then(|name| name.to_str())
        .context("the Keyring replica is saved under a name that is not Unicode")?;
    let key = PurposeKey::derive(
        master_key,
        Purpose::of_control_object(ControlObjectKind::Keyring),
    );
    let opened = decode_control_object(&replica, name, &key).with_context(|| {
        format!("opening the Keyring replica; it has to be saved under its own object name, not {name:?}")
    })?;
    let key_table = decode_keyring(&opened.payload).context("reading the Keyring")?;
    let element = key_table
        .elements()
        .iter()
        .find(|element| element.container_id == container_id)
        .with_context(|| {
            format!(
                "the Keyring holds nothing for Container {}",
                container_id.to_hex()
            )
        })?;
    match &element.key {
        ContainerKeyStatus::Envelope(envelope) => unwrap_container_key(
            &PurposeKey::derive(master_key, Purpose::ContainerWrap),
            &container_id,
            envelope,
        )
        .context("unwrapping the Container Key"),
        ContainerKeyStatus::KeyLost => bail!(
            "the Keyring records the key of Container {} as lost",
            container_id.to_hex()
        ),
    }
}

/// A Container's meta section: its kind, its padding and its entry table, read
/// through the outline a reader opens before any chunk (spec: FM-2, FM-9).
pub(super) fn container(bytes: &[u8], key: &ContainerKey) -> Result<serde_json::Value> {
    let header = bytes
        .get(..Header::LEN)
        .context("the object is shorter than a Container header")?;
    let prefix_len =
        ContainerOutline::prefix_len(header).context("reading the Container header")?;
    let prefix = usize::try_from(prefix_len)
        .ok()
        .and_then(|len| bytes.get(..len))
        .context("the object is shorter than the meta section its header declares")?;
    let outline = ContainerOutline::open(prefix, key).context("opening the meta section")?;
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
