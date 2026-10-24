use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, ContainerKind, EntryPath};
use tracing::debug;

use crate::device_state::{DeviceTime, LiveSince, Reservation};
use crate::freeze::freeze_error::FreezeResult;
use crate::freeze::freeze_preview::FreezePreview;
use crate::freeze::lapse::Lapse;
use crate::freeze::lapsed::Lapsed;
use crate::freeze::not_frozen::NotFrozen;
use crate::freeze::survey::{Closing, Survey};
use crate::index::Index;
use crate::local_scan::{
    kept_by_system, unavailable_roots, walk_mappings, RootState, SourceFile, SystemFile,
    UnknownBirths, Walked, WalkedRoot,
};
use crate::mapped_roots::MappedRoots;
use crate::spool_file::WRITE_CHUNK;

mod examine;
use examine::{examine, judge, Verdict};

/// Which files one invocation considers: those under the prefix, and of those
/// only the ones the selection names where there is one (spec: PK-17).
///
/// Both halves narrow and neither widens. The prefix is the folder a run was
/// asked for; the selection is the exact Entry Paths a caller wants packed —
/// the files one drop carried, say — so a file already in that folder that the
/// drop did not carry is outside the run rather than drawn into it.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Scope<'a> {
    /// The folder, or `None` for everything the mappings cover.
    pub(super) prefix: Option<&'a EntryPath>,
    /// The exact paths, or `None` for every file under the prefix.
    pub(super) only: Option<&'a BTreeSet<EntryPath>>,
}

impl Scope<'_> {
    /// Whether a file at `path` is one this run considers.
    fn covers(&self, path: &EntryPath) -> bool {
        self.prefix.is_none_or(|prefix| path.is_under(prefix))
            && self.only.is_none_or(|only| only.contains(path))
    }
}

/// The reservations this device holds, as one run read them, and which of
/// them it is to spare (spec: PK-17).
#[derive(Clone, Copy)]
pub(super) struct Reserved<'a> {
    /// Every reservation, as the run read them before its scan.
    pub(super) all: &'a [Reservation],
    /// The ones a drop in this process held live at any moment since.
    pub(super) live: &'a LiveSince<'a>,
}

