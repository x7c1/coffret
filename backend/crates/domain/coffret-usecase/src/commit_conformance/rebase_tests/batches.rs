use std::collections::BTreeMap;

use coffret_model::{ContainerKind, EntryNumber, Generation, Mtime};

use crate::commit::{
    CommitError, CommitOutcome, EntryRef, PendingChange, PendingRename, PreparedAddition,
    PreparedBatch, RebaseRule,
};
use crate::commit_conformance::fixtures::{container_id, path, prepared};
use crate::device_state::{DeviceTime, LocalObservation};

/// The Entry numbered `number` in Container `seed`.
pub(super) fn entry(seed: u8, number: u64) -> EntryRef {
    EntryRef::new(
        container_id(seed),
        EntryNumber::new(number).expect("a small number is an entry number"),
    )
}

/// A rename of `entry`, prepared against its standing at `from`, to `to`.
pub(super) fn rename(entry: EntryRef, from: &str, to: &str) -> PendingRename {
    PendingRename {
        entry,
        from: path(from),
        path: path(to),
    }
}

/// A batch renaming `renames` and nothing else.
pub(super) fn renaming(renames: Vec<PendingRename>) -> PreparedBatch {
    PreparedBatch::default().renaming(renames)
}

/// Container `seed` of `kind` at `paths`, each Entry of which takes the place
/// of the one `supersedes` pairs with its number.
pub(super) fn superseding(
    seed: u8,
    kind: ContainerKind,
    paths: &[&str],
    supersedes: &[(u64, EntryRef)],
) -> PreparedAddition {
    prepared(seed, kind, paths).superseding(
        supersedes
            .iter()
            .map(|(number, superseded)| {
                (
                    EntryNumber::new(*number).expect("a small number is an entry number"),
                    *superseded,
                )
            })
            .collect::<BTreeMap<_, _>>(),
    )
}

/// A sync's update of the one-file Container `old` at `text`: Container `new`
/// in its place, the file on disk recorded as materialized.
pub(super) fn updating(new: u8, old: u8, text: &str) -> PreparedBatch {
    PreparedBatch::adding(vec![superseding(
        new,
        ContainerKind::OneFile,
        &[text],
        &[(0, entry(old, 0))],
    )])
    .removing(vec![container_id(old)])
    .materializing(vec![observed(text, entry(new, 0))])
}

/// A deletion's rebuild of Pack `old` around the Entries it keeps: Pack `new`
/// holding `kept`, each one carried forward from the Entry `old` numbered as
/// given.
pub(super) fn rebuilding(new: u8, old: u8, kept: &[(&str, u64)]) -> PreparedBatch {
    let paths: Vec<&str> = kept.iter().map(|(text, _)| *text).collect();
    let supersedes: Vec<(u64, EntryRef)> = kept
        .iter()
        .enumerate()
        .map(|(position, (_, number))| (position as u64, entry(old, *number)))
        .collect();
    PreparedBatch::adding(vec![superseding(
        new,
        ContainerKind::Pack,
        &paths,
        &supersedes,
    )])
    .removing(vec![container_id(old)])
}

/// What this device sees of its own file at `text`, a copy of `entry`.
pub(super) fn observed(text: &str, entry: EntryRef) -> LocalObservation {
    LocalObservation {
        path: path(text),
        size: 100,
        mtime: Mtime::from_unix_seconds(1_700_000_100),
        at: DeviceTime::from_unix_seconds(1_700_000_200),
        entry,
        hash: None,
    }
}

/// The refusal a conflicting rebase answers with, taken apart for the case.
pub(super) fn conflict(
    result: Result<CommitOutcome, CommitError>,
) -> (PendingChange, Generation, RebaseRule, String) {
    match result {
        Err(CommitError::RebaseConflict {
            pending,
            record,
            rule,
            path,
        }) => (pending, record, rule, path.as_str().to_owned()),
        other => panic!("expected a rebase conflict, got {other:?}"),
    }
}

/// Whether `pending` is the change named.
pub(super) fn is(pending: PendingChange, expected: PendingChange) -> bool {
    match (pending, expected) {
        (PendingChange::Addition(found), PendingChange::Addition(wanted))
        | (PendingChange::Rename(found), PendingChange::Rename(wanted)) => found == wanted,
        _ => false,
    }
}
