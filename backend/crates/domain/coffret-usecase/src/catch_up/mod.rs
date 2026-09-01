//! Learning what the Library has become, without bringing any of it over.
//!
//! A device that joins a Library, and one whose Library another device has since
//! written to, learns about the new state one way: by replaying the Journal
//! records its Index has not seen (spec: CK-9). Every flow that touches the
//! Library already starts by doing that, because none of them may answer from a
//! catalog it knows may be stale — and until now the catch-up existed only as
//! their first step, reachable by running a whole [`sync`](crate::sync) or a
//! whole [`fetch`](crate::fetch) for it.
//!
//! This is that step on its own, for the caller that wants nothing else: a
//! reader looking at the Library who asks what is new. It is the same routine the
//! commit flow runs, so there is one reading of CK-9 and not two.
//!
//! # What it deliberately does not do
//!
//! It does not scan the mapped folders, spool anything, upload anything, fetch a
//! Container, or write a file. The catalog advances and every Entry that arrives
//! in it is `remote` (spec: EP-10) — the bytes come later, through the ordinary
//! open-and-fill path, and only for what somebody actually looks at.
//!
//! It does not read the committed Keyring either. A catch-up opens no Container,
//! so it needs no Container Key; reading the replica set would be Storage calls
//! for something nothing in the run uses, and a degraded set — which RV-2 says a
//! fetch still reads through — would turn "here is what is new" into a refusal
//! about keys. What the Keyring holds is the fetch's question, asked when there
//! is a Container to open.
//!
//! And it does not repair, prune, checkpoint, or commit. Nothing here writes to
//! Storage at all, which is what makes it the one flow that is safe to run at
//! the moment a process starts.

mod catch_up_outcome;
pub use catch_up_outcome::CatchUpOutcome;

mod catch_up_request;
pub use catch_up_request::CatchUpRequest;

mod run;
pub use run::catch_up_catalog;

// The commit flow's own vocabulary, which is what a catch-up fails in: it is the
// commit's routine, and re-drawing its distinctions here would give one verdict
// two spellings. Re-exported where the callers of this flow already reach for
// the rest of what it takes.
pub use crate::commit::{CommitError, CommitResult};