/// Walks the folder and decides what this invocation can pack.
///
/// The eligibility rule is PK-1's, and it is about the Container kind rather
/// than the Entry count: a file not yet in the Library, or one whose current
/// Entry is held by a one-file Container. The second half holds however the
/// local file compares — a modification and a lost key are both eligible here,
/// because either way the replacement is built from the local bytes (spec:
/// PK-13). An Entry a Pack already holds is never eligible, and existing Packs
/// are neither read nor listed for removal (spec: PK-1, PK-2).
///
/// Scope is EP-10's, exactly as the sync reads it: a path with a current Entry
/// and no local materialization row is one this device never put on disk, and a
/// mapping covering it does not change that. Such a path is left alone rather
/// than surfaced — it is outside this device's scope, not a file it is failing
/// to back up.
///
/// A mapped root the device cannot vouch for is bounded out the same way the
/// sync bounds it (spec: EP-12): nothing under it is walked, so it contributes no
/// candidate, and it is reported in [`FreezeOutcome::unavailable`] rather than
/// being passed over. A freeze infers no deletion, so the only harm such a root
/// can do it is silence — and a run that packed nothing because a disk is
/// unplugged looks exactly like the second run over an already-packed folder,
/// which is why the mapping is reported. A root that holds files on a filesystem
/// the mapping does not record is available and re-stamped, which is this step's
/// own write through the port rather than the walk's.
///
/// The scope bounds the same question a second way. A freeze selects the
/// eligible files under the folder one invocation names, and of those only the
/// paths its selection names where it has one (spec: PK-17), so a file outside
/// either is not a candidate this scan passed over quietly: PK-14 governs what a
/// scan may keep silent about among the files it considered, and a run over
/// that other folder — or over the Library root — is what considers the rest.
/// Being named in the selection makes nothing eligible: every rule above still
/// decides.
///
/// The expensive comparison is paid only where it decides something. Every
/// selected file is hashed, because a Pack's entry table has to be written
/// before its content and the hash is part of that table. A Pack-resident Entry
/// is hashed only when the cheap comparison says the file may have moved, and a
/// file that turns out unchanged costs a read and refreshes what this device
/// last saw of it rather than being read again next time.
///
/// The reservations this device holds are read against the same walk
/// (spec: PK-17): one whose every path this run considered is released — by the
/// commit, where any of its paths is selected, or on its own, saying why for
/// each path, where none is ([`Closing`]). A file a drop in progress reserved
/// is not considered at all: the drop has not finished bringing its book, and
/// packing the pages landed so far would leave the rest for a second Pack.
///
/// [`FreezeOutcome::unavailable`]: crate::freeze::FreezeOutcome::unavailable
pub(super) async fn scan(
    index: &dyn Index,
    roots: &dyn MappedRoots,
    births: &UnknownBirths,
    scope: Scope<'_>,
    key_lost: &BTreeSet<ContainerId>,
    reserved: Reserved<'_>,
    now: DeviceTime,
) -> FreezeResult<Survey> {
    let Candidates {
        mappings,
        walked,
        found,
        system: _,
        kinds,
        waiting,
    } = candidates(index, roots, births, scope, reserved.all).await?;

    // The re-stamp is this step's own write through the port rather than the
    // walk's, and it is the run's alone: a preview reads the same walk and
    // writes nothing (spec: EP-12).
    for root in &walked {
        if let RootState::Stamp(identity) = &root.state {
            index
                .set_mapping(root.mapping.clone().stamped(identity.clone()))
                .await?;
        }
    }

    let mut survey = Survey {
        selected: Vec::new(),
        packed_already: 0,
        surfaced: Vec::new(),
        refreshed: Vec::new(),
        unavailable: unavailable_roots(&walked),
        closing: Vec::new(),
    };
    // `found` is keyed by Entry Path, so this walk is already the order
    // segmentation needs (spec: EP-3, PK-3).
    let mut considered = 0usize;
    let mut buffer = vec![0u8; WRITE_CHUNK];
    for source in found
        .values()
        .filter(|source| scope.covers(&source.path) && !waiting.contains(&source.path))
    {
        considered += 1;
        examine(
            index,
            roots,
            &kinds,
            key_lost,
            now,
            source,
            &mut buffer,
            &mut survey,
        )
        .await?;
    }

    survey.closing = closing(
        index,
        Walk {
            walked: &walked,
            found: &found,
            kinds: &kinds,
        },
        scope,
        key_lost,
        reserved,
        &survey,
    )
    .await?;

    // Counts only: a prefix and a selection are Entry Paths and a local root is
    // a local path, and none of them may reach a diagnostic event.
    debug!(
        mappings,
        files = considered,
        waiting = waiting.len(),
        closing = survey.closing.len(),
        selection = scope.only.map(BTreeSet::len),
        selected = survey.selected.len(),
        packed_already = survey.packed_already,
        surfaced = survey.surfaced.len(),
        unavailable = survey.unavailable.len(),
        "scanned a folder for freezing",
    );
    Ok(survey)
}

/// What the walk found, as the reading of the reservations asks it.
struct Walk<'a> {
    walked: &'a [WalkedRoot],
    found: &'a BTreeMap<EntryPath, SourceFile>,
    kinds: &'a BTreeMap<ContainerId, ContainerKind>,
}

