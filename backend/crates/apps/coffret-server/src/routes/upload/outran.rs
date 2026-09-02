use tracing::warn;

use crate::api_error::ApiError;

/// A budget the request passed, said to the browser and said to the log.
///
/// Both, because either may be the only one that arrives. The answer goes out in
/// the middle of a request the browser is still sending and may reach nothing
/// that reads it; and the refusal is about the request's own shape rather than
/// about a failure underneath it, so it carries no cause for the answer to put
/// in the log the way the `413` the extractor raises carries its own. What goes
/// in is the sentence and nothing else — no name of anything the drop was
/// carrying (spec: EP-1).
pub(super) fn outran(defect: &'static str) -> ApiError {
    warn!(
        operation = "upload",
        defect, "a drop passed what this server takes in one request and was stopped",
    );
    ApiError::too_large(defect)
}
