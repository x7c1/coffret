use std::fs;
use std::path::{Path, PathBuf};

use coffret_model::EntryPath;
use coffret_usecase::device_state::Mapping;
use coffret_usecase::Index;

use super::{index, open, root_marker};
use crate::error::{Error, Result};
use crate::marker_request::MarkerRequest;
use crate::open_library::OpenLibrary;
use crate::recorded_mapping::RecordedMapping;

/// Records that `local_root` on this device holds `prefix` of the Library, and
/// reports the mapping it replaced.
///
/// `prefix` is one top-level component of the Library, or `None` for the
/// Library root itself (spec: EP-9). Recording a prefix that is already mapped
/// replaces the root it stood for, which is what makes this the one call for
/// both mapping a folder and moving one — and why what it replaced comes back
/// rather than being dropped. Moving a mapping takes everything under the old
/// root out of the Library's reach on this device, so a caller that cannot say
/// what was there cannot tell a person what just happened.
///
/// `local_root` has to be a directory that is there now. A mapped root that has
/// gone missing is an ordinary state a scan reports when it looks
/// (spec: EP-12); a root that was never there is a typo, and recording it would
/// turn every later scan into that report.
///
/// Recording a mapping also gives the root an identity of its own, and this is
/// the only call in coffret that does: a marker file inside the root's own
/// management area holding a random identifier, and that same identifier kept as
/// what this mapping expects to find there before anything is ever placed into
/// it (spec: EP-13). `marker` says whether a root that already carries an
/// identity keeps it ([`MarkerRequest`]).
///
/// The marker is written after everything else that can refuse: the prefix, the
/// root, and the catalog are all decided first, so a refusal any of them makes
/// leaves nothing behind in somebody's folder.
pub async fn set_mapping(
    name: &str,
    prefix: Option<&str>,
    local_root: &Path,
    marker: MarkerRequest,
) -> Result<RecordedMapping> {
    let dir = open(name)?;
    let mapping = unstamped(prefix, local_root)?;
    record(&index(&dir)?, mapping, marker).await
}

impl OpenLibrary {
    /// Records a mapping in the catalog this open Library already holds, which
    /// is [`set_mapping`] for a process that has the Library open.
    ///
    /// The same body and the same refusals in the same order: what differs is
    /// only where the catalog comes from. A process that stays up — the
    /// explorer's server — reads its mappings through this catalog on every
    /// request, so recording one through it is seen by the very next listing,
    /// with no second connection to the file and nothing to restart. Nothing in
    /// it needs a key: a mapping is device state (spec: CK-7), and it takes an
    /// open Library only because that is where the catalog is.
    pub async fn set_mapping(
        &self,
        prefix: Option<&str>,
        local_root: &Path,
        marker: MarkerRequest,
    ) -> Result<RecordedMapping> {
        let mapping = unstamped(prefix, local_root)?;
        record(self.index.as_ref(), mapping, marker).await
    }
}

/// The mapping a request names, with nothing about the root's filesystem or
/// identity recorded yet — or the refusal of its prefix or its root.
///
/// Nothing stamped yet: the next scan stamps whichever filesystem it finds the
/// root standing on (spec: EP-12).
fn unstamped(prefix: Option<&str>, local_root: &Path) -> Result<Mapping> {
    Ok(Mapping::new(
        prefix.map(entry_path).transpose()?,
        existing_directory(local_root)?,
    ))
}

/// Records `mapping` in `index`, writing or adopting the marker its root is to
/// carry, and reports what it replaced.
async fn record(
    index: &dyn Index,
    mapping: Mapping,
    marker: MarkerRequest,
) -> Result<RecordedMapping> {
    // Read before the write rather than after: one prefix holds one mapping, so
    // afterwards there is nothing left to have replaced.
    let replaced = index
        .mappings()
        .await?
        .into_iter()
        .find(|recorded| recorded.prefix == mapping.prefix);

    let (expected, marked) = root_marker::register(&mapping.local_root, marker)?;

    let local_root = mapping.local_root.clone();
    index.set_mapping(mapping.expecting(expected)).await?;
    Ok(RecordedMapping {
        local_root,
        replaced,
        marker: marked,
    })
}

/// The prefix as the Library spells it, or a refusal.
///
/// Two questions in the order they are owed. Whether the text is an Entry Path
/// at all is the type's — it becomes NFC on the way in and is held to the shape
/// every Entry Path is in (spec: EP-1, EP-2) — and whether it is one a mapping
/// can stand for is this crate's: a mapping is keyed by exactly one top-level
/// component, so a path of more than one names a subtree no mapping represents
/// (spec: EP-9).
///
/// Nothing else is asked. A backslash and every other character an Entry Path
/// may carry is carried here too: this names a folder inside the Library, not a
/// directory on this device, and the shape of the one says nothing about the
/// other.
fn entry_path(prefix: &str) -> Result<EntryPath> {
    let malformed = |cause| Error::MalformedMappingPrefix {
        prefix: prefix.to_owned(),
        cause,
    };
    let path = EntryPath::parse(prefix).map_err(|cause| malformed(Some(cause)))?;
    if path.top_level() != path.as_str() {
        return Err(malformed(None));
    }
    Ok(path)
}

/// The root as one absolute path with no symlinks left in it, or a refusal.
///
/// Canonicalised because a mapping outlives the working directory the command
/// was run from, and a relative root would mean a different folder the next
/// time anything read it.
fn existing_directory(local_root: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(local_root).map_err(|cause| Error::NoSuchLocalRoot {
        path: local_root.to_path_buf(),
        cause: Some(cause),
    })?;
    if !canonical.is_dir() {
        return Err(Error::NoSuchLocalRoot {
            path: canonical,
            cause: None,
        });
    }
    Ok(canonical)
}
