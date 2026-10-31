use serde::Serialize;

use crate::read_findings::Found;

use super::logged_dto::iso8601;
use super::FindingDto;

/// One thing a read that armed no fill found, as a page is told it: a finding
/// in the shape every run's findings take, numbered and stamped as an entry of
/// the log is, since there is no run whose line it could ride in (spec:
/// LA-33).
#[derive(Serialize)]
pub(super) struct FoundDto {
    /// Which finding this is, counted from 1 in this process.
    id: u64,
    /// When the server heard it, as ISO 8601 in UTC.
    at: String,
    /// What was found.
    #[serde(flatten)]
    finding: FindingDto,
}

impl FoundDto {
    pub(super) fn of(found: &Found) -> Self {
        Self {
            id: found.id,
            at: iso8601(found.at),
            finding: FindingDto::of(&found.finding),
        }
    }
}
