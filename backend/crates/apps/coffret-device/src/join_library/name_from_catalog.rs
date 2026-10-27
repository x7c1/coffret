use std::sync::Arc;

use coffret_local_fs::UnixFs;
use coffret_model::{LibraryName, Redacted};
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::catch_up::{catch_up_catalog, CatchUpRequest};
use coffret_usecase::{Index, LibraryKeys, ObjectStore};
use tracing::warn;

use super::run::OPERATION;
use crate::device_time::now;
use crate::error::Result;
use crate::staging::Staging;

/// The Library's name, read from the catalog the join replays into the staged
/// directory (spec: CK-9, FM-15).
///
/// `None` where the catch-up did not complete, or the Library's catalog names
/// none — one made before records carried a name, whose first open by a
/// device that held it before names it. A catch-up that did not complete is
/// recorded and not raised: the join has what it needs, and the first open
/// catches up again.
pub(super) async fn name_from_catalog(
    store: Arc<dyn ObjectStore>,
    staging: &Staging,
    keys: &LibraryKeys,
) -> Option<LibraryName> {
    let caught: Result<Option<LibraryName>> = async {
        let index = SqliteIndex::open(staging.staged().index_file())?;
        catch_up_catalog(CatchUpRequest::new(
            store.as_ref(),
            &index,
            keys,
            &UnixFs::new(),
            now(),
        ))
        .await?;
        Ok(index.library_name().await?)
    }
    .await;
    match caught {
        Ok(name) => name,
        Err(refused) => {
            warn!(
                operation = OPERATION,
                reason = %refused.redacted(),
                "the joined Library's catalog was not caught up; its name is read when it is \
                 first opened"
            );
            None
        }
    }
}
