use std::error;
use std::fmt;

use coffret_model::{EntryPath, Redacted};

use crate::index_error::RenameRule;
use crate::rename::rename_pair::RenamePair;

/// Why a rename was refused before anything was written (spec: FM-15, EP-5).
///
/// Every variant is a state of the Library or of what was asked that a person
/// can see and put right, and none of them committed or uploaded anything: the
/// refusal comes out of the plan, before the commit is asked for (spec: CP-1).
///
/// The paths are in the values and in the message, which is a response to
/// whoever asked for the rename; [`redacted`](Redacted::redacted) carries their
/// lengths alone, since an Entry Path may not reach a diagnostic event
/// (spec: EL-1).
#[derive(Debug, Clone)]
pub enum RenameRefusal {
    /// The request named no `(from, to)` at all.
    Nothing,
    /// Two of the pairs one request names overlap: one source named with two
    /// destinations, two pairs with one destination, or a destination under
    /// another pair's destination. A source under another pair's source is
    /// not among them: it is one move of the folder (spec: FM-15, EP-18).
    Overlapping {
        /// One of the two pairs: where a destination is under another's, the
        /// one whose destination is under.
        one: RenamePair,
        /// The other: where a destination is under another's, the one whose
        /// destination it is under.
        other: RenamePair,
    },
    /// The Library holds no current Entry at the source path or under it.
    NothingAt {
        /// The source path.
        path: EntryPath,
    },
    /// The destination is the source itself.
    SamePath,
    /// The source or the destination lies in the namespace coffret reserves
    /// for derived Entries, which this version does not rename and into which
    /// no Entry is renamed (spec: FM-15).
    ReservedNamespace {
        /// The side that lies there.
        path: EntryPath,
    },
    /// A folder renamed to a path under itself.
    IntoItself,
    /// A destination is already held by an Entry the rename does not rename
    /// (spec: EP-5).
    DestinationTaken {
        /// The destination it holds.
        path: EntryPath,
    },
    /// The rename would leave one path both a file and a folder: a destination
    /// under the path of an Entry the rename does not rename, or a destination
    /// with such an Entry under it (spec: EP-5, EP-4).
    FileAndFolder {
        /// The path that would be both.
        path: EntryPath,
    },
    /// One of the renames breaks a rule FM-15 holds a record's renames to
    /// against the catalog it is applied to.
    Rule(RenameRule),
}

impl fmt::Display for RenameRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nothing => f.write_str("nothing was named to rename"),
            Self::Overlapping { one, other } if one.from == other.from => write!(
                f,
                "\"{}\" is named to be renamed to both \"{}\" and \"{}\"",
                one.from, one.to, other.to
            ),
            Self::Overlapping { one, other } if one.to == other.to => write!(
                f,
                "\"{}\" and \"{}\" are both named to be renamed to \"{}\"",
                one.from, other.from, one.to
            ),
            Self::Overlapping { one, other } => write!(
                f,
                "\"{}\" would be renamed to \"{}\", inside \"{}\", which \"{}\" is renamed to",
                one.from, one.to, other.to, other.from
            ),
            Self::NothingAt { path } => {
                write!(f, "the Library holds no Entry at or under \"{path}\"")
            }
            Self::SamePath => f.write_str("the new Entry Path is the one it already has"),
            Self::ReservedNamespace { path } => write!(
                f,
                "\"{path}\" lies under {}, which coffret keeps for derived Entries",
                EntryPath::DERIVED_COMPONENT
            ),
            Self::IntoItself => f.write_str("a folder cannot be renamed to a path under itself"),
            Self::DestinationTaken { path } => {
                write!(f, "another Entry already stands at \"{path}\"")
            }
            Self::FileAndFolder { path } => write!(
                f,
                "the rename would make \"{path}\" both a file and a folder"
            ),
            Self::Rule(rule) => write!(f, "the rename is not one the Library admits: {rule}"),
        }
    }
}

// Nothing underneath any of them: each is the plan's own reading of the
// catalog and of what was asked.
impl error::Error for RenameRefusal {}

impl Redacted for RenameRefusal {
    /// The variant and the length of the path it names, never the path.
    fn redacted(&self) -> String {
        match self {
            Self::Nothing => "Nothing".to_owned(),
            Self::Overlapping { one, other } => format!(
                "Overlapping(one={}, other={})",
                one.redacted(),
                other.redacted()
            ),
            Self::NothingAt { path } => format!("NothingAt(path_len={})", path.as_str().len()),
            Self::SamePath => "SamePath".to_owned(),
            Self::ReservedNamespace { path } => {
                format!("ReservedNamespace(path_len={})", path.as_str().len())
            }
            Self::IntoItself => "IntoItself".to_owned(),
            Self::DestinationTaken { path } => {
                format!("DestinationTaken(path_len={})", path.as_str().len())
            }
            Self::FileAndFolder { path } => {
                format!("FileAndFolder(path_len={})", path.as_str().len())
            }
            Self::Rule(rule) => format!("Rule({})", rule.redacted()),
        }
    }
}
