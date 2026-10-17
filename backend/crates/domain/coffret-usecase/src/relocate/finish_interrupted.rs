use std::collections::BTreeSet;

use coffret_model::EntryPath;

use crate::departure::MappedFile;
use crate::device_state::LocalEntryState;
use crate::index_error::IndexResult;

use super::look::Look;
use super::{Relocation, Run};

impl Run<'_> {
    /// Finishes what an interrupted run left between a rename on disk and the
    /// record that follows it, and answers whether there was any.
    ///
    /// A file relocated and its record not: the file is gone from where the record
    /// says, and the record's description of it stands at the Entry's path,
    /// where no record is. Two files exchanged and their records not: each
    /// stands where the other's record says, as that record describes it.
    pub(super) async fn finish_interrupted(&mut self, looked: &[Look]) -> IndexResult<bool> {
        let mut touched: BTreeSet<EntryPath> = BTreeSet::new();
        for look in looked {
            let Look::Looked(one) = look else {
                continue;
            };
            if one.local.state != LocalEntryState::Present
                || touched.contains(one.from())
                || touched.contains(one.to())
            {
                continue;
            }
            let MappedFile::File(arrived) = &one.target else {
                continue;
            };
            if !self.holds_copy(&one.local, arrived).await {
                continue;
            }
            let standing = self.index.local_entry_at(one.to()).await?;
            match (&one.source, standing) {
                (MappedFile::Nothing | MappedFile::Folder, None) => {
                    self.index
                        .relocate_local_entries(&[(one.from().clone(), one.to().clone())])
                        .await?;
                }
                // The file left where this record stands is the other
                // record's and not this one's: two files with one length and
                // one modification time are told apart by their content where
                // the records hold its hash.
                (MappedFile::File(left), Some(other))
                    if other.state == LocalEntryState::Present
                        && other.awaits_relocation()
                        && self.holds_copy(&other, left).await
                        && !self.holds_copy(&one.local, left).await =>
                {
                    self.index
                        .relocate_local_entries(&[
                            (one.from().clone(), one.to().clone()),
                            (one.to().clone(), one.from().clone()),
                        ])
                        .await?;
                    if other.relocate_to.as_ref() == Some(one.from()) {
                        self.found.push(Relocation::Relocated {
                            from: one.to().clone(),
                            to: one.from().clone(),
                        });
                    }
                }
                _ => continue,
            }
            self.found.push(Relocation::Relocated {
                from: one.from().clone(),
                to: one.to().clone(),
            });
            touched.insert(one.from().clone());
            touched.insert(one.to().clone());
        }
        Ok(!touched.is_empty())
    }
}
