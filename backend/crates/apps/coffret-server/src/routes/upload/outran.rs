use tracing::warn;

use crate::api_error::ApiError;

/// A budget the request passed, rendered twice: once for the log, once for
/// whoever is at the browser.
///
/// Both, because either may be the only one that arrives. The answer goes out in
/// the middle of a request the browser is still sending and may reach nothing
/// that reads it; and the refusal is about what the request carried rather than
/// about a failure underneath it, so it carries no cause for the answer to put
/// in the log the way the `413` the extractor raises carries its own.
///
/// Two renderings and not one sentence used twice, which is the whole of the
/// rule rather than its first half (spec: EL-1). `recorded` is the event's: a
/// diagnostic record, so it is the sentence and nothing else — no name of
/// anything the drop was carrying (spec: EL-1). `said` is the response, which
/// the same rule lets name the file it is about; where one named
/// file is what passed the budget, withholding the name leaves a person who
/// dropped three hundred scans to find it themselves.
///
/// Where the budget is about the request's own shape there is no one file to
/// name and the two sentences are the same, which is what [`outran`] is for.
pub(super) fn outran_as(recorded: &'static str, said: &str) -> ApiError {
    warn!(
        operation = "upload",
        defect = recorded,
        "a drop passed what this server takes in one request and was stopped",
    );
    ApiError::too_large(said)
}

/// A budget about the request's own shape, where the drop carries no one file
/// the sentence could name.
pub(super) fn outran(defect: &'static str) -> ApiError {
    outran_as(defect, defect)
}
