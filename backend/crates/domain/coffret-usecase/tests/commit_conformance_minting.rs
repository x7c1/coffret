//! The commit flow's contract, run against an in-memory store that mints
//! identifiers.
//!
//! The in-memory store the suite's own run uses keys objects by name, and on
//! such a store a commit slot is exclusive however a writer comes by it: two
//! writers that derive one name collide on it. Google Drive is the other shape —
//! every reservation mints a fresh identifier, the conditional create excludes
//! by identifier alone, and names are not unique — and there the exclusion
//! holds only if every writer consumes the one slot its head carries
//! (spec: CP-2, CP-3). A backend of that shape needs an account to run against,
//! so this runs the suite over a double of it as part of an ordinary
//! `cargo test`.
//!
//! Every case runs here, the races for a Library's first commit among them.
//! What excludes those is that a Library is never without a head to carry a
//! slot: creating it writes `head-0`, carrying the identifier minted for its
//! successor, before anything else can happen to it, and every case starts
//! from a Library created that way. The first commit is therefore one more
//! successor of a head, and two writers racing for it consume the one
//! identifier `head-0` carries.
//!
//! The one shape this run does not hold is a Library created before its first
//! head was written at creation: it has no head until something commits, and
//! the first commit writes an empty `head-0` under an identifier of its own
//! before committing on top of it. Two first commits racing there can both
//! create `head-0`, and the next reader refuses the Library rather than read one
//! of the two (spec: FM-12); the commit flow's own cases cover that shape
//! rather than this suite, whose every case starts from a created Library.

use coffret_usecase::commit_conformance::CommitUnderTest;
use coffret_usecase::{InMemoryIndex, InMemoryMintingStore};

/// Small enough that the cases reach a second listing page, as in the
/// name-keyed run.
const PAGE_SIZE: usize = 3;

/// An empty Library and two empty catalogs for one case.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<CommitUnderTest> {
    Some(CommitUnderTest::new(
        Box::new(InMemoryMintingStore::new(PAGE_SIZE)),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::commit_conformance!(fixture().await);