/// Which reservations this run decides every path of, and what it makes of
/// each (spec: PK-17).
///
/// A reservation in progress is not decided at all, nor are any of its paths
/// selected ([`Candidates::waiting`]): its drop is still landing pages the run
/// never read, or was cut off before it brought them all, and releasing it —
/// with the commit or as lapsed — would leave the rest of the book landing
/// unreserved, or arriving with the next drop of its folder for a Pack of its
/// own. Its own run decides it, once the drop that holds it or takes it over
/// has finished receiving (spec: PK-17, LA-12). One a drop in this process
/// held live at any moment since the run read the reservations is spared as
/// well, for the reason [`LiveSince`] gives.
///
/// Any other reservation is decided where every one of its paths is one the
/// run considered: inside its scope, and not under a mapped root the walk
/// could not vouch for (spec: EP-12) — a book on an unplugged disk is not a
/// book whose pages are gone. One that names a path the run did not consider
/// is left for a run that does, and one that names no path is decided only by
/// the run a server takes it up as (spec: PK-17).
///
/// A decided reservation with any path selected is packed, and the commit
/// releases it. One with none is released with a [`Lapsed`] per path saying
/// why — save a path the scan has already surfaced, which has its finding
/// (spec: PK-14). A path no mapping represents is ineligible rather than gone:
/// nothing looked for it (spec: EP-9).
async fn closing(
    index: &dyn Index,
    walk: Walk<'_>,
    scope: Scope<'_>,
    key_lost: &BTreeSet<ContainerId>,
    reserved: Reserved<'_>,
    survey: &Survey,
) -> FreezeResult<Vec<Closing>> {
    let selected: BTreeSet<&EntryPath> = survey
        .selected
        .iter()
        .map(|selected| &selected.source.path)
        .collect();
    let surfaced: BTreeSet<&EntryPath> = survey
        .surfaced
        .iter()
        .map(|surfaced| match surfaced {
            NotFrozen::ModifiedInPack { path, .. } | NotFrozen::KeyLostInPack { path, .. } => path,
        })
        .collect();
    let unwalked = |path: &EntryPath| {
        represented_by(walk.walked, path)
            .is_some_and(|root| matches!(root.state, RootState::Unavailable(_)))
    };

    let mut closing = Vec::new();
    for reservation in reserved.all {
        if reservation.in_progress() || reserved.live.spares(reservation.id) {
            continue;
        }
        // A received reservation naming no path is one no drop leaves — a
        // drop that landed nothing releases its reservation rather than
        // marking it — but the Index can hold one all the same. Any run
        // would pass the test below for it, there being no path to fail it,
        // so only the run taken up for it decides it: its folder with a
        // selection of nothing.
        if reservation.paths.is_empty()
            && !(scope.prefix == reservation.folder.as_ref()
                && scope.only.is_some_and(BTreeSet::is_empty))
        {
            continue;
        }
        if !reservation
            .paths
            .iter()
            .all(|path| scope.covers(path) && !unwalked(path))
        {
            continue;
        }
        if reservation.paths.iter().any(|path| selected.contains(path)) {
            closing.push(Closing::Packs(reservation.id));
            continue;
        }
        let mut lapsed = Vec::new();
        for path in &reservation.paths {
            if surfaced.contains(path) {
                continue;
            }
            let lapse = if represented_by(walk.walked, path).is_none() {
                Lapse::Ineligible
            } else {
                match walk.found.get(path) {
                    None => Lapse::Gone,
                    Some(source) => match judge(index, walk.kinds, key_lost, source).await? {
                        Verdict::InPack | Verdict::InPackTouched(_) | Verdict::KeyLostInPack(_) => {
                            Lapse::InPack
                        }
                        _ => Lapse::Ineligible,
                    },
                }
            };
            lapsed.push(Lapsed {
                path: path.clone(),
                lapse,
            });
        }
        closing.push(Closing::Lapses(reservation.id, lapsed));
    }
    Ok(closing)
}

/// What every scan of the folder starts from: the walk of the mappings and the
/// kinds of the Containers under the folder.
///
/// Shared by the run and the preview, so that the two cannot read a different
/// set of files or decide a kind differently.
struct Candidates {
    /// How many mappings this device records.
    mappings: usize,
    /// Each mapping's root, and what the walk found it to be.
    walked: Vec<WalkedRoot>,
    /// The regular files under the available roots, by Entry Path.
    found: BTreeMap<EntryPath, SourceFile>,
    /// What the walk left out because an operating system keeps it, which a
    /// preview counts and a run passes over (spec: EP-17).
    system: Vec<SystemFile>,
    /// The kind of every Container holding an Entry under the folder.
    kinds: BTreeMap<ContainerId, ContainerKind>,
    /// The paths a drop in progress reserved, which no run selects until the
    /// drop has finished receiving (spec: PK-17).
    waiting: BTreeSet<EntryPath>,
}

