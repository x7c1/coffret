use std::collections::BTreeSet;
use std::path::PathBuf;

use coffret_device::{EntryPath, ReservationId};

use crate::folder::Folder;

use super::{AddId, Written};

/// One add, as this server keeps it from its first file to its run's end.
#[derive(Clone, Debug)]
pub(crate) struct Add {
    /// What the add is called.
    pub(crate) id: AddId,
    /// The folder it was made onto.
    pub(crate) folder: Folder,
    /// Whether it is carried in by a freeze, the person having chosen a Pack
    /// (spec: PK-17), rather than by a sync.
    pub(crate) freeze: bool,
    /// Whether its request is still receiving files.
    pub(crate) receiving: bool,
    /// Whether a person asked for it to be stopped.
    pub(crate) stopping: bool,
    /// The files it wrote, in the order they landed.
    pub(crate) files: Vec<Written>,
    /// The folders on this device it made on the way, in the order made.
    pub(crate) made: Vec<PathBuf>,
    /// The reservation its drop made for its Pack, once the drop has finished
    /// receiving, and whether it took that reservation over from a drop cut
    /// off before it (spec: PK-17).
    pub(crate) reservation: Option<(ReservationId, bool)>,
    /// Every Entry Path that reservation names: what the add's freeze was
    /// asked to pack.
    pub(crate) reserved: BTreeSet<EntryPath>,
    /// The files it is writing, or wrote, over a file this device could not
    /// keep aside: while there is one, nothing could put that file back, so
    /// the add is not stopped (spec: LA-36).
    pub(crate) unkept: Vec<EntryPath>,
}
