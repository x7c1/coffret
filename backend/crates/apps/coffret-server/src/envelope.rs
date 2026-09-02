//! What one request may bring, and what this device has room for.
//!
//! The Library's storage layer is deliberately size-agnostic: a five-gigabyte
//! scan belongs in a Pack exactly as a five-hundred-kilobyte page does — an
//! Entry larger than a Pack's size target is a Pack of its own rather than a
//! file refused (spec: PK-3) — and nothing in the format or the flows puts a
//! number on a file. This is not that contract and does not weaken it. It is
//! the *server's* own, about one HTTP request from one browser on this device —
//! a boundary the Library does not have and does not want.
//!
//! # The three budgets
//!
//! They are numbers rather than a policy anybody configures, and they are chosen
//! against the largest gesture the explorer actually makes: a scanned book
//! dropped onto the folder made for it, which is one request carrying hundreds
//! of page images — and one folder's worth of them, because the freeze such a
//! drop arms considers what stands under the folder it names (spec: PK-17).
//! Every one of them is set past that and well short of anything a person
//! could mean, because what an envelope is for is refusing the absurd rather
//! than the ambitious — a request that would fill the disk before anybody
//! noticed, or a `Content-Type: multipart/form-data` aimed at this port by
//! something that is not the explorer at all.
//!
//! # And one question
//!
//! The budgets bound the request; they say nothing about whether this device can
//! hold what it is being sent. That is [`space`](Envelope::space), asked of the
//! volume the mapped folder is on before each part is taken, so a drop that
//! would run the disk out is refused while there is still room to refuse it in.
//! It is a courtesy fence and not a quota: nothing here reserves anything,
//! accounts for anything, or knows what else on this machine is writing.

use std::io;
use std::path::Path;

/// The most one request may carry, across every part of it.
///
/// Sixty-four gibibytes. A book of a few hundred pages, scanned at a size worth
/// keeping, is single-digit gigabytes; the same book at the extreme this route
/// is asked to allow — hundreds of pages at hundreds of megabytes each, which is
/// a large-format colour scan nobody has compressed — is tens of them. This is
/// past that, and past anything a browser will ever be asked to send in one
/// `FormData`.
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
/// Four thousand and ninety-six. A drop is one gesture — a handful of
/// photographs, or one book — and a long book is a thousand pages. This is past
/// any book and short of a request that is really somebody's whole disk being
/// walked into one `FormData`.
pub const MOST_PARTS: usize = 4096;

/// The bounds one drop is taken within.
///
/// A value rather than three constants read at the point of use, for one reason:
/// a case has to be able to state what exceeding a budget does, and stating it
/// against the numbers below would mean sending gigabytes to say so. The binary
/// serves within [`generous`](Self::generous) and nothing else does; a case
/// names the one field it is about and takes the rest from there.
#[derive(Clone, Copy)]
pub struct Envelope {
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
    /// that has to be stated.
    pub space: fn(&Path) -> io::Result<u64>,
}

impl Envelope {
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
/// The path is the temporary file the bytes are already going to, so the answer
/// is about the volume they will land on rather than about wherever a folder
/// name might have resolved to.
fn available_beside(path: &Path) -> io::Result<u64> {
    let volume = rustix::fs::statvfs(path)?;
    Ok(volume.f_bavail.saturating_mul(volume.f_frsize))
}
