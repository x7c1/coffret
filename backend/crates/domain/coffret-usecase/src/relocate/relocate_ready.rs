use std::collections::BTreeMap;

use coffret_model::EntryPath;

use crate::device_state::LocalEntryState;
use crate::index_error::IndexResult;
use crate::relocate_error::RelocateError;

use super::{relocate_refusal, Pending, Relocation, RelocationRefused, Run};

impl Run<'_> {
    /// Makes every pending relocation whose destination no other pending one is
    /// still to vacate, until none is left that can be made.
    pub(super) async fn relocate_ready(
        &mut self,
        pending: &mut BTreeMap<EntryPath, Pending>,
    ) -> IndexResult<()> {
        loop {
            let ready: Vec<EntryPath> = pending
                .iter()
                .filter(|(_, one)| !pending.contains_key(one.looked.to()))
                .map(|(from, _)| from.clone())
                .collect();
            if ready.is_empty() {
                return Ok(());
            }
            for from in ready {
                let one = pending
                    .remove(&from)
                    .expect("a ready relocation is pending");
                self.relocate_one(one).await?;
            }
        }
    }

    /// One rename on disk, and then the record follows it.
    async fn relocate_one(&mut self, one: Pending) -> IndexResult<()> {
        let Pending { looked, source } = one;
        let root = &looked.mapping.local_root;
        let expected = looked.mapping.expected_root_id;
        match self
            .disk
            .relocations
            .relocate(
                root,
                expected.as_ref(),
                &source.relative,
                &looked.to_relative,
            )
            .await
        {
            Ok(()) => {
                self.index
                    .relocate_local_entries(&[(looked.from().clone(), looked.to().clone())])
                    .await?;
                self.emptied.vacated(root, expected, &source.relative);
                self.found.push(Relocation::Relocated {
                    from: looked.from().clone(),
                    to: looked.to().clone(),
                });
            }
            // What stands in the way may be this device's own file, waiting
            // for a relocation of its own that was refused this run.
            Err(RelocateError::Taken { .. }) if self.waiting_at(looked.to()).await? => {
                self.refuse(&looked.local, RelocationRefused::WaitingFileInTheWay);
            }
            Err(refused) => self.refuse(&looked.local, relocate_refusal(refused)),
        }
        Ok(())
    }

    /// Whether the file at `path` is this device's own, still waiting to be
    /// relocated itself.
    async fn waiting_at(&self, path: &EntryPath) -> IndexResult<bool> {
        Ok(self
            .index
            .local_entry_at(path)
            .await?
            .is_some_and(|standing| {
                standing.state == LocalEntryState::Present && standing.awaits_relocation()
            }))
    }
}
