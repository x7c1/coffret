use std::path::Path;

use tracing::warn;

use crate::api_error::ApiError;
use crate::envelope::Envelope;

use super::refusal::Refusal;

/// Refuses the drop where this device has not the room for what is still coming.
///
/// A courtesy fence and not a quota. Nothing is reserved and nothing is
/// accounted for: the answer is what the volume said a moment ago, and something
/// else on this machine may take the room between the question and the write.
/// What it is worth is the accident it does catch — a drop far larger than the
/// disk it is aimed at, refused before it fills it rather than after.
///
/// A volume that cannot be asked at all is the server's own failure and is said
/// as one, about the request rather than about the part: a fence that opened
/// whenever it broke would be no fence.
///
/// The two numbers are kept out of the sentence and put in the log, which is the
/// only place they belong: how much room a person's disk has is theirs, and
/// whoever has to do something about a device that is filling up is at the
/// device rather than at the browser. Neither is anybody's name for anything
/// (spec: EP-1).
pub(super) fn room_for(envelope: &Envelope, scratch: &Path, coming: u64) -> Result<(), Refusal> {
    let available = envelope
        .space_beside(scratch)
        .map_err(|cause| Refusal::Request(ApiError::unreadable(cause)))?;

    match available >= coming {
        true => Ok(()),
        false => {
            warn!(
                operation = "upload",
                available,
                coming,
                "a drop was refused for want of room on the volume the mapped folder is on",
            );
            Err(Refusal::Request(ApiError::no_room()))
        }
    }
}
