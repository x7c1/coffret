//! How much of an answer this gateway is willing to take into memory.
//!
//! Every answer that is *not* a Storage Object's bytes is a small structured
//! document — a file resource, a page of a listing, a minted-id set, an OAuth
//! token, an error envelope — and reading one means holding it. How many bytes
//! that costs is otherwise decided by whatever answered: a provider having a bad
//! day, a proxy on the path, or something standing in for Drive entirely. None
//! of them is inside the trust boundary, and none of the documents here carries
//! anything this build could authenticate before parsing it.
//!
//! So the caller says how much of an answer it will take before the answer
//! arrives, and the ceiling comes from the document's own shape: the fields the
//! request asked Drive for, and how many of them one answer can carry. They live
//! here rather than beside the client that performs the call for the reason the
//! format's ceilings live beside the payload schemas — the size of a
//! `files.list` page is a fact about `files.list`, not about HTTP.
//!
//! A Storage Object's bytes are the exception, and they are not bounded here:
//! they are as large as the files they carry, they arrive with a length Drive
//! declares, and what they are held against is the port's own reckoning of what
//! the caller asked for.

/// One JSON document: a file resource, a set of minted identifiers, an OAuth
/// token response, or an error envelope.
///
/// The fields these calls ask for are counted out by name — an id, a name, an
/// MD5 — so a real one is a few hundred bytes, and an access token pushes its
/// response to a couple of kilobytes. The ceiling is three orders of magnitude
/// above that on purpose: it is not a budget for the documents this build reads
/// but the point past which an answer is not one of them at all. A refusal at a
/// megabyte can only be something that is not Drive answering, or a proxy's
/// error page grown past anything worth reading.
pub(crate) const MAX_DOCUMENT_LEN: u64 = 1024 * 1024;

/// One page of `files.list`.
///
/// The one answer that grows with the Library rather than with a single file,
/// and it grows only as far as the page size lets it: Drive caps `pageSize` at
/// 1000, and each element carries the three fields
/// [`LIST_FIELDS`](crate::api::LIST_FIELDS) names. At a few hundred bytes an
/// element that is well under a megabyte, and the headroom above it is for the
/// names Drive itself would allow rather than for the ones coffret writes, which
/// are a Container id or a generation.
pub(crate) const MAX_LISTING_PAGE_LEN: u64 = 16 * 1024 * 1024;
