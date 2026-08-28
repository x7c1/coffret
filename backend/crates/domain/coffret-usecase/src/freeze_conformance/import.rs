use coffret_format::DecodedContainer;
use coffret_model::{ContainerKind, EntryPath, Generation};

use crate::freeze_conformance::fixtures::{
    filler, footprint, freeze, freeze_under, keys, map, merged, opened, spooled, write, TARGET,
};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;

/// The files a case imports, in the order they stand in the Library.
///
/// Sizes on both sides of the target, and none of them a multiple of anything,
/// so the cut falls in several different places rather than in one.
fn files() -> Vec<(String, Vec<u8>)> {
    (0..14)
        .map(|index| {
            (
                format!("albums/2026/{index:03}.jpg"),
                filler(40 + (index * 53) % 190, index as u8),
            )
        })
        .collect()
}

/// A folder of files freezes into path-ordered Packs, and another device can
/// open them.
///
/// This is the whole point of the operation, so it is asserted the whole way
/// round rather than from what the run reported. Every produced Container
/// records kind `Pack` — a Pack holding one Entry is still a Pack, and the kind
/// is never inferred from the count (spec: PK-15). The Entries arrive in Entry
/// Path order and every one of them decodes back to the file that was on disk
/// (spec: PK-3, FM-1, FM-2, FM-3, FM-4, FM-5, FM-6, FM-7, FM-8, FM-9). The
/// batch is one Journal record adding exactly those Packs and removing nothing,
/// because nothing was in the Library to absorb — a freeze builds Packs from
/// local files directly rather than uploading one-file Containers first
/// (spec: PK-7, CP-1).
///
/// The segmentation is held to both halves of its rule from the Packs
/// themselves: every normal Pack measures at or below the target before padding
/// (spec: PK-6), and no two adjacent normal Packs could be merged without going
/// over — which is what says the cut was made as late as it could be
/// (spec: PK-4). No empty Pack exists, because there is nothing for one to hold.
pub async fn a_folder_freezes_into_path_ordered_packs(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let files = files();
    for (relative, content) in &files {
        write(fixture.source_folder(), relative, content).await;
    }

    let outcome = freeze(fixture, &keys, TARGET, 1).await;

    assert!(
        outcome.packs.len() > 1,
        "the target is small enough that the folder is cut several times, got {} Pack(s)",
        outcome.packs.len(),
    );
    assert_eq!(outcome.frozen_entries(), files.len());
    assert!(
        outcome.absorbed.is_empty(),
        "nothing was in the Library to absorb (spec: PK-7)",
    );
    assert!(outcome.surfaced.is_empty(), "nothing was surfaced");
    assert_eq!(outcome.packed_already, 0);

    let commit = outcome
        .commit
        .expect("a folder of new files is worth a commit");
    assert_eq!(commit.record.generation, Generation::FIRST);
    assert_eq!(commit.record.additions.len(), outcome.packs.len());
    assert!(commit.record.removals.is_empty());
    for addition in &commit.record.additions {
        assert_eq!(addition.container.kind, ContainerKind::Pack, "spec: PK-15");
        assert!(
            !addition.entries.is_empty(),
            "no empty Pack is created (spec: PK-3)",
        );
    }

    // Opened the long way round, so what is asserted is what a second device
    // would find rather than what the run said it wrote.
    let mut packs: Vec<DecodedContainer> = Vec::new();
    for pack in &outcome.packs {
        packs.push(opened(store, &commit.record, pack.container_id).await);
    }

    let placed: Vec<(String, Vec<u8>)> = packs
        .iter()
        .flat_map(|pack| &pack.entries)
        .map(|entry| {
            (
                entry.metadata.path.as_str().to_owned(),
                entry.content.clone(),
            )
        })
        .collect();
    assert_eq!(
        placed, files,
        "every file, in Entry Path order across the Packs, decoding to its own bytes \
         (spec: PK-3)",
    );

    for (pack, decoded) in outcome.packs.iter().zip(&packs) {
        assert_eq!(decoded.kind, ContainerKind::Pack);
        assert_eq!(footprint(decoded), pack.footprint);
        assert!(
            pack.oversized || pack.footprint <= TARGET,
            "a normal Pack measured {} against a target of {TARGET} (spec: PK-6)",
            pack.footprint,
        );
    }

    for (index_of, window) in packs.windows(2).enumerate() {
        if outcome.packs[index_of].oversized || outcome.packs[index_of + 1].oversized {
            continue;
        }
        assert!(
            merged(&window[0], &window[1]) > TARGET,
            "Packs {index_of} and {} would merge inside the target (spec: PK-4)",
            index_of + 1,
        );
    }

    // Uploading a file is placing it: the device may now report it as deleted if
    // it goes missing, which it could not for an Entry it never held
    // (spec: EP-10).
    for (relative, _) in &files {
        assert!(
            index
                .local_entry_at(&EntryPath::new(relative.clone()))
                .await
                .expect("asking the catalog for a local row must succeed")
                .is_some(),
            "{relative} is this device's own materialization",
        );
    }
    assert_eq!(
        spooled(fixture.spool()).await,
        0,
        "a committed batch leaves no ciphertext on the device",
    );
    assert!(
        index
            .pending_uploads()
            .await
            .expect("asking the catalog for pending uploads must succeed")
            .is_empty(),
        "a committed Container is no longer a candidate for cleanup (spec: OC-2)",
    );
}