async fn candidates(
    index: &dyn Index,
    roots: &dyn MappedRoots,
    births: &UnknownBirths,
    scope: Scope<'_>,
    reservations: &[Reservation],
) -> FreezeResult<Candidates> {
    let mappings = index.mappings().await?;
    // The stale scratches the walk met are a sync's to remove (spec: OC-8): a
    // freeze reads the folders and writes nothing into them.
    let Walked {
        found,
        roots: walked,
        stale: _,
        system,
    } = walk_mappings(roots, &mappings, births).await?;

    // The kind is what decides eligibility, and the port answers kinds a prefix
    // at a time. One walk under the run's own prefix answers every lookup below,
    // however many mappings overlap it and whichever Containers their Entries
    // turn out to share (spec: PK-8).
    let kinds = index
        .containers_under(scope.prefix)
        .await?
        .into_iter()
        .map(|container| (container.id, container.kind))
        .collect();
    let waiting = reservations
        .iter()
        .filter(|reservation| reservation.in_progress())
        .flat_map(|reservation| reservation.paths.iter().cloned())
        .collect();
    Ok(Candidates {
        mappings: mappings.len(),
        walked,
        found,
        system,
        kinds,
        waiting,
    })
}

/// Counts what [`scan`] would select under the same scope, and what it would
/// leave out, without reading a byte of any file and without writing anything.
///
/// The same walk and the same verdict per file as the run (spec: PK-1, PK-2,
/// EP-10, PK-17) — a file a drop in progress reserved left out of both — stopped
/// where the run would start hashing: the selection is
/// decided by the catalog and the stat alone, and the bytes a selected file
/// would bring are its length as the walk read it. The one question a hash
/// answers — whether a Pack-held file whose stat moved really changed — is
/// left unasked, so such a file is counted as one that may have changed; it is
/// left out either way.
///
/// The Keyring is not read. A lost key decides nothing about selection — a
/// one-file Container is absorbed whether or not its key survives, and a Pack
/// is left alone whether or not it does (spec: PK-1, PK-13) — so a preview
/// needs no Storage and no key at all.
///
/// An Entry under the folder whose file this device does not have is counted
/// as not here, beside the files on disk this device never placed: a freeze
/// would need to fetch either before it could pack it, which is not a freeze's
/// work (spec: EP-10).
///
/// What an operating system keeps beside the person's files under the folder —
/// which the walk left out by name — is counted apart, files and unentered
/// folders each, and an Entry carried in at such a path before that rule is not
/// counted as not here: its file was never looked for (spec: EP-17).
///
/// A mapping whose root this device cannot vouch for is reported where it is
/// the one representing the folder, and nothing under it is counted at all —
/// not even as not here, since its files were never looked for (spec: EP-12).
/// One standing for some other part of the Library says nothing about this
/// folder, so it is not reported: the run reports every such mapping because
/// its outcome is about the device's scan, and this answer is about one folder.
pub(super) async fn preview(
    index: &dyn Index,
    roots: &dyn MappedRoots,
    births: &UnknownBirths,
    scope: Scope<'_>,
) -> FreezeResult<FreezePreview> {
    // Read here rather than handed in: a preview decides no reservation, and
    // what it needs of them is only which files wait for a drop to finish.
    let reservations = index.reservations().await?;
    let Candidates {
        mappings,
        walked,
        found,
        system,
        kinds,
        waiting,
    } = candidates(index, roots, births, scope, &reservations).await?;

    let unreachable = |path: &EntryPath| {
        represented_by(&walked, path)
            .is_some_and(|root| matches!(root.state, RootState::Unavailable(_)))
    };
    // An Entry carried in before EP-17 at a path the walk leaves out: its file
    // was never looked for, so it is not one this device has not got.
    let left_out = |path: &EntryPath| {
        represented_by(&walked, path)
            .is_some_and(|root| kept_by_system(root.mapping.prefix.as_ref(), path))
    };
    let mut preview = FreezePreview {
        unavailable: match scope.prefix {
            Some(prefix) => {
                let folders = represented_by(&walked, prefix).map(|root| &root.mapping.prefix);
                unavailable_roots(&walked)
                    .into_iter()
                    .filter(|root| Some(&root.prefix) == folders)
                    .collect()
            }
            None => unavailable_roots(&walked),
        },
        ..FreezePreview::default()
    };
    let nothing_lost = BTreeSet::new();
    for source in found
        .values()
        .filter(|source| scope.covers(&source.path) && !waiting.contains(&source.path))
    {
        match judge(index, &kinds, &nothing_lost, source).await? {
            Verdict::New | Verdict::Absorbs(_) => {
                preview.files += 1;
                preview.bytes += source.size;
            }
            Verdict::NotMaterialized => preview.not_here += 1,
            // Neither in the Library nor going into it: the sync is what says
            // what becomes of it (spec: EP-15).
            Verdict::Departed => {}
            // A copy of an Entry that stands elsewhere now, waiting to follow
            // it rather than to be frozen (spec: FM-15).
            Verdict::AwaitsRelocation => {}
            Verdict::KeyLostInPack(_) | Verdict::InPack => preview.in_pack += 1,
            Verdict::InPackTouched(_) => preview.changed_in_pack += 1,
        }
    }
    preview.not_here += index
        .entries_under(scope.prefix)
        .await?
        .iter()
        .filter(|location| {
            scope.covers(location.path())
                && !found.contains_key(location.path())
                && !unreachable(location.path())
                && !left_out(location.path())
        })
        .count();
    for left in system.iter().filter(|left| scope.covers(&left.path)) {
        if left.folder {
            preview.system_folders += 1;
        } else {
            preview.system_files += 1;
        }
    }

    // Counts only, for the reason the run gives.
    debug!(
        mappings,
        selection = scope.only.map(BTreeSet::len),
        selected = preview.files,
        in_pack = preview.in_pack,
        changed_in_pack = preview.changed_in_pack,
        not_here = preview.not_here,
        system_files = preview.system_files,
        system_folders = preview.system_folders,
        unavailable = preview.unavailable.len(),
        "previewed a freeze of a folder",
    );
    Ok(preview)
}

