use coffret_format::ContainerFootprint;
use coffret_model::ContainerKind;
use tracing::debug;

use crate::freeze::freeze_error::FreezeResult;
use crate::freeze::selected::Selected;

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
        if !members.is_empty() && extended.bytes() > target {
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
    use coffret_model::{ContentHash, EntryPath, Mtime};

    use super::*;
    use crate::local_scan::SourceFile;

    fn selected(path: &str, size: u64) -> Selected {
        let plan = EntryPlan::new(
            EntryPath::new(path.to_owned()),
            Mtime::from_unix_seconds(1_700_000_000),
            size,
            ContentHash::from_bytes([0x11; ContentHash::BYTE_LEN]),
        );
        Selected {
            source: SourceFile {
                path: EntryPath::new(path.to_owned()),
                local_path: path.into(),
                size,
                mtime: Mtime::from_unix_seconds(1_700_000_000),
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
}
