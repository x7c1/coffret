//! The name a scratch inside a mapped folder is called by.
//!
//! One prefix, shared by everything that writes into a folder a scan walks, and
//! stepped over by the scan itself. What each writer does with it is its own
//! business; that every one of them uses it is what keeps an interrupted write
//! from becoming an Entry.
//!
//! And what keeps an interrupted write from staying forever: a name handed out
//! here is held for as long as its writer lives, so a sync that meets a scratch
//! of coffret's own shape that nothing in this process holds knows it was left
//! by a writer that is gone, and removes it (spec: OC-8, EP-11).

use std::sync::atomic::{AtomicU64, Ordering};

use coffret_model::{lowercase_hex, ContainerId};

mod scratch_name;
pub use scratch_name::ScratchName;

/// The name every scratch written inside a mapped folder begins with.
///
/// A fetch writes into the destination directory and renames, because that is
/// the only way a verified file becomes visible at its final path without a
/// reader ever seeing a partial one (spec: EP-11). The destination directory is
/// also a mapped folder, so a run killed between the write and the rename leaves
/// a file inside the very folder a later sync walks — and a sync that took it for
/// user data would commit a Library Entry out of coffret's own scratch.
///
/// So the two flows agree on one prefix: a fetch only ever writes scratches
/// whose names begin with it, and a scan passes over every name that does
/// (spec: EP-8, EP-11). Coffret owns that prefix inside a mapped folder;
/// anything of the user's own carrying it is not backed up — a file, or a folder
/// and everything under it, since the walk stops at the name — which is the
/// trade for a crash never inventing an Entry.
///
/// The reservation serves a second writer as well as the fetch: a file arriving
/// from outside — the explorer adding a dropped file to a mapped folder — is
/// written and renamed exactly like a fetched one, for exactly the same reason,
/// so it takes its scratch names from here too ([`incoming_name`]). A writer
/// joining them adds a function below and never a second prefix: a second one is
/// a name the scan does not know to step over.
pub(crate) const PREFIX: &str = ".coffret-fetch-";

/// Whether one local filename is coffret's own scratch rather than a file to
/// walk.
pub fn is_scratch(name: &str) -> bool {
    name.starts_with(PREFIX)
}

/// What every scratch name ends with.
const SUFFIX: &str = ".part";

/// What an incoming file's scratch name says in place of a Container ID.
const INCOMING: &str = "incoming-";

/// Whether one local filename is a scratch of the exact shape coffret's writers
/// name them by, rather than merely a name carrying the reserved prefix.
///
/// The prefix reserves a name from the scan; it does not make a file coffret's
/// to remove. A person's own `.coffret-fetch-notes.txt` is passed over by the
/// scan and is still theirs, so a removal asks for the whole shape — the prefix,
/// a Container ID or `incoming-`, a tail, and the suffix — and leaves
/// anything else that carries the prefix where it is (spec: OC-8).
pub fn is_own_scratch(name: &str) -> bool {
    let Some(middle) = name
        .strip_prefix(PREFIX)
        .and_then(|rest| rest.strip_suffix(SUFFIX))
    else {
        return false;
    };
    let tail = match middle.strip_prefix(INCOMING) {
        Some(tail) => tail,
        None => match middle.split_once('-') {
            Some((container_id, tail)) if is_hex(container_id, CONTAINER_ID_HEX) => tail,
            _ => return false,
        },
    };
    is_hex(tail, TAIL_HEX)
}

/// How many hex digits a Container ID is spelled with (spec: FM-3).
const CONTAINER_ID_HEX: usize = 32;

/// How many hex digits a [`tail`] is spelled with.
const TAIL_HEX: usize = 16;