/// The mapping that represents `path`: the top-level one standing for its first
/// component, or else the Library-root one, which represents the remainder
/// (spec: EP-9). `None` where this device maps neither.
fn represented_by<'w>(walked: &'w [WalkedRoot], path: &EntryPath) -> Option<&'w WalkedRoot> {
    walked
        .iter()
        .find(|root| {
            root.mapping
                .prefix
                .as_ref()
                .is_some_and(|prefix| prefix.as_str() == path.top_level())
        })
        .or_else(|| walked.iter().find(|root| root.mapping.prefix.is_none()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::device_state::Mapping;
    use crate::entry_paths::entry_path;
    use crate::in_memory_fs::InMemoryFs;
    use crate::in_memory_index::InMemoryIndex;
    use crate::local_scan::NONE_PLACED;

    // EP-9, EP-12: a preview is about one folder. A mapping whose root is
    // gone is reported where it is the one representing the folder, and not
    // where it stands for another part of the Library — which a person asked
    // about this folder could do nothing about.
    #[tokio::test]
    async fn a_preview_reports_only_the_unreachable_mapping_its_folder_lies_under() {
        let fs = InMemoryFs::new();
        fs.write_file(Path::new("/library/books/page-001.jpg"), b"a page");
        let index = InMemoryIndex::new();
        index
            .set_mapping(Mapping::new(None, "/library".into()))
            .await
            .expect("a mapping is recorded");
        index
            .set_mapping(Mapping::new(
                Some(entry_path("albums")),
                "/unplugged".into(),
            ))
            .await
            .expect("a mapping is recorded");

        let books = entry_path("books");
        let elsewhere = preview(
            &index,
            &fs,
            &NONE_PLACED,
            Scope {
                prefix: Some(&books),
                only: None,
            },
        )
        .await
        .expect("the preview reads the folder");
        assert_eq!(elsewhere.files, 1);
        assert_eq!(elsewhere.unavailable, Vec::new());

        let albums = entry_path("albums");
        let under = preview(
            &index,
            &fs,
            &NONE_PLACED,
            Scope {
                prefix: Some(&albums),
                only: None,
            },
        )
        .await
        .expect("the preview reads the folder");
        assert_eq!(under.files, 0);
        assert_eq!(
            under
                .unavailable
                .iter()
                .map(|root| root.prefix.clone())
                .collect::<Vec<_>>(),
            [Some(albums)],
        );
    }
}
