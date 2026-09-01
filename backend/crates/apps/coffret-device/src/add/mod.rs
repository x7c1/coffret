//! Taking a file somebody handed this device into the folder the Library maps.
//!
//! The way into a Library has always been the same one: put a file in a mapped
//! folder, and let the next sync carry it in. This is that gesture performed by
//! a program rather than by a file manager — the explorer writing a dropped file
//! where the mappings say it goes — and it is deliberately no more than that.
//! Nothing here encrypts, uploads, or commits anything: what turns the file into
//! an Entry is [`sync`](crate::OpenLibrary::sync), unchanged, exactly as it
//! would have been had the person copied the file in themselves.
//!
//! # Where the file goes
//!
//! [`local_path_for`](coffret_usecase::fetch::local_path_for) answers that, which
//! is the same EP-9 translation a fetch makes before placing an Entry — one rule,
//! one implementation. The difference is only which question is being asked: a
//! fetch asks where an Entry the Library holds belongs, and this asks where a
//! file at a path the Library holds nothing at would go.
//!
//! # Whole or absent
//!
//! [`IncomingFile`] writes to a temporary name inside the destination directory
//! and renames it into place, so what a reader or a scan can see at the final
//! path is either nothing or the whole file (spec: EP-11). The temporary name is
//! coffret's reserved scratch prefix, which the scan already steps over
//! ([`scratch`](coffret_usecase::scratch)) — so a transfer that stops halfway
//! leaves something the next sync passes over rather than half a file it commits.
//!
//! # What is not decided here
//!
//! Whether the file *should* be written. An Entry already standing at the path,
//! inside a Pack, is a replacement this device cannot propagate (spec: PK-10,
//! PK-12), and writing it anyway would leave a file no sync can carry in — which
//! is the one state a person must never be shown. That is a caller's refusal to
//! make, out of what [`container_of`](crate::OpenLibrary::container_of) says,
//! before it opens anything.

// Where one such file is on this device, for a caller holding a path and no
// listing.
mod added_at;

mod added_file;
pub use added_file::AddedFile;

// The files under a mapped folder that no Entry of the Library stands at.
mod added_locally;

mod incoming_file;
pub use incoming_file::IncomingFile;

// Opening one, which is where EP-9 is asked and the reserved prefix is refused.
mod receive_file;
