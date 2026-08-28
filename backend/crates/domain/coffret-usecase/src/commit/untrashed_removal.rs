use coffret_model::ContainerId;

use crate::error::Error;

/// A removed Container whose object Storage would not move to the trash
/// (spec: OC-6).
///
/// The record is already the truth about which Containers are current, so this
/// un-commits nothing: what is left is an object no current state names, which a
/// later run can still reach. Which is exactly why the reason belongs in the
/// outcome. A caller finishing the job needs to know whether it is looking at a
/// provider that was busy, at credentials that do not authorize the move, or at
/// an account that is out of room — and those are three different next steps,
/// none of which can be read off a Container ID.
///
/// The `warn!` line the commit writes stays where it is: a log serves whoever is
/// watching the run, and this serves whoever is handed the outcome afterwards.
///
/// There is deliberately no `PartialEq`, for the reason the errors have none: a
/// caller decides from the variant of the cause and the fields it names.
#[derive(Debug)]
pub struct UntrashedRemoval {
    /// The Container the batch removed and this device could not trash.
    pub container_id: ContainerId,
    /// What Storage answered the trash with.
    pub cause: Error,
}
