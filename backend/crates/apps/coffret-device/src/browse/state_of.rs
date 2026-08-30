use coffret_model::EntryPath;
use coffret_usecase::device_state::LocalEntryState;

use super::EntryState;
use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Whether this device has the file for one Entry (spec: EP-10).
    ///
    /// It answers about the local row alone and says nothing about whether the
    /// Library holds an Entry at the path: a row survives the Entry it was made
    /// for, so a path the Library no longer holds can still be
    /// [`Present`](EntryState::Present) here — which is exactly the file this
    /// device has and the Library does not.
    pub async fn state_of(&self, path: &EntryPath) -> Result<EntryState> {
        let held = self.index.local_entry_at(path).await?;
        Ok(match held.map(|local| local.state) {
            Some(LocalEntryState::Present) => EntryState::Present,
            Some(LocalEntryState::Absent) | None => EntryState::Remote,
        })
    }
}
