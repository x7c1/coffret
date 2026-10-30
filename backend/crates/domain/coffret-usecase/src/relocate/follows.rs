use coffret_model::EntryPath;

use crate::device_state::Mapping;
use crate::fetch::{reaching, relative_below};

/// Whether this device's file at `from` can follow its Entry to `to` with one
/// rename, as far as the mappings tell before the disk is asked: a mapped root
/// reaches `to`, and it is the root `from` stands under or one that stands on
/// the same filesystem (spec: EP-10, EP-12).
///
/// What a preview of a rename counts by. A relocation asks the same of the
/// mappings, and then the disk: a rename the filesystems refuse as crossing
/// from one to another is a file that cannot follow either.
pub(crate) fn follows(mappings: &[Mapping], from: &EntryPath, to: &EntryPath) -> bool {
    let Some(source) = reaching(mappings, from) else {
        return false;
    };
    match reaching(mappings, to).filter(|target| relative_below(target, to).is_some()) {
        None => false,
        Some(target) => target.prefix == source.prefix || !another_filesystem(source, target),
    }
}

/// Whether the roots of two mappings stand on two filesystems, as far as the
/// scans that stamped them recorded (spec: EP-12).
///
/// Only where both filesystem identities are recorded and differ: a root no
/// scan has stamped yet is tried with the rename, and a rename the two
/// filesystems refuse says the same thing then (spec: EP-10).
pub(super) fn another_filesystem(one: &Mapping, other: &Mapping) -> bool {
    matches!(
        (&one.root_identity, &other.root_identity),
        (Some(one), Some(other)) if one != other
    )
}