/// A prefix narrows the run to one folder and never widens it.
///
/// Which folder to freeze is the request's to say, and it is an intersection
/// with the mappings rather than a substitute for them: a mapping is what puts a
/// local file at an Entry Path at all (spec: EP-9), so the run covers exactly
/// the part of the mapped subtree the prefix names. A file outside it is not
/// eligible and not a finding either — never having been considered is a
/// different thing from having been passed over (spec: PK-14).
///
/// The second run is what says the narrowing was the prefix's doing rather than
/// a walk that missed the rest: over the same mappings with no prefix, what the
/// first run left is eligible after all, and what it packed is not (spec: PK-2).
/// The two runs' Packs are separate groupings, whose path ranges neither
/// partition the Library nor need stay apart (spec: PK-8).
pub async fn a_prefix_narrows_the_run_to_one_folder(fixture: &FreezeUnderTest) {
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let inside = ["albums/2026/a.jpg", "albums/2026/b.jpg"];
    let outside = ["albums/2025/old.jpg", "books/page.png"];
    for (seed, relative) in inside.iter().chain(&outside).enumerate() {
        write(fixture.source_folder(), relative, &filler(60, seed as u8)).await;
    }

    let outcome = freeze_under(fixture, &keys, "albums/2026", TARGET, 1).await;

    assert_eq!(outcome.frozen_entries(), inside.len());
    assert!(
        outcome.surfaced.is_empty(),
        "a file the run never considered is not a finding (spec: PK-14)",
    );
    assert_eq!(outcome.packed_already, 0);
    assert!(outcome.absorbed.is_empty());

    let commit = outcome
        .commit
        .expect("the folder the prefix names is worth a commit");
    let mut packed: Vec<String> = commit
        .record
        .additions
        .iter()
        .flat_map(|addition| &addition.entries)
        .map(|entry| entry.path.as_str().to_owned())
        .collect();
    packed.sort();
    assert_eq!(
        packed,
        inside
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<String>>(),
        "the batch holds the folder the prefix names and nothing else",
    );

    for path in outside {
        assert!(
            index
                .entry_at(&EntryPath::new(path))
                .await
                .expect("asking the catalog for a path must succeed")
                .is_none(),
            "{path} is outside the prefix, so the run left it out of the Library",
        );
    }

    let rest = freeze(fixture, &keys, TARGET, 2).await;
    assert_eq!(
        rest.frozen_entries(),
        outside.len(),
        "with no prefix the rest of the mapped folder is eligible after all",
    );
    assert_eq!(
        rest.packed_already,
        inside.len(),
        "and what the first run packed is not (spec: PK-2)",
    );
    assert!(
        rest.absorbed.is_empty(),
        "no one-file Container was ever in the Library (spec: PK-7)"
    );
    assert!(rest.surfaced.is_empty());
}

/// A file larger than the target forms an oversized singleton Pack.
///
/// Entries are indivisible across Containers, so a file over the target is not
/// split and is not carried along with its neighbors either: it becomes a Pack
/// of its own, still of kind `Pack` rather than of some third kind
/// (spec: PK-3, PK-15). The Entries around it are unaffected, which is what says
/// the singleton is a consequence of the rule rather than of the cut collapsing.
pub async fn a_file_larger_than_the_target_forms_a_singleton_pack(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;

    let small = filler(30, 0x11);
    let huge = filler(4 * TARGET as usize, 0x37);
    for (relative, content) in [
        ("albums/a.jpg", &small),
        ("albums/b.jpg", &small),
        ("albums/m-huge.arw", &huge),
        ("albums/x.jpg", &small),
        ("albums/y.jpg", &small),
    ] {
        write(fixture.source_folder(), relative, content).await;
    }

    let outcome = freeze(fixture, &keys, TARGET, 1).await;
    let commit = outcome.commit.expect("five new files are worth a commit");

    let shapes: Vec<(usize, bool)> = outcome
        .packs
        .iter()
        .map(|pack| (pack.entries, pack.oversized))
        .collect();
    assert_eq!(
        shapes,
        vec![(2, false), (1, true), (2, false)],
        "the large Entry is a Pack of its own and does not disturb its neighbors \
         (spec: PK-3, PK-4)",
    );

    let singleton = outcome.packs[1].container_id;
    assert!(
        outcome.packs[1].footprint > TARGET,
        "an oversized singleton is over the target by definition",
    );
    let decoded = opened(store, &commit.record, singleton).await;
    assert_eq!(
        decoded.kind,
        ContainerKind::Pack,
        "an oversized singleton is a form of Pack, not a third kind (spec: PK-15)",
    );
    assert_eq!(decoded.entries.len(), 1);
    assert_eq!(decoded.entries[0].content, huge);
    assert_eq!(
        decoded.entries[0].metadata.path.as_str(),
        "albums/m-huge.arw",
    );
}
