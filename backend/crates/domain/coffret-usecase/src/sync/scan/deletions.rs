use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use coffret_model::{EntryLocation, EntryPath};

use crate::device_state::{LocalEntry, LocalEntryState, Mapping};
use crate::index::Index;
use crate::local_scan::{kept_by_system, RootState, ScanScope, SourceFile, WalkedRoot};
use crate::sync::survey::Survey;
use crate::sync::sync_error::SyncResult;

/// A file this device materialized that the scan found gone, at a path the
/// Library still holds an Entry at: a deletion this device witnessed, unless it
/// pairs with a file that appeared as a rename (spec: EP-10).
pub(super) struct Gone {
    /// This device's record of the file.
    pub(super) local: LocalEntry,
    /// The Entry standing at the path the file stood at.
    pub(super) location: EntryLocation,
    /// The local root of the mapping the file stood under, which is the one
    /// root a file it pairs with has to stand under too (spec: EP-9).
    pub(super) root: PathBuf,
}

/// What the scan made of this device's records under the roots it walked.
pub(super) struct Deletions {
    /// The files gone from disk whose Entries are still in the Library, in
    /// Entry Path order.
    pub(super) gone: Vec<Gone>,
    /// The records whose files the walk found where they stand.
    pub(super) kept: Vec<LocalEntry>,
}

/// The files this device materialized that are no longer on disk (spec: EP-10).
///
/// Only rows the port reports under a mapped prefix are considered, so a
/// subtree this device does not map is not a subtree of deletions. A row whose
/// path no walk found is a file the device put there and lost; one path can be
/// reported by two mappings, so the findings are deduplicated by path.
///
/// A mapping whose root the device cannot vouch for produces no deletion at all
/// (spec: EP-12). Nothing under such a root was walked, so every row beneath it
/// would be absent from `found` for a reason that has nothing to do with the
/// files: an unplugged disk must never read as the user having emptied the
/// folder.
///
/// That has to be applied as the same partition the walk applies, and not merely
/// as skipping the unavailable mapping's own pass. A top-level mapping represents
/// its subtree and the Library-root mapping represents *the remainder*
/// (spec: EP-9), so the root mapping's records are read as the remainder too
/// ([`reads`]): a row under a prefix another mapping stands for belongs to that
/// mapping, and if that mapping is unavailable the row is nobody's evidence.
///
/// A row whose Entry has left the Library is no deletion to report: the Library
/// no longer holds what the file was a copy of, and the disk no longer holds
/// the file. Whether the sync discarded it or the person moved or removed it
/// themselves, nothing is left for a finding to be about, so the row goes into
/// [`Survey::forgotten`] instead (spec: EP-15).
///
/// A row at a path the walk leaves out because an operating system keeps the
/// name — an Entry carried in before that rule — is neither: the walk never
/// looked for its file, so it is not read as gone (spec: EP-17).
///
/// A gone file whose Entry is still current is handed back rather than
/// reported here: whether it is a deletion or one half of a rename is decided
/// once the files that appeared are known (spec: EP-10). The records whose
/// files are still there are handed back too, because a file holding the same
/// content as a gone one makes the pair ambiguous.
///
/// A row standing in a folder out of `scope` is neither (spec: EP-10): the
/// walk did not look there, so its file's absence from `found` says nothing
/// about it. So a scope of folders is read folder by folder, each folder's own
/// files ([`reads`]), rather than every record under a mapping read for the
/// few in scope.
pub(super) async fn deletions(
    index: &dyn Index,
    mappings: &[Mapping],
    roots: &[WalkedRoot],
    found: &BTreeMap<EntryPath, SourceFile>,
    scope: &ScanScope,
    survey: &mut Survey,
) -> SyncResult<Deletions> {
    // Every mapping's prefix, available or not, and walked or not: the same
    // set the walk holds its root mapping to (spec: EP-9, EP-12).
    let represented_elsewhere = Mapping::represented_prefixes(mappings);

    let mut gone: BTreeMap<EntryPath, (LocalEntry, PathBuf)> = BTreeMap::new();
    let mut kept: BTreeMap<EntryPath, LocalEntry> = BTreeMap::new();
    for root in roots {
        if matches!(root.state, RootState::Unavailable(_)) {
            continue;
        }
        let prefix = root.mapping.prefix.as_ref();
        for read in reads(scope, prefix, &represented_elsewhere) {
            let present = match read {
                Read::Under(prefix) => index.present_under(Some(prefix)).await?,
                Read::Remainder => index.present_outside(&represented_elsewhere).await?,
                Read::In(folder) => index.present_in(folder).await?,
                Read::At(path) => index
                    .local_entry_at(path)
                    .await?
                    .into_iter()
                    .filter(|local| {
                        local.state == LocalEntryState::Present && !local.awaits_relocation()
                    })
                    .collect(),
            };
            for local in present {
                // Every read is bounded to what its mapping stands for, save one:
                // the files standing directly at the Library root, where a file
                // can bear the very name a top-level mapping stands for. Its path
                // is that mapping's prefix, so it is that mapping's to account
                // for, not the root mapping's (spec: EP-9).
                if prefix.is_none()
                    && represented_elsewhere.contains(local.observation.path.top_level())
                {
                    continue;
                }
                // A path a scan leaves out because an operating system keeps the
                // name: its file is never found, so its absence is no deletion,
                // and the record stays as it is (spec: EP-17).
                if kept_by_system(prefix, &local.observation.path) {
                    continue;
                }
                // A row in a folder the walk did not look in is no evidence of
                // anything (spec: EP-10). The reads already stop at the scope;
                // this holds the rule here too, whatever an adapter answers.
                if !scope.covers(&local.observation.path) {
                    continue;
                }
                let path = local.observation.path.clone();
                if found.contains_key(&path) {
                    kept.entry(path).or_insert(local);
                } else {
                    gone.entry(path)
                        .or_insert_with(|| (local, root.mapping.local_root.clone()));
                }
            }
        }
    }
    let mut current = Vec::new();
    for (path, (local, root)) in gone {
        match index.entry_at(&path).await? {
            Some(location) => current.push(Gone {
                local,
                location,
                root,
            }),
            None => survey.forgotten.push(path),
        }
    }
    Ok(Deletions {
        gone: current,
        kept: kept.into_values().collect(),
    })
}

