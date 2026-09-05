use coffret_model::{
    Btime, CiphertextLenClaim, ContainerId, ContainerKind, ContainerSummary, ControlObjectName,
    DerivedFrom, EntryExtent, EntryLocation, EntryMetadata, EntryPath, Generation, IndexCheckpoint,
    KeyringCommitment, MasterKeyEpoch, Mtime, ObjectRef,
};
use coffret_usecase::IndexResult;
use rusqlite::Row;

use super::columns::{
    container_id, content_hash, entry_path, from_integer, integer, optional_blob, optional_integer,
    optional_text, text,
};
use crate::error::{unreadable, unreadable_model};

/// One row of `containers`.
pub(crate) fn container_summary(row: &Row<'_>) -> IndexResult<ContainerSummary> {
    const OPERATION: &str = "reading a Container";
    Ok(ContainerSummary {
        id: container_id(row, "id", OPERATION)?,
        kind: match text(row, "kind", OPERATION)?.as_str() {
            "one-file" => ContainerKind::OneFile,
            "pack" => ContainerKind::Pack,
            found => return Err(unreadable(OPERATION, "Container kind", found)),
        },
        ciphertext_hash: content_hash(row, "ciphertext_hash", OPERATION)?,
        ciphertext_len: CiphertextLenClaim::new(from_integer(row, "ciphertext_len", OPERATION)?),
        object_ref: optional_text(row, "object_ref", OPERATION)?.map(ObjectRef::new),
    })
}

/// One row of `entries`.
pub(crate) fn entry_location(row: &Row<'_>) -> IndexResult<EntryLocation> {
    const OPERATION: &str = "reading an Entry";
    let derived_container = optional_blob(row, "derived_from_container", OPERATION)?;
    let derived_path = optional_text(row, "derived_from_path", OPERATION)?;
    let derived_from = match (derived_container, derived_path) {
        (Some(container), Some(path)) => Some(DerivedFrom {
            container_id: ContainerId::from_slice(&container)
                .map_err(unreadable_model(OPERATION))?,
            path: EntryPath::stored(path).map_err(unreadable_model(OPERATION))?,
        }),
        (None, None) => None,
        // Half a reference points at nothing, and guessing which half was meant
        // would attach derived data to the wrong parent. Which half is there
        // says whether it is the Container column or the path column that was
        // left unwritten; the path itself stays out of it.
        (Some(_), None) => {
            return Err(unreadable(
                OPERATION,
                "derived-from reference",
                "a Container with no Entry Path",
            ))
        }
        (None, Some(_)) => {
            return Err(unreadable(
                OPERATION,
                "derived-from reference",
                "an Entry Path with no Container",
            ))
        }
    };

    Ok(EntryLocation {
        container_id: container_id(row, "container_id", OPERATION)?,
        entry: EntryMetadata {
            path: entry_path(row, "path", OPERATION)?,
            extent: entry_extent(row, OPERATION)?,
            mtime: Mtime::from_unix_seconds(integer(row, "mtime", OPERATION)?),
            btime: optional_integer(row, "btime", OPERATION)?.map(Btime::from_unix_seconds),
            hash: content_hash(row, "hash", OPERATION)?,
            derived_from,
            mime: optional_text(row, "mime", OPERATION)?,
        },
    })
}

/// The single row of `checkpoint`, and the checkpoint object it was adopted
/// from.
pub(crate) fn checkpoint(
    row: &Row<'_>,
) -> IndexResult<(IndexCheckpoint, Option<ControlObjectName>)> {
    const OPERATION: &str = "reading the checkpoint";
    let replica_count = integer(row, "keyring_replica_count", OPERATION)?;
    // Whether the two generations stand in an order a commit produces is CK-1's
    // rule and the checkpoint's own, so a row that says otherwise makes the
    // catalog unreadable rather than becoming a checkpoint no head ever had.
    let checkpoint = IndexCheckpoint::new(
        MasterKeyEpoch::new(from_integer(row, "master_key_epoch", OPERATION)?)
            .map_err(unreadable_model(OPERATION))?,
        Generation::new(from_integer(row, "head_generation", OPERATION)?),
        Generation::new(from_integer(row, "journal_generation", OPERATION)?),
        optional_text(row, "next_commit_slot", OPERATION)?,
        KeyringCommitment::new(
            Generation::new(from_integer(row, "keyring_generation", OPERATION)?),
            u16::try_from(replica_count).map_err(|_| {
                unreadable(
                    OPERATION,
                    "Keyring replica count",
                    replica_count.to_string(),
                )
            })?,
            &text(row, "keyring_set_digest", OPERATION)?,
        )
        .map_err(unreadable_model(OPERATION))?,
    )
    .map_err(unreadable_model(OPERATION))?;
    let adopted_from = optional_text(row, "adopted_snapshot", OPERATION)?
        .map(|name| ControlObjectName::parse(&name))
        .transpose()
        .map_err(unreadable_model(OPERATION))?;

    Ok((checkpoint, adopted_from))
}

/// Where one row places its Entry in its Container's plaintext stream
/// (spec: FM-9).
///
/// A row whose `offset` and `size` end past what the stream can address places
/// no Entry, so it is a row this build cannot read — the verdict a malformed
/// path in the same row gets, and for the same reason: the file is a cache that
/// can be rebuilt from Storage (spec: RV-5), so saying so is cheaper than
/// handing a fetch a range nothing could ever be read from.
fn entry_extent(row: &Row<'_>, operation: &'static str) -> IndexResult<EntryExtent> {
    EntryExtent::new(
        from_integer(row, "offset", operation)?,
        from_integer(row, "size", operation)?,
    )
    .map_err(unreadable_model(operation))
}
