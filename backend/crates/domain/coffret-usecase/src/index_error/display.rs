use std::fmt;

use super::IndexError;

/// A refused Index may hold the only copy of device-local records. Rebuilding
/// the Catalog cannot replace them, even when mappings remain readable.
const RECOVERY: &str = "keep the Index file and its spools intact; use a compatible build or a \
                        migration that preserves device-local records. Storage can rebuild \
                        only the Catalog, not mappings, materialization records, or pending work";

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PendingRowsBusy { .. } => {
                f.write_str("another import owns this device's pending rows; wait for it to finish")
            }
            Self::NoCheckpoint => f.write_str("the Index stands at no committed Library state"),
            // The Entry Path is what identifies the conflict, so the message
            // carries it — which is why a diagnostic event renders this
            // through [`Redacted`] instead: an Entry Path never belongs in one.
            Self::DuplicatePath { path } => {
                write!(f, "two Entries claim the Entry Path {path:?}")
            }
            Self::DuplicateContainer { container_id } => {
                write!(f, "Container {container_id} is added twice")
            }
            Self::UnknownContainer { container_id } => {
                write!(f, "no current Container {container_id} to hold this Entry")
            }
            // The path stays out of the message, and stays in the value: see
            // the variant.
            Self::UnrepresentablePath { operation, .. } => write!(
                f,
                "a local path given while {operation} is not one this catalog can keep: \
                 it is not valid UTF-8"
            ),
            // The number goes into the message, where a local path stays out of
            // it: see the variant.
            Self::UnrepresentableValue {
                operation,
                column,
                value,
            } => write!(
                f,
                "the value {value} given while {operation} is past what this catalog's \
                 {column} column can hold"
            ),
            // Which of the two refusals it is, said in words: the pair of
            // versions already distinguishes them, and a reader deciding what
            // to do should not have to compare two numbers to find out that a
            // build older than their file is a different situation from a file
            // older than their build.
            Self::UnsupportedSchema { found, supported } if found < supported => write!(
                f,
                "the Index file is at schema version {found}, an older layout than the \
                 version {supported} this build can carry forward: {RECOVERY}"
            ),
            Self::UnsupportedSchema { found, supported } => write!(
                f,
                "the Index file is at schema version {found}, newer than the version \
                 {supported} this build reads: use the build that wrote it. Otherwise, \
                 {RECOVERY}"
            ),
            // The operation is what this layer knows and the store below it
            // does not; what the store or the reader answered is left to
            // `cause`, which `source` hands on, so a caller printing the whole
            // chain reads each part once rather than twice.
            Self::UnreadableCatalog { operation, .. } => {
                write!(
                    f,
                    "the Index file holds something this build cannot read while {operation}"
                )
            }
            Self::Backend { operation, .. } => {
                write!(f, "the Index store failed while {operation}")
            }
        }
    }
}
