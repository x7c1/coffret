use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

use coffret_model::{EntryLocation, EntryPath, EntryRename};

use crate::catalog_renames::{check_rename_set, Named};
use crate::commit::PendingRename;
use crate::entry_ref::EntryRef;
use crate::index::Index;
use crate::index_error::IndexError;
use crate::rename::rename_error::RenameResult;
use crate::rename::rename_refusal::RenameRefusal;
use crate::rename::renamed_path::RenamedPath;

/// What a rename will commit, worked out from the catalog alone.
pub(super) struct Plan {
    /// One per Entry renamed, as the batch carries them.
    pub(super) renames: Vec<PendingRename>,
    /// The same, as paths, ordered by the path each leaves.
    pub(super) renamed: Vec<RenamedPath>,
    /// Whether the source named Entries under it rather than one Entry alone.
    pub(super) folder: bool,
}

/// Refuses what is refused whatever the Library holds: the same path on both
/// sides, and either side in the namespace reserved for derived Entries
/// (spec: FM-15).
pub(super) fn refuse_outright(from: &EntryPath, to: &EntryPath) -> Result<(), RenameRefusal> {
    if from == to {
        return Err(RenameRefusal::SamePath);
    }
    for side in [from, to] {
        if side.is_derived() {
            return Err(RenameRefusal::ReservedNamespace { path: side.clone() });
        }
    }
    Ok(())
}

/// Works out which Entries renaming `from` to `to` renames, and refuses a
/// rename the catalog does not admit (spec: EP-5, FM-15).
///
/// `from` names the Entry standing at it, if any, and every current Entry
/// under it — the subtree [`EntryPath::is_under`] covers — and each is renamed
/// by its own element, the rest of its path kept under `to`. The elements are
/// enumerated here, against the catalog as it stands, and are all the rename
/// ever names: an Entry another device adds under `from` before the commit is
/// not swept in (spec: EP-7).
///
/// The state the rename leaves is held to three questions about every
/// destination, against the Entries it leaves where they are: none of them
/// stands at it (spec: EP-5); none stands at a folder above it, which would
/// then be a file and a folder at once; and none stands under it, for the same
/// reason the other way round. Then to every rule FM-15's rename-set check
/// holds a record to, so that a record this plan produces is one every catalog
/// applies (spec: RV-5).
pub(super) async fn plan(
    index: &dyn Index,
    from: &EntryPath,
    to: &EntryPath,
) -> RenameResult<Plan> {
    let current = index.entries_under(None).await?;
    let (renaming, staying): (Vec<&EntryLocation>, Vec<&EntryLocation>) = current
        .iter()
        .partition(|location| location.path().is_under(from));
    if renaming.is_empty() {
        return Err(RenameRefusal::NothingAt { path: from.clone() }.into());
    }
    let folder = renaming.iter().any(|location| location.path() != from);
    if folder && to.is_under(from) {
        return Err(RenameRefusal::IntoItself.into());
    }

    let mut renamed: Vec<(EntryRef, RenamedPath)> = renaming
        .iter()
        .map(|location| {
            let path = location
                .path()
                .renamed_from(from, to)
                .expect("an Entry under the source is renamed with it");
            (
                EntryRef::of(location),
                RenamedPath {
                    from: location.path().clone(),
                    to: path,
                },
            )
        })
        .collect();
    renamed.sort_by(|left, right| left.1.from.cmp(&right.1.from));

    let staying: BTreeSet<&str> = staying
        .iter()
        .map(|location| location.path().as_str())
        .collect();
    for (_, one) in &renamed {
        if let Some(refusal) = clash(&staying, &one.to) {
            return Err(refusal.into());
        }
    }

    check_rules(index, &current, &renamed).await?;

    Ok(Plan {
        renames: renamed
            .iter()
            .map(|(entry, one)| PendingRename {
                entry: *entry,
                from: one.from.clone(),
                path: one.to.clone(),
            })
            .collect(),
        renamed: renamed.into_iter().map(|(_, one)| one).collect(),
        folder,
    })
}

/// Whether `to` would clash with an Entry the rename leaves where it is: stand
/// at the same path, stand under a path such an Entry stands at, or have such
/// an Entry under it.
fn clash(staying: &BTreeSet<&str>, to: &EntryPath) -> Option<RenameRefusal> {
    if staying.contains(to.as_str()) {
        return Some(RenameRefusal::DestinationTaken { path: to.clone() });
    }
    let mut above = to.parent();
    while let Some(folder) = above {
        if staying.contains(folder.as_str()) {
            return Some(RenameRefusal::FileAndFolder { path: folder });
        }
        above = folder.parent();
    }
    // Every path under `to/` sorts from `to/` onwards and before anything that
    // does not start with it, so the first one at or after it is the one to ask.
    let below = format!("{to}/");
    let first = staying
        .range::<str, _>((Bound::Included(below.as_str()), Bound::Unbounded))
        .next();
    if first.is_some_and(|path| path.starts_with(&below)) {
        return Some(RenameRefusal::FileAndFolder { path: to.clone() });
    }
    None
}

/// Holds the renames to FM-15's rename-set check against the catalog the plan
/// read, the check a commit asks again of its batch and every catalog asks of
/// the record (spec: FM-15, RV-5).
///
/// The plan's own questions answer most of what it asks, so what reaches a
/// refusal here is a rule they do not cover — a derived Entry stored outside
/// the reserved namespace, above all — and it is the rename's refusal rather
/// than an error inside the commit.
async fn check_rules(
    index: &dyn Index,
    current: &[EntryLocation],
    renamed: &[(EntryRef, RenamedPath)],
) -> RenameResult<()> {
    let renames = renamed
        .iter()
        .map(|(entry, one)| {
            EntryRename::new(entry.container_id, entry.entry_number, one.to.clone())
                .expect("refuse_outright kept the destination out of the derived namespace")
        })
        .collect::<Vec<_>>();
    let containers: BTreeSet<_> = index
        .containers_under(None)
        .await?
        .into_iter()
        .map(|container| container.id)
        .collect();
    let by_number: BTreeMap<EntryRef, &EntryLocation> = current
        .iter()
        .map(|location| (EntryRef::of(location), location))
        .collect();
    let standing: BTreeMap<&EntryPath, EntryRef> = current
        .iter()
        .map(|location| (location.path(), EntryRef::of(location)))
        .collect();

    check_rename_set(
        &renames,
        &[],
        &[],
        |container_id, entry_number| {
            Ok(
                match by_number.get(&EntryRef::new(container_id, entry_number)) {
                    Some(location) => Named::Entry(Box::new((*location).clone())),
                    None if containers.contains(&container_id) => Named::NoEntry,
                    None => Named::NoContainer,
                },
            )
        },
        |path| {
            Ok(standing
                .get(path)
                .map(|entry| (entry.container_id, entry.entry_number)))
        },
    )
    .map_err(|refusal| match refusal {
        IndexError::RenameRefused { rule, .. } => RenameRefusal::Rule(rule).into(),
        other => other.into(),
    })
    .map(|_| ())
}
