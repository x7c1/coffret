use std::fmt;

use coffret_model::ContainerId;

/// The state rule one element of a Journal record's `renames` breaks against
/// the catalog the record is applied to (spec: FM-15, RV-5).
///
/// A record's decoder holds what one element can show on its own: its order,
/// that it names one Entry once, and that its new path is an Entry Path outside
/// the derived namespace. Whether the Entry it names is there to be renamed,
/// and whether the state the record leaves keeps one Entry per Entry Path, are
/// questions about the catalog, and these are their refusals.
///
/// None of them carries an Entry Path. The element's position in the record
/// says which rename it was, and the record holds the path; an Entry Path in an
/// error is one more place it could leak from (spec: EL-1).
#[derive(Debug, Clone, Copy)]
pub enum RenameRule {
    /// The Container the element names is among the same record's removals: a
    /// record does not rename an Entry it takes out of the Library.
    ContainerRemoved {
        /// The Container both collections name.
        container_id: ContainerId,
    },
    /// The Container the element names is not in the current set immediately
    /// before the record.
    ContainerNotCurrent {
        /// The Container the element names.
        container_id: ContainerId,
    },
    /// The Container is current, and its entry table holds no Entry at the
    /// element's entry number (spec: FM-9).
    EntryNumberOutsideTable {
        /// The Container the element names.
        container_id: ContainerId,
        /// The number its table does not reach.
        entry_number: u32,
    },
    /// The element's path is the Entry's current path already: a rename that
    /// changes nothing is not one a commit writes.
    PathUnchanged,
    /// The element names a derived Entry, which this version does not rename.
    DerivedEntry,
    /// After the record, the element's path would be held by another Entry as
    /// well — one the record neither removes nor renames away (spec: EP-5).
    PathTaken,
    /// An earlier element of the same record renames another Entry to the same
    /// path (spec: EP-5).
    PathClaimedTwice {
        /// The position of that earlier element in `renames`.
        earlier: usize,
    },
    /// One of the same record's additions places an Entry at the element's
    /// path: a record's additions are never targets of its renames, and the
    /// two together would leave two Entries there (spec: EP-5).
    PathAdded {
        /// The Container the addition brings.
        container_id: ContainerId,
    },
}

impl RenameRule {
    /// The rule's name and the values that identify it, and nothing the
    /// catalog holds: what a diagnostic event may carry (spec: EL-1).
    pub(super) fn redacted(&self) -> String {
        match self {
            Self::ContainerRemoved { container_id } => {
                format!("ContainerRemoved(container={container_id})")
            }
            Self::ContainerNotCurrent { container_id } => {
                format!("ContainerNotCurrent(container={container_id})")
            }
            Self::EntryNumberOutsideTable {
                container_id,
                entry_number,
            } => format!("EntryNumberOutsideTable(container={container_id}, entry={entry_number})"),
            Self::PathUnchanged => "PathUnchanged".to_owned(),
            Self::DerivedEntry => "DerivedEntry".to_owned(),
            Self::PathTaken => "PathTaken".to_owned(),
            Self::PathClaimedTwice { earlier } => format!("PathClaimedTwice(earlier={earlier})"),
            Self::PathAdded { container_id } => format!("PathAdded(container={container_id})"),
        }
    }
}

impl fmt::Display for RenameRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContainerRemoved { container_id } => write!(
                f,
                "its Container {container_id} is among the same record's removals"
            ),
            Self::ContainerNotCurrent { container_id } => {
                write!(f, "its Container {container_id} is not current")
            }
            Self::EntryNumberOutsideTable {
                container_id,
                entry_number,
            } => write!(
                f,
                "Container {container_id} holds no Entry numbered {entry_number}"
            ),
            Self::PathUnchanged => f.write_str("the Entry stands at that path already"),
            Self::DerivedEntry => f.write_str("the Entry is a derived Entry, which is not renamed"),
            Self::PathTaken => f.write_str("another Entry would stand at the same path"),
            Self::PathClaimedTwice { earlier } => write!(
                f,
                "rename {earlier} of the same record gives another Entry the same path"
            ),
            Self::PathAdded { container_id } => write!(
                f,
                "the same record adds an Entry at that path in Container {container_id}"
            ),
        }
    }
}
