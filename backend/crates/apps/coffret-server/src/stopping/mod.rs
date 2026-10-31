//! A person asking a run to stop, and the run answering.
//!
//! An add, a freeze and a fill can be stopped from the Activity panel; a
//! deletion and a move cannot (spec: LA-34). What a stop is, is a request: the
//! route that takes it marks the run asked and answers at once, and the run
//! answers it by ending — `cancelled` where it ended before it began
//! committing, and its real result otherwise (spec: LA-35).
//!
//! The mark lives on the run on record, under the same lock the run's steps
//! are written through, so the one question that decides a stop — has this run
//! begun committing? — is answered by one reading: the step that says the run
//! is committing and a stop that arrives are never both taken.

mod stop;
pub use stop::Stop;
