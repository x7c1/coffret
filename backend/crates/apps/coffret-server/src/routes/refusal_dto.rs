use serde::Serialize;

use crate::reported::Reported;

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
