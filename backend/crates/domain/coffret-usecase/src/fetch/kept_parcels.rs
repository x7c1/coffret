use std::path::{Path, PathBuf};

use coffret_format::ParcelLen;
use coffret_model::ContainerId;

use crate::parcel_files::ParcelFiles;

/// Where this device keeps the parcels it read, and how long a parcel is.
///
/// A fetched parcel stays on the device until every Entry it covers that the
/// device maps is on disk or witnessed absent, or its Container leaves the
/// current set, and a parcel the device holds is never requested from Storage
/// again (spec: PK-21). The three things that takes travel together because
/// they are one device's arrangement rather than one run's: the capability the
/// parcel files are written and read through, the directory under the state
/// directory they live in, and the parcel length `S` every read of a
/// Container's chunks is divided by (spec: PK-19).
///
/// The length is here rather than read off a constant at the point of use
/// because the register's value is provisional, and a test that wants a Pack
/// of several parcels out of a few megabytes lowers it. Every production
/// caller passes [`PARCEL_LEN`](coffret_format::PARCEL_LEN).
#[derive(Clone, Copy)]
pub struct KeptParcels<'a> {
    /// The disk the parcel files are on.
    pub files: &'a dyn ParcelFiles,
    /// The directory they are kept in.
    ///
    /// Device state: it never travels into a Container, a Journal record, or a
    /// diagnostic event.
    pub dir: &'a Path,
    /// How much of a chunk sequence one parcel is meant to hold (spec: PK-19).
    pub len: ParcelLen,
}

impl<'a> KeptParcels<'a> {
    /// Parcels kept in `dir` on `files`, at the register's parcel length.
    pub fn new(files: &'a dyn ParcelFiles, dir: &'a Path) -> Self {
        Self {
            files,
            dir,
            len: ParcelLen::default(),
        }
    }

    /// The same parcels at a different length.
    pub fn with_len(mut self, len: ParcelLen) -> Self {
        self.len = len;
        self
    }

    /// Where parcel `index` of one Container is kept.
    ///
    /// Named by what it is, so a parcel read again lands on the file it
    /// replaces rather than beside it.
    pub(super) fn path_of(&self, container_id: ContainerId, index: u64) -> PathBuf {
        self.dir.join(format!("{container_id}-{index}.parcel"))
    }
}
