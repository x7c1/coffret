use std::sync::Arc;

use coffret_model::Redacted;
use tracing::warn;

use crate::departure::departed::Departed;
use crate::index::Index;
use crate::index_error::IndexError;
use crate::local_trash::LocalTrash;
use crate::sync::Surfaced;

/// Discards one departed file into the trash, and says what became of it
/// (spec: EP-15).
///
/// A refusal is a finding and not a failure: the file stays where it is with
/// its row, so the next sync finds it departed again and asks again, and the
/// rest of the run goes on. Nothing here falls back on deleting the file. Only
/// a failure to forget the row of a file that did go is an error, since that
/// is the catalog failing rather than the trash — and the next sync, finding
/// the row and no file, forgets it then.
pub(crate) async fn discard_departed(
    index: &dyn Index,
    trash: &dyn LocalTrash,
    departed: Departed,
) -> Result<Surfaced, IndexError> {
    let Departed { source, expected } = departed;
    match trash
        .discard(&source.root, expected.as_ref(), &source.relative)
        .await
    {
        Ok(()) => {
            index.forget_local_entry(&source.path).await?;
            Ok(Surfaced::Discarded { path: source.path })
        }
        Err(refused) => {
            // What refused and of what kind, and nothing that names the file:
            // the path stays in the finding, for whoever asked for the run.
            warn!(
                reason = %refused.redacted(),
                "a deleted Entry's local file could not be discarded into the trash; it is left \
                 in place for the next sync",
            );
            Ok(Surfaced::DiscardRefused {
                path: source.path,
                cause: Arc::new(refused),
            })
        }
    }
}
