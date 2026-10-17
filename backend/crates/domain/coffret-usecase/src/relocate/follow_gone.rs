use crate::departure::MappedFile;
use crate::index_error::IndexResult;

use super::look::Looked;
use super::Run;

impl Run<'_> {
    /// The record of a file that is gone, or of one this device witnessed the
    /// deletion of, follows its Entry where nothing stands at the Entry's path,
    /// and is forgotten where something does: it describes nothing there
    /// (spec: EP-10).
    pub(super) async fn follow_gone(&mut self, one: Looked) -> IndexResult<()> {
        let vacant = matches!(one.target, MappedFile::Nothing)
            && self.index.local_entry_at(one.to()).await?.is_none();
        if vacant {
            self.index
                .relocate_local_entries(&[(one.from().clone(), one.to().clone())])
                .await
        } else {
            self.index.forget_local_entry(one.from()).await
        }
    }
}
