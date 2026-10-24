use coffret_device::{DropRefusal, Redacted};

/// Why a handle held no drop, kept as the value the drop store answered with.
///
/// Every variant is a fact about the handle — never drawn, used already, held
/// past its lifetime — and none carries a name, so its identity is the whole
/// of what an event is told.
pub(crate) struct NotHeld(pub(crate) DropRefusal);

impl Redacted for NotHeld {
    fn redacted(&self) -> String {
        format!("DropRefusal::{:?}", self.0)
    }
}
