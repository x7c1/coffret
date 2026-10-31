//! Why the numbers one request is taken within are these numbers.
//!
//! What the three budgets are, what passing one does, and what they are
//! emphatically not a budget on, is the register's (spec: LA-9, LA-10, LA-11).
//! What is here is the half no rule should carry — why each number is the one
//! it is.
//!
//! They are not a measure of what one drop should be. A drop may be one book,
//! or several volumes of a series added at once, or a folder of photographs
//! years deep, and nothing on this route asks which: one gesture in the explorer
//! is one request, however many files it carries. What the numbers guard is the
//! loopback socket — a request that would fill the disk before anybody noticed,
//! or a `Content-Type: multipart/form-data` aimed at this port by something that
//! is not the explorer at all. Two of them are byte budgets, and they carry
//! almost the whole of that: what a request may bring in all, and what one part
//! of it may. The third, the count of parts, guards the one thing the byte
//! budgets cannot see: a request made of countless tiny parts, each of which is
//! a name to check, a scratch to stand up and a file to rename, and none of
//! which brings enough bytes to pass anything. Each is set well past what a
//! person adding their own files reaches, because what they are for is refusing
//! the absurd rather than the ambitious.
//!
//! The room question beside them (spec: LA-11) is [`space`](Allowance::space),
//! and it is here for the same reason the numbers are: nothing about it is
//! configurable either, and what a case needs of it is an answer no disk can be
//! made to give.

use std::io;
use std::path::Path;

/// The most one request may carry, framing included.
///
/// Sixty-four gibibytes. A book of a few hundred pages, scanned at a size worth
/// keeping, is single-digit gigabytes, and a shelf of them added at once is
/// tens. This is past that, and past anything a browser will ever be asked to
/// send in one `FormData`; what it stops is a request that would fill the disk
/// before it ended.
pub const MOST_PER_REQUEST: usize = 64 * 1024 * 1024 * 1024;

/// The most one part may carry.
///
/// A gibibyte, which is one file: the part is one page of a book or one
/// photograph, and the largest of either is a couple of hundred megabytes even
/// uncompressed. Nothing the explorer sends approaches this, and a part that
/// passes it is not a page.
pub const MOST_PER_PART: u64 = 1024 * 1024 * 1024;

/// The most parts one request may carry.
///
/// Sixty-five thousand five hundred and thirty-six. Not a size for a drop — a
/// series of volumes or a photo library added at once runs to tens of
/// thousands of files, and is one gesture all the same — but a guard against a
/// request of countless tiny parts, which the byte budgets above let through
/// however many there are. Past this, a request is not somebody adding their
/// files but something walking a disk into one `FormData`, or not a browser at
/// all.
pub const MOST_PARTS: usize = 65_536;

/// The most an import's body may come to.
///
/// Sixty-four mebibytes, which is a kibibyte of name for each of the
/// [`MOST_PARTS`] files one import may name. An import carries no file's bytes,
/// only a handle and an index and a name per file, so this is no budget on what
/// lands: it is the room the route reads the body into, set so that the count
/// is what bounds an import rather than the length of its names (spec: LA-28).
/// Axum's own default, a couple of mebibytes, would refuse a drop of tens of
/// thousands of files long before the count does.
pub const MOST_PER_IMPORT: usize = 64 * 1024 * 1024;

/// The budgets one request is taken within (spec: LA-9, LA-10, LA-11).
///
/// A value rather than three constants read at the point of use, for one reason:
/// a case has to be able to state what passing a budget does, and stating it
/// against the numbers below would mean sending gigabytes to say so. The binary
/// serves within [`generous`](Self::generous) and nothing else does; a case
/// names the one field it is about and takes the rest from there.
#[derive(Clone, Copy)]
pub struct Allowance {
    /// The whole request's ceiling, in bytes, framing included.
    ///
    /// Enforced by the body limit the route is mounted with rather than counted
    /// here, so a request that passes it is stopped as the bytes arrive instead
    /// of after they have all been read.
    pub request_bytes: usize,
    /// One part's ceiling, in bytes of file content.
    pub part_bytes: u64,
    /// How many parts one request may carry.
    pub parts: usize,
    /// How many bytes may still be written beside a path on this device.
    ///
    /// The volume's own answer in the binary. It is a function and not a call
    /// because a disk with nothing left on it is not something a case can
    /// arrange, and refusing a drop for want of room is exactly the behaviour
    /// that has to be stated (spec: LA-11).
    pub space: fn(&Path) -> io::Result<u64>,
}

impl Allowance {
    /// What the binary serves within.
    pub const fn generous() -> Self {
        Self {
            request_bytes: MOST_PER_REQUEST,
            part_bytes: MOST_PER_PART,
            parts: MOST_PARTS,
            space: available_beside,
        }
    }

    /// How much room the volume holding `path` still has for this caller.
    pub(crate) fn space_beside(&self, path: &Path) -> io::Result<u64> {
        (self.space)(path)
    }
}

/// What the filesystem says is left on the volume `path` is on.
///
/// `statvfs` rather than anything in the standard library, which offers no way
/// to ask: the blocks available *to this caller* (`f_bavail`, which is what is
/// left after whatever the filesystem reserves for the superuser) times the size
/// of one of them.
///
/// The path is the scratch the bytes are already going to, so the answer
/// is about the volume they will land on rather than about wherever a folder
/// name might have resolved to.
fn available_beside(path: &Path) -> io::Result<u64> {
    let volume = rustix::fs::statvfs(path)?;
    Ok(volume.f_bavail.saturating_mul(volume.f_frsize))
}
