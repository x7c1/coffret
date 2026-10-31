use coffret_model::Redacted;
use coffret_usecase::sync::remove_stale_scratches;
use tracing::{info, warn};

use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Removes what the writers of a process that is gone left in the mapped
    /// folders under coffret's scratch names, and answers how many it removed
    /// (spec: OC-8).
    ///
    /// For a server opening the Library, before it serves anything; why, and
    /// what is left, is [`remove_stale_scratches`]'s to say.
    ///
    /// A walk that could not go on is reported and goes no further: the
    /// Library opens all the same, and the next sync meets what is left.
    pub async fn remove_left_behind(&self) -> usize {
        let fs = self.local_fs.as_ref();
        match remove_stale_scratches(self.index.as_ref(), fs, fs).await {
            Ok(removed) => {
                if removed > 0 {
                    // How many and nothing else: a scratch's place is an
                    // Entry Path's folder (spec: EL-1).
                    info!(
                        operation = "remove_left_behind",
                        removed,
                        "removed the scratches a process that is gone left in the mapped folders",
                    );
                }
                removed
            }
            Err(refused) => {
                warn!(
                    operation = "remove_left_behind",
                    reason = %refused.redacted(),
                    "the mapped folders could not be walked for what a process that is gone left; the next sync removes it",
                );
                0
            }
        }
    }
}
