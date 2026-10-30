use std::collections::BTreeMap;

use coffret_model::EntryPath;

use crate::index_error::IndexResult;
use crate::mapped_name::MappedName;

use super::{relocate_refusal, Pending, Relocation, RelocationRefused, Run};

impl Run<'_> {
    /// Relocates files that stand in each other's places, `members` in the
    /// order each one's Entry stands where the next one's file does.
    ///
    /// No member can be relocated first while another stands where it goes, so the
    /// first member's place is exchanged with each other member's in turn:
    /// each exchange puts one file where its Entry stands and brings the next
    /// one's file to the first place, and the last one brings the last file
    /// home. Each exchange is followed by its two records.
    pub(super) async fn exchange_cycle(&mut self, members: Vec<Pending>) -> IndexResult<()> {
        let anchor = &members[0];
        let anchor_mapping = anchor.looked.mapping.clone();
        let anchor_at = anchor.looked.from().clone();
        let anchor_relative = anchor.source.relative.clone();
        for (position, member) in members.iter().enumerate().skip(1) {
            let arriving = &members[position - 1];
            let mapping = &member.looked.mapping;
            match self
                .disk
                .relocations
                .exchange(
                    MappedName {
                        root: &anchor_mapping.local_root,
                        expected: anchor_mapping.expected_root_id.as_ref(),
                        relative: &anchor_relative,
                    },
                    MappedName {
                        root: &mapping.local_root,
                        expected: mapping.expected_root_id.as_ref(),
                        relative: &member.source.relative,
                    },
                )
                .await
            {
                Ok(()) => {
                    let at = member.looked.from().clone();
                    self.index
                        .relocate_local_entries(&[
                            (anchor_at.clone(), at.clone()),
                            (at, anchor_at.clone()),
                        ])
                        .await?;
                    self.found.push(Relocation::Relocated {
                        from: arriving.looked.from().clone(),
                        to: arriving.looked.to().clone(),
                    });
                }
                Err(refused) => {
                    // The file now at the first place stays there, its record
                    // with it; every file still to come stays where it is, and
                    // where each is to go stands another of them, still waiting.
                    self.found.push(Relocation::Refused {
                        from: anchor_at.clone(),
                        to: arriving.looked.to().clone(),
                        reason: relocate_refusal(refused),
                    });
                    for rest in &members[position..] {
                        self.refuse(&rest.looked.local, RelocationRefused::WaitingFileInTheWay);
                    }
                    return Ok(());
                }
            }
        }
        let last = members.last().expect("a cycle has members");
        self.found.push(Relocation::Relocated {
            from: last.looked.from().clone(),
            to: last.looked.to().clone(),
        });
        Ok(())
    }
}

/// A run of pending relocations each of whose destination is the next one's source,
/// back to the first, where one is left: what no relocation of them can start.
pub(super) fn cycle_in(pending: &BTreeMap<EntryPath, Pending>) -> Option<Vec<EntryPath>> {
    let start = pending.keys().next()?.clone();
    let mut seen = vec![start];
    loop {
        let next = pending[seen.last().expect("the walk holds its start")]
            .looked
            .to()
            .clone();
        if let Some(position) = seen.iter().position(|from| *from == next) {
            return Some(seen.split_off(position));
        }
        // Every pending relocation is waiting on another pending one once the ready
        // ones are made, so the walk stays among them.
        if !pending.contains_key(&next) {
            return None;
        }
        seen.push(next);
    }
}
