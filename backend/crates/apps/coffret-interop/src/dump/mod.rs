//! Printing one object fetched from a Library: a control object's payload with
//! the field names the field-number registry gives its field numbers, or a
//! Container's meta section.
//!
//! A stored payload carries its fields under field numbers (spec: FM-9,
//! FM-15, FM-16, FM-17), which makes a raw dump a list of numbers to look up by
//! hand. This opens an object fetched from a Library — with the Master Key its
//! Recovery Code carries — and prints the payload as JSON keyed by name, for a
//! person checking what a device actually wrote.
//!
//! A Container is opened by a key of its own, which the Master Key does not
//! derive: it is wrapped in the Keyring (spec: FM-14, FM-17). So a Container is
//! rendered only where a Keyring replica is handed over beside it, and what is
//! rendered is its meta section — kind, padding and entry table (spec: FM-9) —
//! and never its content.

use std::path::Path;

use anyhow::{bail, Context, Result};
use coffret_format::{decode_control_object, Purpose, PurposeKey, RecoveryCode};
use coffret_model::ControlObjectKind;

mod container;
use container::{container, container_key, container_named};

mod control_object;
use control_object::control_object;

/// Opens the object at `object`, stored under `object_name`, and returns what
/// it holds as JSON: a control object's payload keyed by field name, or — given
/// `keyring`, a Keyring replica saved under its own object name — a
/// Container's meta section.
///
/// The kind of a control object rides in the authenticated header, which only
/// the kind's own key opens, so each kind's key is tried in turn and the one
/// that opens the object says which kind it is. Where none does, the object is
/// read as a Container: its header is plaintext and names its Container ID
/// (spec: FM-2), and the Keyring is what holds the key it is opened with.
pub fn dump(
    object: &Path,
    object_name: &str,
    recovery_code: &Path,
    keyring: Option<&Path>,
) -> Result<String> {
    let bytes = std::fs::read(object).context("reading the object")?;
    let code = std::fs::read_to_string(recovery_code).context("reading the Recovery Code")?;
    let code = RecoveryCode::parse(&code).context("parsing the Recovery Code")?;

    let opened = ControlObjectKind::ALL.iter().find_map(|kind| {
        let key = PurposeKey::derive(code.master_key(), Purpose::of_control_object(*kind));
        decode_control_object(&bytes, object_name, &key).ok()
    });
    let rendered = match (opened, container_named(&bytes, object_name)) {
        (Some(opened), _) => control_object(&opened)?,
        (None, Some(container_id)) => match keyring {
            Some(keyring) => {
                let key = container_key(keyring, code.master_key(), container_id)?;
                container(&bytes, &key)?
            }
            None => {
                bail!("{object_name} is a Container; pass --keyring to render its meta section")
            }
        },
        (None, None) => {
            bail!("no control-object kind's key opens this object under this name")
        }
    };
    serde_json::to_string_pretty(&rendered).context("rendering the object")
}

#[cfg(test)]
mod tests;
