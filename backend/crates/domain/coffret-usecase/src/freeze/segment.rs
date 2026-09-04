use coffret_format::{ContainerFootprint, Header};
use coffret_model::ContainerKind;
use tracing::debug;

use crate::freeze::freeze_error::FreezeResult;
use crate::freeze::selected::Selected;

/// The meta section a Pack is closed at, whatever the size target says.
///
/// A Container's entry table has a ceiling of its own — a reader refuses a
/// declared meta section past [`Header::MAX_META_LEN`], and the layout refuses
/// to write one — and it is a ceiling the *size target* does not imply. The
/// target counts content and table together (spec: PK-6), so a gigabyte-scale
/// target filled with kilobyte-scale files reaches a table of tens of megabytes
/// while the Pack is still comfortably under target. Segmentation is the only
/// step that can do anything about it: by the time the layout refuses the table,
/// the cut has already been made, and repeating the freeze cuts it the same way.
///
/// So the table is a second reason to close a Pack, and the point it closes at
/// is half the ceiling rather than the ceiling itself. The margin is what makes
/// the invariant obvious instead of arithmetic: whatever a single further Entry
/// costs, and however the accumulator's reckoning of a row differs by a byte
/// from the encoder's, a Pack closed here is nowhere near a table a reader would
/// refuse. Nothing is lost by cutting early — the Packs are smaller and there
/// are more of them, which is the outcome PK-4 already admits for large Entries.
const MAX_SEGMENT_META_LEN: u64 = Header::MAX_META_LEN as u64 / 2;

/// One Pack's worth of selected Entries.
pub(super) struct Segment {
    /// Its members, in Entry Path order.
    pub(super) members: Vec<Selected>,
    /// What the Pack will measure before padding (spec: PK-6).
    pub(super) footprint: ContainerFootprint,
}

impl Segment {
    /// Whether this is an oversized singleton: one Entry that exceeds the target
    /// on its own (spec: PK-3).
    pub(super) fn oversized(&self, target: u64) -> bool {
        self.footprint.bytes() > target
    }
}

