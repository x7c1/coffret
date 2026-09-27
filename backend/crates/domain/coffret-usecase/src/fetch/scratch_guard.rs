use std::ops::{Deref, DerefMut};

use crate::fetch::decoding::Decoding;
use crate::fetch::placement::{discard_all, Placement};

/// What one fetch attempt has written to disk before it knows whether it
/// succeeded, and how that goes when it did not.
pub(super) trait Scratch {
    /// Removes it, reporting rather than raising a removal that failed: what
    /// the caller is about to report is the failure that made the removal
    /// necessary.
    fn discard(self);
}

impl Scratch for Decoding<'_, '_> {
    fn discard(self) {
        Decoding::discard(self);
    }
}

impl Scratch for Placement<'_> {
    fn discard(self) {
        discard_all(vec![self]);
    }
}

/// An attempt's scratch, discarded on drop unless the attempt disarms it.
///
/// A guard rather than a discard written at each exit, for the reason
/// [`DegradedReport`](crate::commit::DegradedReport) is one: an attempt leaves
/// by every `?` and every early return it contains, through either of its two
/// error channels — Storage's and the Library's — and each of them has to leave
/// nothing behind. So the scratch goes when this is dropped, from wherever the
/// attempt ended, and the one exit that keeps it is the one that says so:
/// [`disarm`](Self::disarm), at success.
pub(super) struct ScratchGuard<T: Scratch> {
    /// The scratch, until it is disarmed or dropped.
    ///
    /// An `Option` only so that [`disarm`](Self::disarm) and `drop` can take it
    /// out: it is `Some` for as long as anything can reach it through the
    /// guard.
    scratch: Option<T>,
}

impl<T: Scratch> ScratchGuard<T> {
    /// Holds `scratch`, armed.
    pub(super) fn armed(scratch: T) -> Self {
        Self {
            scratch: Some(scratch),
        }
    }

    /// Hands the scratch back to an attempt that succeeded, to keep.
    pub(super) fn disarm(mut self) -> T {
        self.scratch
            .take()
            .expect("a guard holds its scratch until it is disarmed or dropped")
    }
}

impl<T: Scratch> Deref for ScratchGuard<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.scratch
            .as_ref()
            .expect("a guard holds its scratch until it is disarmed or dropped")
    }
}

impl<T: Scratch> DerefMut for ScratchGuard<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.scratch
            .as_mut()
            .expect("a guard holds its scratch until it is disarmed or dropped")
    }
}

impl<T: Scratch> Drop for ScratchGuard<T> {
    fn drop(&mut self) {
        if let Some(scratch) = self.scratch.take() {
            scratch.discard();
        }
    }
}
