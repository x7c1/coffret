use std::sync::atomic::{AtomicU64, Ordering};

use coffret_model::ContainerId;

/// The name every temporary file a fetch writes begins with.
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
pub(crate) const PREFIX: &str = ".coffret-fetch-";

/// Whether one local filename is a fetch's scratch rather than a file to walk.
pub(crate) fn is_scratch(name: &str) -> bool {
    name.starts_with(PREFIX)
}

/// A temporary name nothing else in a destination directory is using.
///
/// The Container ID keeps two runs fetching different Containers apart, and the
/// random tail keeps two runs fetching the *same* Container apart — which is
/// what would otherwise have them writing into one file. Neither is a secret and
/// neither has to be unguessable; the name only has to be unique, so a machine
/// with no entropy to spare gets a counter mixed with the Container ID rather
/// than a failed fetch.
pub(crate) fn name(container_id: ContainerId) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);

    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        bytes = NEXT.fetch_add(1, Ordering::Relaxed).to_be_bytes();
    }
    let tail: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{PREFIX}{container_id}-{tail}.part")
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
}
