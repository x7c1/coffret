use std::sync::Arc;

use crate::device_state::{LocalEntry, Mapping};
use crate::index_error::IndexResult;
use crate::local_scan::SourceFile;

use super::{Relocation, RelocationRefused, Run};

impl Run<'_> {
    /// Discards a waiting file into the desktop's trash and forgets its record,
    /// reporting `done` once it has (spec: EP-10, EP-15): a file whose Entry's
    /// current path already holds this device's own copy of it, and a file
    /// that cannot follow its Entry on this device.
    ///
    /// A discard the trash refuses leaves the file and its record where they
    /// are, still waiting, and is reported with what refused it.
    pub(super) async fn discard(
        &mut self,
        local: &LocalEntry,
        mapping: &Mapping,
        source: &SourceFile,
        done: Relocation,
    ) -> IndexResult<()> {
        let root = &mapping.local_root;
        let expected = mapping.expected_root_id;
        match self
            .disk
            .trash
            .discard(root, expected.as_ref(), &source.relative)
            .await
        {
            Ok(()) => {
                self.index
                    .forget_local_entry(&local.observation.path)
                    .await?;
                self.emptied.vacated(root, expected, &source.relative);
                self.found.push(done);
            }
            Err(refused) => {
                self.refuse(local, RelocationRefused::Disk(Arc::new(refused)));
            }
        }
        Ok(())
    }

    /// Takes a file that cannot follow its Entry off this device: discarded
    /// into the trash and its record forgotten, so the Entry is in the Library
    /// and not on this device at its current path (spec: EP-10).
    ///
    /// Only for a file already found unedited: an edited one is never
    /// discarded, and is refused before it comes here (spec: EP-15).
    pub(super) async fn leave(
        &mut self,
        local: &LocalEntry,
        mapping: &Mapping,
        source: &SourceFile,
    ) -> IndexResult<()> {
        let done = Relocation::Left {
            from: local.observation.path.clone(),
            to: local
                .relocate_to
                .clone()
                .expect("only a waiting record is relocated"),
        };
        self.discard(local, mapping, source, done).await
    }
}
