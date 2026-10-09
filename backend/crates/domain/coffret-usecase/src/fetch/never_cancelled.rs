use crate::fetch::cancellation::Cancellation;

/// A fetch nobody will cancel, which is every fetch whose caller waits for its
/// answer.
#[derive(Debug, Clone, Copy, Default)]
pub struct NeverCancelled;

impl Cancellation for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// The one [`NeverCancelled`] a request defaults to.
pub static NEVER_CANCELLED: NeverCancelled = NeverCancelled;
