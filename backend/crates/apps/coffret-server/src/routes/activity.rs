use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::fill::{Activity, Declined};
use crate::reported::Reported;
use crate::state::ServerState;
use crate::sync::{Noted, SyncActivity};

/// What the server is doing on its own, which is two things.
///
/// A fill and a sync. Everything else this server does it does because a request
/// asked it to, and a request is answered rather than reported on; these two are
/// the work nobody asked for — the rest of a folder being brought over behind a
/// reader, and files somebody dropped being carried into the Library — so they
/// are what there is to tell a browser about.
///
/// Side by side and not one after the other: they are separate work over one
/// Library, either can be running without the other, and a browser reads each on
/// its own.
#[derive(Serialize)]
pub struct ActivityDto {
    /// The latest fill, running or finished, and `null` where none has run.
    fill: Option<FillDto>,
    /// The latest sync, running or finished, and `null` where none has run.
    sync: Option<SyncDto>,
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
struct SyncDto {
    /// `syncing`, `done` or `stopped`.
    status: &'static str,
    /// How many files the run carried into the Library, and `0` until it is
    /// over.
    added: usize,
    /// What the run found and did not act on — a file inside a Pack it cannot
    /// replace, a file this device no longer has, a mapped root it could not
    /// vouch for.
    noted: Vec<NotedDto>,
    /// What stopped the sync, and `null` where nothing did.
    stopped: Option<RefusalDto>,
}

/// One thing a sync that succeeded still has to say.
///
/// Unlike a declined Entry this carries no refusal vocabulary, and deliberately:
/// nothing was refused. The run succeeded and left this alone, so what there is
/// to show is the sentence and the row it belongs to — `null` for the findings
/// that are about no single Entry.
#[derive(Serialize)]
struct NotedDto {
    path: Option<String>,
    message: String,
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
/// declined Entry with the code it already has for a refused request. Shared
/// with the upload's per-part list for that reason: a refusal a person meets by
/// dropping a file and one they meet by opening it are one vocabulary.
#[derive(Serialize)]
pub(super) struct RefusalDto {
    error: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    surfaced: Option<&'static str>,
}

impl ActivityDto {
    /// What the server is doing right now, as the browser is told it.
    pub fn of(fill: Option<Activity>, sync: Option<SyncActivity>) -> Self {
        Self {
            fill: fill.as_ref().map(FillDto::of),
            sync: sync.as_ref().map(SyncDto::of),
        }
    }
}

impl SyncDto {
    fn of(activity: &SyncActivity) -> Self {
        Self {
            status: activity.status.as_str(),
            added: activity.added,
            noted: activity.noted.iter().map(NotedDto::of).collect(),
            stopped: activity.stopped.as_ref().map(RefusalDto::of),
        }
    }
}

impl NotedDto {
    fn of(noted: &Noted) -> Self {
        Self {
            path: noted.path.clone(),
            message: noted.message.clone(),
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
    pub(super) fn of(refusal: &Reported) -> Self {
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
    Json(ActivityDto::of(
        state.fills.activity(),
        state.syncs.activity(),
    ))
}