/// Whether `text` is exactly `len` lowercase hex digits.
fn is_hex(text: &str, len: usize) -> bool {
    text.len() == len
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether a scratch of coffret's own shape was left by a writer that is gone,
/// as far as this process can tell.
///
/// Every name this module hands out is held from that moment until its
/// [`ScratchName`] drops, which a writer makes sure happens only after its
/// scratch was renamed or removed. A name is unique and never handed out
/// twice, so one nothing holds now can never be held later: a scratch under it
/// is either already gone or nobody's in this process (spec: OC-8, EP-11).
/// What another process is writing this cannot see, which is why the removal
/// itself asks the operating system as well
/// ([`LocalTrash::remove_scratch`](crate::LocalTrash::remove_scratch)).
pub fn is_stale(name: &str) -> bool {
    is_own_scratch(name) && !ScratchName::is_held(name)
}

/// A scratch name nothing else in a destination directory is using.
///
/// The Container ID keeps two runs fetching different Containers apart, and the
/// random tail keeps two runs fetching the *same* Container apart — which is
/// what would otherwise have them writing into one file. Neither is a secret and
/// neither has to be unguessable; the name only has to be unique.
pub(crate) fn name(container_id: ContainerId) -> ScratchName {
    ScratchName::held(format!("{PREFIX}{container_id}-{}{SUFFIX}", tail()))
}

/// A scratch name for a file arriving from outside the Library.
///
/// There is no Container to name it after — the file has never been in one, and
/// the point of writing it is that a later sync makes it one — so the unique tail
/// is the whole of the name. Two writers taking the same filename into one folder
/// therefore still write two scratches, and the second rename is what
/// decides which of them ends up standing there.
pub fn incoming_name() -> ScratchName {
    ScratchName::held(format!("{PREFIX}{INCOMING}{}{SUFFIX}", tail()))
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
    lowercase_hex::encode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scratch_name_is_recognized_as_one() {
        let container_id = coffret_format::generate_container_id().expect("the OS CSPRNG");
        assert!(is_scratch(name(container_id).as_str()));
        assert!(!is_scratch("spring.jpg"));
        assert!(!is_scratch(".hidden"));
    }

    #[test]
    fn two_names_for_one_container_differ() {
        let container_id = coffret_format::generate_container_id().expect("the OS CSPRNG");
        assert_ne!(name(container_id).as_str(), name(container_id).as_str());
    }

    // The second writer's names are the scan's to step over for the reason the
    // fetch's are: a drop interrupted halfway leaves one of these inside a
    // mapped folder, and a scan that read it would commit half a file.
    #[test]
    fn an_incoming_files_scratch_is_scratch_too() {
        assert!(is_scratch(incoming_name().as_str()));
        assert_ne!(incoming_name().as_str(), incoming_name().as_str());
    }

    // OC-8: only the shape the writers name scratches by is coffret's to
    // remove. A name that merely carries the prefix is reserved from the scan
    // and is still not coffret's file.
    #[test]
    fn only_the_writers_own_shape_is_an_own_scratch() {
        let container_id = coffret_format::generate_container_id().expect("the OS CSPRNG");
        assert!(is_own_scratch(name(container_id).as_str()));
        assert!(is_own_scratch(incoming_name().as_str()));
        for resembling in [
            ".coffret-fetch-notes.txt",
            ".coffret-fetch-incoming-.part",
            ".coffret-fetch-incoming-0123456789abcdef",
            ".coffret-fetch-incoming-0123456789ABCDEF.part",
            ".coffret-fetch-incoming-0123456789abcdef0.part",
            ".coffret-fetch-0123456789abcdef-0123456789abcdef.part",
            ".coffret-fetch-album",
            "coffret-fetch-incoming-0123456789abcdef.part",
        ] {
            assert!(!is_own_scratch(resembling), "{resembling}");
        }
    }

    // OC-8, EP-11: a name is held from the moment it is handed out until its
    // writer lets it go, so a scratch being written is never stale and one its
    // writer left behind is.
    #[test]
    fn a_name_is_stale_only_once_its_writer_let_it_go() {
        let held = incoming_name();
        let spelled = held.as_str().to_owned();
        assert!(!is_stale(&spelled), "a live writer holds it");
        drop(held);
        assert!(is_stale(&spelled), "nothing holds it any longer");
        assert!(!is_stale(".coffret-fetch-notes.txt"), "never coffret's");
    }
}
