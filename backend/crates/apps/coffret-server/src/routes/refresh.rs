use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use coffret_device::CatchUpOutcome;
use serde::Serialize;

use crate::api_error::ApiError;
use crate::refresh::refresh_catalog;
use crate::state::ServerState;

/// What one refresh came to.
///
/// Three fields, and each of them answers a question a screen has: did anything
/// change at all, how much did the catalog gain, and how large is it now. The
/// generations the catalog moved between are not among them — they are the
/// Library's own bookkeeping (spec: CK-1), a browser can do nothing with them,
/// and the log already carries them for whoever is keeping the Library.
#[derive(Serialize)]
pub struct RefreshedDto {
    /// Whether the Library had a head this device had not seen.
    ///
    /// Kept apart from [`gained`](Self::gained) being zero, because they are not
    /// the same answer: a commit that only removed Entries advanced the catalog
    /// and gained nothing, and calling that "up to date" would tell somebody
    /// their screen is current when a row has just left it.
    advanced: bool,
    /// How many current Entries the catalog gained — negative where another
    /// device's commit removed more than it added.
    gained: i64,
    /// How many current Entries the Library holds now.
    entries: usize,
}

impl RefreshedDto {
    fn of(outcome: &CatchUpOutcome) -> Self {
        Self {
            advanced: outcome.advanced(),
            gained: outcome.gained(),
            entries: outcome.entries_after,
        }
    }
}

/// `POST /api/refresh`
///
/// Asks the Library what is new, and replays it into this device's catalog
/// (spec: CK-9).
///
/// This is the one control on the screen that reaches Storage because somebody
/// pressed it, and it exists because nothing else can: the explorer never polls
/// the remote head, and until a device catches up it cannot know a Container
/// another device committed — a joined device's first window would be an empty
/// Library, and a running one would never hear of what the other device added.
///
/// It takes no path. What a catch-up covers is the Library entire, because a
/// Journal record is replayed whole or not at all, and a route that narrowed it
/// would be asking for a catalog standing at no committed state.
///
/// It answers with what changed rather than with `202` and a poll, unlike the
/// fill and the sync: a refresh is over when the replay is, there is no progress
/// to report in between, and the person who pressed it is owed the answer.
///
/// Nothing is fetched. Every Entry it learns of arrives `remote`, and the bytes
/// come the way they always do — when a file is opened (spec: EP-10, EP-11).
pub async fn refresh(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<RefreshedDto>, ApiError> {
    Ok(Json(RefreshedDto::of(&refresh_catalog(&state).await?)))
}
