use serde::Serialize;

use crate::finding::Finding;

/// One thing a run that succeeded still has to say.
///
/// Unlike a declined Entry this carries no refusal kind, and deliberately:
/// nothing was refused. The run succeeded and left this alone, so what there is
/// to show is the sentence and the row it belongs to — `null` for the findings
/// that are about no single Entry — and which finding it is, named beside the
/// sentence in the `reason` and `surfaced` a declined Entry names the same
/// state by, so that a page branching on it never has to read it out of prose.
#[derive(Serialize)]
pub(super) struct FindingDto {
    path: Option<String>,
    message: String,
    /// One of the names [`Finding::reason`] lists.
    reason: &'static str,
    /// The device layer's name for a finding about one Entry, and absent for
    /// one about a mapping or a Container — left out rather than `null`, as a
    /// refusal's is, so the two read alike.
    #[serde(skip_serializing_if = "Option::is_none")]
    surfaced: Option<&'static str>,
}

impl FindingDto {
    pub(super) fn of(finding: &Finding) -> Self {
        Self {
            path: finding.path.clone(),
            message: finding.message.clone(),
            reason: finding.reason,
            surfaced: finding.surfaced,
        }
    }
}
