//! The adds this server is carrying in: what each one wrote on this device,
//! so that one a person stops can be undone (spec: LA-34, LA-36).
//!
//! An **add** is one drop or one **+**: the files a request wrote into a
//! mapped folder, and the run that carries them into the Library — a sync, or
//! a freeze where the person chose a Pack (spec: PK-17). It is received by the
//! upload or the import, is then armed for its run, and ends with that run.
//! While it has not begun committing it can be stopped, and stopping it undoes
//! it on this device: what it wrote is taken away — removed, or replaced by
//! the file it had written over, which it kept aside for that — and nothing
//! else ([`undo`]).
//!
//! The registry is the process's own state, like the rest of what a server
//! holds about its work (spec: LA-12): a server started afterwards holds no
//! add, and the files an add the stopped server held had written are in the
//! folder as anything copied there is. What it kept aside of the files they
//! replaced is under a scratch name nothing holds any longer, which the next
//! sync removes (spec: OC-8).

mod add;
pub(crate) use add::Add;

mod add_id;
pub use add_id::AddId;

mod registry;
pub use registry::Adds;

mod stopped_add;
pub use stopped_add::StoppedAdd;

mod undo;
pub(crate) use undo::{log_undone, undo};

mod written;
pub(crate) use written::{Replaced, Written};