/// One read of this device's present records, bounded to what a mapping
/// stands for within a scan's scope.
#[derive(Debug, PartialEq, Eq)]
enum Read<'a> {
    /// Everything under a top-level mapping's prefix.
    Under(&'a EntryPath),
    /// Everything outside the prefixes other mappings stand for: the
    /// remainder a Library-root mapping represents (spec: EP-9).
    Remainder,
    /// The files standing directly in one folder — `None` is the Library
    /// root — which is all a scope of folders looks at there (spec: EP-10).
    In(Option<&'a EntryPath>),
    /// The one file standing at a top-level mapping's own prefix: directly at
    /// the Library root, bearing the name the mapping stands for. The walk never
    /// finds it, the root mapping leaves it to this one (spec: EP-9), and no
    /// folder's own files hold it but the root's, so a scope holding both the
    /// root and the mapping's folder reads it here.
    At(&'a EntryPath),
}

/// What one mapping's records are read as: the subtree a top-level mapping
/// stands for, or the remainder the Library-root mapping stands for, where the
/// scope is every mapped folder; and otherwise the files directly in each
/// folder in scope that the mapping reaches, each folder read on its own rather
/// than the whole mapping read and the rest thrown away (spec: EP-9, EP-10).
///
/// So the Library-root mapping never reads every present record of the device:
/// the rows under another mapping's prefix are left out of the read itself, and
/// a scope holding the Library root reads the files standing directly there.
fn reads<'a>(
    scope: &'a ScanScope,
    prefix: Option<&'a EntryPath>,
    elsewhere: &BTreeSet<&str>,
) -> Vec<Read<'a>> {
    let ScanScope::Folders { root, folders } = scope else {
        return vec![prefix.map_or(Read::Remainder, Read::Under)];
    };
    let own = match prefix {
        None => root.then_some(Read::In(None)),
        Some(prefix) => (*root && folders.contains(prefix)).then_some(Read::At(prefix)),
    };
    own.into_iter()
        .chain(
            folders
                .iter()
                .filter(|folder| match prefix {
                    Some(prefix) => folder.is_under(prefix),
                    None => !elsewhere.contains(folder.top_level()),
                })
                .map(|folder| Read::In(Some(folder))),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{reads, Read};
    use crate::entry_paths::entry_path;
    use crate::local_scan::ScanScope;

    /// The Library-root mapping reads the remainder, or the files directly in
    /// each folder in scope, and never every present row of the device
    /// (spec: EP-9, EP-10).
    #[test]
    fn the_root_mapping_reads_only_what_it_stands_for() {
        let elsewhere = BTreeSet::from(["albums"]);
        assert_eq!(
            reads(&ScanScope::Mappings, None, &elsewhere),
            [Read::Remainder]
        );

        let albums = entry_path("albums");
        assert_eq!(
            reads(&ScanScope::Mappings, Some(&albums), &elsewhere),
            [Read::Under(&albums)]
        );

        let scope = ScanScope::folders_of(&[
            entry_path("top.jpg"),
            entry_path("albums/2026/spring.jpg"),
            entry_path("misc/a.jpg"),
            entry_path("misc/deep/b.jpg"),
        ]);
        let misc = entry_path("misc");
        let deep = entry_path("misc/deep");
        assert_eq!(
            reads(&scope, None, &elsewhere),
            [Read::In(None), Read::In(Some(&misc)), Read::In(Some(&deep))],
            "the root's own files and each folder in scope outside other mappings",
        );
        assert_eq!(
            reads(&scope, Some(&albums), &elsewhere),
            [Read::In(Some(&entry_path("albums/2026")))]
        );

        // A file at the Library root bearing a top-level mapping's name is
        // that mapping's, read on its own once the scope holds both the root
        // and the mapping's folder.
        let both = ScanScope::folders_of(&[entry_path("top.jpg"), entry_path("albums/a.jpg")]);
        assert_eq!(
            reads(&both, Some(&albums), &elsewhere),
            [Read::At(&albums), Read::In(Some(&albums))]
        );
    }
}
