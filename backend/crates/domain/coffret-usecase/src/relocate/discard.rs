use std::sync::Arc;

use crate::index_error::IndexResult;
use crate::local_scan::SourceFile;

use super::look::Looked;
use super::{Relocation, RelocationRefused, Run};

impl Run<'_> {
    /// Discards a waiting file whose Entry's current path already holds this
    /// device's own copy of it, and forgets its record (spec: EP-15).
    pub(super) async fn discard(&mut self, one: Looked, source: SourceFile) -> IndexResult<()> {
        let root = &one.mapping.local_root;
        let expected = one.mapping.expected_root_id;
        match self
            .disk
            .trash
            .discard(root, expected.as_ref(), &source.relative)
            .await
        {
            Ok(()) => {
                self.index.forget_local_entry(one.from()).await?;
                self.emptied.vacated(root, expected, &source.relative);
                self.found.push(Relocation::Discarded {
                    from: one.from().clone(),
                    to: one.to().clone(),
                });
            }
            Err(refused) => {
                self.refuse(&one.local, RelocationRefused::Disk(Arc::new(refused)));
            }
        }
        Ok(())
    }
}
