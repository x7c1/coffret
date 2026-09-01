//! The name a half-written file inside a mapped folder is called by.
//!
//! One prefix, shared by everything that writes into a folder a scan walks, and
//! stepped over by the scan itself. What each writer does with it is its own
//! business; that every one of them uses it is what keeps an interrupted write
//! from becoming an Entry.

use std::sync::atomic::{AtomicU64, Ordering};

use coffret_model::ContainerId;

/// The name every temporary file written inside a mapped folder begins with.
///
/// A fetch writes into the destination directory and renames, because that is
/// the only way a verified file becomes visible at its final path without a
/// reader ever seeing a partial one (spec: EP-11). The destination directory is
/// also a mapped folder, so a run killed between the write and the rename leaves
/// a file inside the very folder a later sync walks — and a sync that took it for
/// user data would commit a Library Entry out of coffret's own scratch.
///
/// So the two flows agree on one prefix: a fetch only ever writes temporary
/// files whose names begin with it, and a scan passes over every name that does
/// (spec: EP-8, EP-11). Coffret owns that prefix inside a mapped folder;
/// anything of the user's own carrying it is not backed up — a file, or a folder
/// and everything under it, since the walk stops at the name — which is the
/// trade for a crash never inventing an Entry.
///
/// The reservation serves a second writer as well as the fetch: a file arriving
/// from outside — the explorer taking a dropped file into a mapped folder — is
/// written and renamed exactly like a fetched one, for exactly the same reason,
/// so it takes its temporary names from here too ([`incoming_name`]). A writer
/// joining them adds a function below and never a second prefix: a second one is
/// a name the scan does not know to step over.
pub(crate) const PREFIX: &str = ".coffret-fetch-";

/// Whether one local filename is coffret's own scratch rather than a file to
/// walk.
pub fn is_scratch(name: &str) -> bool {
    name.starts_with(PREFIX)
}

/// A temporary name nothing else in a destination directory is using.
///
/// The Container ID keeps two runs fetching different Containers apart, and the
/// random tail keeps two runs fetching the *same* Container apart — which is
/// what would otherwise have them writing into one file. Neither is a secret and
/// neither has to be unguessable; the name only has to be unique.
pub(crate) fn name(container_id: ContainerId) -> String {
    format!("{PREFIX}{container_id}-{}.part", tail())
}

/// A temporary name for a file arriving from outside the Library.
///
/// There is no Container to name it after — the file has never been in one, and
/// the point of writing it is that a later sync makes it one — so the unique tail
/// is the whole of the name. Two writers taking the same filename into one folder
/// therefore still write two temporary files, and the second rename is what
/// decides which of them ends up standing there.
pub fn incoming_name() -> String {
    format!("{PREFIX}incoming-{}.part", tail())
}

/// The part of a scratch name that makes it nobody else's.
///
/// Random where the operating system has entropy to spare and a counter where it
/// has not: the name has only to be unique, so a machine that cannot answer for
/// randomness gets a name rather than a refused write.
fn tail() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);

    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        bytes = NEXT.fetch_add(1, Ordering::Relaxed).to_be_bytes();
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scratch_name_is_recognized_as_one() {
        let container_id = coffret_format::generate_container_id().expect("the OS CSPRNG");
        assert!(is_scratch(&name(container_id)));
        assert!(!is_scratch("spring.jpg"));
        assert!(!is_scratch(".hidden"));
    }

    #[test]
    fn two_names_for_one_container_differ() {
        let container_id = coffret_format::generate_container_id().expect("the OS CSPRNG");
        assert_ne!(name(container_id), name(container_id));
    }

    // The second writer's names are the scan's to step over for the reason the
    // fetch's are: a drop interrupted halfway leaves one of these inside a
    // mapped folder, and a scan that read it would commit half a file.
    #[test]
    fn an_incoming_files_scratch_is_scratch_too() {
        assert!(is_scratch(&incoming_name()));
        assert_ne!(incoming_name(), incoming_name());
    }
}
