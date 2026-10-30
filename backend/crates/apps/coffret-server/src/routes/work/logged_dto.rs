use std::time::SystemTime;

use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::activity_log::{Ended, Logged};
use crate::delete::DeleteReport;
use crate::latest::Latest;
use crate::routes::RefusalDto;

use super::{DeleteDto, FillDto, FreezeDto, SyncDto};

/// One entry of the server's log of finished work, as a page is told it.
#[derive(Serialize)]
pub(super) struct LoggedDto {
    /// Which entry this is, counted from 1 in this process.
    id: u64,
    /// When the server logged it, as ISO 8601 in UTC: which zone to show it in
    /// is the browser's, which knows the reader's.
    at: String,
    /// Whether a later run of the same work — for a refused file, the same
    /// file asked for again — has taken this one's place. A failure that is
    /// settled is history and offers no second attempt.
    settled: bool,
    /// What ended, by kind.
    #[serde(flatten)]
    ended: EndedDto,
}

/// What one entry is about, in the shapes the work answer already tells each
/// flow in: a run that ended is the run as the work answer would have said it
/// the moment it ended, with nothing of the queue around it, so a page words
/// it with what it words the running one with.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum EndedDto {
    Fill { run: FillDto },
    Sync { run: SyncDto },
    Freeze { run: FreezeDto },
    Delete { run: DeleteDto },
    Discarded { queue: &'static str, folder: String },
    Fetch { path: String, refused: RefusalDto },
}

impl LoggedDto {
    pub(super) fn of(logged: &Logged) -> Self {
        Self {
            id: logged.id,
            at: iso8601(logged.at),
            settled: logged.settled,
            ended: EndedDto::of(&logged.ended),
        }
    }
}

impl EndedDto {
    fn of(ended: &Ended) -> Self {
        match ended {
            // A run that has ended waits for nobody's turn, which is what the
            // turn handed in here says.
            Ended::Fill(run) => Self::Fill {
                run: FillDto::of(&Latest::alone(run.clone())),
            },
            Ended::Sync(run) => Self::Sync {
                run: SyncDto::of(run, None),
            },
            Ended::Freeze(run) => Self::Freeze {
                run: FreezeDto::of(&Latest::alone(run.clone()), None),
            },
            Ended::Delete(run) => Self::Delete {
                run: DeleteDto::of(
                    &DeleteReport {
                        on_record: run.clone(),
                        waiting: 0,
                    },
                    None,
                ),
            },
            Ended::Discarded { queue, folder } => Self::Discarded {
                queue: queue.as_str(),
                folder: folder.as_str().to_owned(),
            },
            Ended::Fetch { path, refusal } => Self::Fetch {
                path: path.clone(),
                refused: RefusalDto::of(refusal),
            },
        }
    }
}

/// A moment of this device's clock, as ISO 8601 in UTC to the second.
///
/// A clock set before 1970 or past what a calendar states is said as the
/// epoch rather than refused: the moment is decoration beside the entry, and
/// the entry is what a page needs.
fn iso8601(at: SystemTime) -> String {
    let seconds = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    i64::try_from(seconds)
        .ok()
        .and_then(|seconds| OffsetDateTime::from_unix_timestamp(seconds).ok())
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}
