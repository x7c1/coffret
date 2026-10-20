use super::Desktop;
use crate::ServedLibrary;

impl Desktop {
    /// The Library served, if one is open and its server has not stopped.
    pub fn served(&self) -> Option<ServedLibrary> {
        self.shared.served().map(|opened| ServedLibrary {
            name: opened.state.name.clone(),
            unlocked: opened.state.holds_library(),
        })
    }
}
