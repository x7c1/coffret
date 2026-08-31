use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::fill::{Activity, Declined, Reported};
use crate::state::ServerState;

/// What the server is doing on its own, which is one thing.
///
/// A fill and nothing else. Everything else this server does it does because a
/// request asked it to, and a request is answered rather than reported on; this
/// is the one piece of work nobody asked for, so it is the one thing there is to
/// tell a browser about.
#[derive(Serialize)]
pub struct ActivityDto {
    /// The latest fill, running or finished, and `null` where none has run.
    fill: Option<FillDto>,
}

#[derive(Serialize)]
struct FillDto {
    /// The folder being brought over; the Library root is the empty string, as
    /// it is in a listing.
    folder: String,
    /// `filling`, `done`, `stopped` or `superseded`.
    status: &'static str,
    /// How many of the folder's files the fill set out to bring over, and `0`
    /// until it has read the folder's listing.
    total: usize,
    /// How many of them are on this device now.
    done: usize,
    /// The Entries it did not bring over, each with what the file route would
    /// have said about it — so a row can be marked without anyone clicking it.
    declined: Vec<DeclinedDto>,
    /// What stopped the fill, and `null` where nothing did.
    stopped: Option<RefusalDto>,
}

#[derive(Serialize)]
struct DeclinedDto {
    path: String,
    #[serde(flatten)]
    refusal: RefusalDto,
}

/// One refusal, in the shape every refusal on these routes takes.
///
/// The same four fields under the same four names, so a browser reads a
/// declined Entry with the code it already has for a refused request.
#[derive(Serialize)]
struct RefusalDto {
    error: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    surfaced: Option<&'static str>,
}

impl ActivityDto {
    /// What the server is filling right now, as the browser is told it.
    pub fn of(activity: Option<Activity>) -> Self {
        Self {
            fill: activity.as_ref().map(FillDto::of),
        }
    }
}

impl FillDto {
    fn of(activity: &Activity) -> Self {
        Self {
            folder: activity.folder.as_str().to_owned(),
            status: activity.status.as_str(),
            total: activity.total,
            done: activity.done,
            declined: activity.declined.iter().map(DeclinedDto::of).collect(),
            stopped: activity.stopped.as_ref().map(RefusalDto::of),
        }
    }
}

impl DeclinedDto {
    fn of(declined: &Declined) -> Self {
        Self {
            path: declined.path.clone(),
            refusal: RefusalDto::of(&declined.refusal),
        }
    }
}

impl RefusalDto {
    fn of(refusal: &Reported) -> Self {
        Self {
            error: refusal.kind,
            message: refusal.message.clone(),
            reason: refusal.reason,
            surfaced: refusal.surfaced,
        }
    }
}

/// `GET /api/activity`
///
/// Polled while something is happening and not otherwise: an explorer with
/// nothing in flight asks for nothing.
pub async fn activity(State(state): State<Arc<ServerState>>) -> Json<ActivityDto> {
    Json(ActivityDto::of(state.fills.activity()))
}
