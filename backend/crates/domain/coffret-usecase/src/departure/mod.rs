//! What becomes of a file this device materialized once its Entry has left the
//! Library (spec: EP-15).
//!
//! Two runs reach that decision. A [`sync`](crate::sync) reaches it for every
//! departed file its scan finds — whichever device removed the Entry — and a
//! [`delete`](crate::delete) reaches it for this device's own copies of the
//! Entries it has just deleted, right after its commit, so that the person who
//! asked for the deletion does not see those copies standing in the folder
//! until the next sync. It is one rule, so it is written once here, and both
//! runs ask the same questions in the same words:
//!
//! - whether the file is **unedited** — still holding what this device last
//!   made it match — which is [`unedited`];
//! - discarding an unedited one into the desktop's trash and forgetting its
//!   row, which is [`discard_departed`];
//! - and removing the folders those discards emptied, which is
//!   [`EmptiedFolders`].
//!
//! The sync finds its departed files by walking the mapped folders; a deletion
//! knows the Entry Paths and asks for the one file at each,
//! [`mapped_source`], which reads the folder the way the walk does and answers
//! with the file the walk would have found there.

mod departed;
pub(crate) use departed::Departed;

mod emptied_folders;
pub(crate) use emptied_folders::EmptiedFolders;

mod mapped_source;
pub(crate) use mapped_source::{mapped_file, mapped_source, MappedFile};

mod discard_departed;
pub(crate) use discard_departed::discard_departed;

mod unedited;
pub(crate) use unedited::unedited;