/// Cuts the selected Entries into Packs around the size target (spec: PK-3).
///
/// The Entries arrive in Entry Path order and stay in it, and the cut is the
/// simplest one that respects both halves of the rule: append the next Entry
/// while the resulting pre-padding footprint stays at or below the target, and
/// close the current Pack first when adding it would not. Entries are
/// indivisible, so a Pack that is still empty takes its first Entry whatever the
/// Entry weighs — which is how an oversized singleton comes about, and why more
/// than one undersized Pack can result (spec: PK-4).
///
/// A Pack closes for a second reason as well: an entry table that has reached
/// [`MAX_SEGMENT_META_LEN`], which the size target does not imply.
///
/// What falls out is the invariant PK-4 states: the first Entry of each Pack was
/// turned away by the one before it, so no two adjacent normal Packs could be
/// merged without going over. That is a property of this invocation and of
/// nothing wider — Packs from different invocations may overlap and interleave
/// in path order, and regrouping across them is repack's or compaction's job
/// (spec: PK-8).
///
/// No empty Pack is produced, because a segment is only closed when it has a
/// member and only opened by one.
pub(super) fn segment(selected: Vec<Selected>, target: u64) -> FreezeResult<Vec<Segment>> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut members: Vec<Selected> = Vec::new();
    let mut footprint = ContainerFootprint::empty(ContainerKind::Pack)?;

    for item in selected {
        let extended = footprint.extended(&item.plan)?;
        // One Entry never fills a table on its own — a row is a couple of
        // hundred bytes — so the meta rule only ever closes a Pack that already
        // has members, and the `is_empty` guard that keeps an Entry indivisible
        // covers both reasons.
        let full = extended.bytes() > target || extended.meta_len() > MAX_SEGMENT_META_LEN;
        if !members.is_empty() && full {
            segments.push(Segment {
                members: std::mem::take(&mut members),
                footprint,
            });
            footprint = ContainerFootprint::empty(ContainerKind::Pack)?.extended(&item.plan)?;
        } else {
            footprint = extended;
        }
        members.push(item);
    }
    if !members.is_empty() {
        segments.push(Segment { members, footprint });
    }

    debug!(
        packs = segments.len(),
        target,
        oversized = segments
            .iter()
            .filter(|segment| segment.oversized(target))
            .count(),
        "cut the selected Entries into Packs",
    );
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use coffret_format::EntryPlan;
    use coffret_model::{ContentHash, Mtime};

    use super::*;
    use crate::entry_paths::entry_path;
    use crate::local_scan::SourceFile;

    fn selected(path: &str, size: u64) -> Selected {
        let plan = EntryPlan::new(
            entry_path(path.to_owned()),
            Mtime::from_unix_seconds(1_700_000_000),
            size,
            ContentHash::from_bytes([0x11; ContentHash::BYTE_LEN]),
        );
        Selected {
            source: SourceFile {
                path: entry_path(path.to_owned()),
                local_path: path.into(),
                size,
                mtime: Mtime::from_unix_seconds(1_700_000_000),
                btime: None,
            },
            plan,
            absorbs: None,
        }
    }

    fn cut(sizes: &[u64], target: u64) -> Vec<Segment> {
        let selected = sizes
            .iter()
            .enumerate()
            .map(|(index, size)| selected(&format!("albums/{index:03}.jpg"), *size))
            .collect();
        segment(selected, target).expect("a segmentation of this size")
    }

    // PK-3, PK-4, PK-6: every normal Pack stays at or below the target, and no
    // two adjacent normal Packs could be merged without going over — which is
    // what says the cut was made as late as it could be rather than
    // arbitrarily.
    #[test]
    fn adjacent_normal_packs_cannot_be_merged() {
        for target in [200u64, 1_000, 4_096, 100_000] {
            let sizes: Vec<u64> = (0..40).map(|index| 100 + index * 137 % 900).collect();
            let packs = cut(&sizes, target);

            assert!(
                packs.iter().all(|pack| !pack.members.is_empty()),
                "no empty Pack is created (spec: PK-3)",
            );
            assert_eq!(
                packs.iter().map(|pack| pack.members.len()).sum::<usize>(),
                sizes.len(),
                "every selected Entry lands in exactly one Pack",
            );

            for window in packs.windows(2) {
                let (left, right) = (&window[0], &window[1]);
                if left.oversized(target) || right.oversized(target) {
                    continue;
                }
                let merged = left
                    .members
                    .iter()
                    .chain(&right.members)
                    .try_fold(
                        ContainerFootprint::empty(ContainerKind::Pack)
                            .expect("an empty Pack measures"),
                        |footprint, member| footprint.extended(&member.plan),
                    )
                    .expect("a merged table measures");
                assert!(
                    merged.bytes() > target,
                    "two adjacent normal Packs merged to {} against a target of {target}",
                    merged.bytes(),
                );
            }
        }
    }

    // PK-3: an Entry larger than the target stays indivisible and forms an
    // oversized singleton, and the Entries around it are unaffected.
    #[test]
    fn an_over_target_entry_forms_a_singleton() {
        // Roomy enough for two of the small Entries and not for three, so the
        // neighbors pair up on each side of the large one. The metadata is most
        // of what they weigh at this size, which is the point of measuring the
        // footprint rather than the content (spec: PK-6).
        const TARGET: u64 = 300;

        let packs = cut(&[10, 10, 5_000, 10, 10], TARGET);

        let shapes: Vec<(usize, bool)> = packs
            .iter()
            .map(|pack| (pack.members.len(), pack.oversized(TARGET)))
            .collect();
        assert_eq!(
            shapes,
            vec![(2, false), (1, true), (2, false)],
            "the large Entry is a Pack of its own and does not disturb its neighbors",
        );
    }

    // PK-4: Entries are indivisible, so consecutive Entries that each nearly
    // fill a Pack leave several undersized ones rather than being combined.
    #[test]
    fn indivisible_entries_leave_undersized_packs() {
        let packs = cut(&[600, 600, 600], 1_000);
        assert_eq!(packs.len(), 3);
        for pack in &packs {
            assert_eq!(pack.members.len(), 1);
            assert!(!pack.oversized(1_000));
        }
    }

    // Nothing selected is nothing to write: a run that produced an empty Pack
    // would commit a Container holding no user data at all (spec: FM-10, PK-3).
    #[test]
    fn nothing_selected_cuts_no_pack() {
        assert!(cut(&[], 1_000).is_empty());
    }

    // A freeze of very many small files splits rather than failing. What closes
    // the Packs here is the entry table's bound, which is this build's own
    // reason to cut: the register gives segmentation only the size target
    // (spec: PK-3).
    //
    // The entry table is what fills up here, not the Pack: the target is the
    // gigabyte-scale one a real run uses, and the content is a byte per Entry,
    // so nothing about the size rule would ever close a Pack. Without the meta
    // rule the whole selection would be cut into one Container, and
    // `Layout::plan` would then refuse to lay it out — permanently, since a
    // second attempt cuts it the same way.
    //
    // The Entries carry long Entry Paths so that the table reaches the bound on
    // thousands of rows rather than on the half-million a real photo library of
    // small files would take. What fills a table is the paths in it either way.
    #[test]
    fn very_many_small_entries_split_rather_than_outgrowing_the_meta_ceiling() {
        // A deep tree of long names: about a kilobyte of path per row, so the
        // table crosses the bound around thirty thousand of them.
        let deep = "folder-with-a-long-name/".repeat(40);
        let selected: Vec<Selected> = (0..40_000)
            .map(|index| selected(&format!("{deep}{index:06}.txt"), 1))
            .collect();

        // What `coffret-device` hands a real run: the target no size rule here
        // would ever reach with content of a byte per Entry.
        const GIGABYTE_TARGET: u64 = 1024 * 1024 * 1024;

        let packs = segment(selected, GIGABYTE_TARGET).expect("a segmentation of this size");

        assert!(
            packs.len() > 1,
            "a selection whose entry table outgrows the bound must be cut into \
             more than one Pack, got {}",
            packs.len(),
        );
        for pack in &packs {
            assert!(
                pack.footprint.meta_len() <= u64::from(Header::MAX_META_LEN),
                "a Pack of {} entries declares a meta section of {}, past the {} a \
                 Container may carry",
                pack.members.len(),
                pack.footprint.meta_len(),
                Header::MAX_META_LEN,
            );
            // The size rule never closed one of these: every Pack is far under
            // target, which is what says the table is what cut them.
            assert!(pack.footprint.bytes() < GIGABYTE_TARGET);
        }
        assert_eq!(
            packs.iter().map(|pack| pack.members.len()).sum::<usize>(),
            40_000,
            "every selected Entry still lands in exactly one Pack",
        );
    }
}
