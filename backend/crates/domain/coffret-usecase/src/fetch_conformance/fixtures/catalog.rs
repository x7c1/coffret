use std::path::Path;

use coffret_model::EntryLocation;

use crate::device_state::{Mapping, RootMarkerId};
use crate::entry_paths::entry_path;
use crate::in_memory_fs::InMemoryFs;
use crate::index::Index;

/// The identity every mapped root in this suite is registered under
/// (spec: EP-13).
///
/// One value for every root, because no case here is about telling two roots
/// apart: what each of them needs is to *be* registered, since a placement into
/// an unregistered root is refused before the case's own question is reached.
const REGISTERED: [u8; RootMarkerId::BYTE_LEN] = [0x2a; RootMarkerId::BYTE_LEN];

/// Maps a device's folder onto the Library at `prefix`, registered the way
/// recording a mapping registers one (spec: EP-9, EP-13).
///
/// The marker is planted in the same fake the flows reach the folder through, so
/// the mapping and the folder agree: a fetch holds the root's marker against
/// what the mapping records before it places anything, and a root nobody
/// registered is a root nothing goes into. Planted for the *source* device as
/// well, at no cost — a scan steps over the management area by name
/// (spec: EP-14), so the folder it walks is the folder it walked before.
pub(crate) async fn map(
    index: &dyn Index,
    fs: &InMemoryFs,
    prefix: Option<&str>,
    local_root: &Path,
) {
    let id = RootMarkerId::from_bytes(REGISTERED);
    fs.plant_marker(local_root, &id);
    index
        .set_mapping(Mapping::new(prefix.map(entry_path), local_root.to_path_buf()).expecting(id))
        .await
        .expect("recording a mapping must succeed");
}

/// Where the current Entry at one path lives, which the case expects to exist.
pub(crate) async fn entry_at(index: &dyn Index, path: &str) -> EntryLocation {
    index
        .entry_at(&entry_path(path))
        .await
        .expect("asking a catalog for a path must succeed")
        .unwrap_or_else(|| panic!("{path:?} must be a current Entry"))
}
