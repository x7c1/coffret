use coffret_model::Redacted;

use super::IndexError;

impl Redacted for IndexError {
    /// Which refusal it is, which operation met it, and nothing the catalog
    /// holds.
    ///
    /// The one number that does travel is
    /// [`UnrepresentableValue`](Self::UnrepresentableValue)'s: a reader of the
    /// log who is not told which value was refused cannot tell a build with a
    /// narrow column from a Library that really holds one.
    ///
    /// The two boxed causes stop here rather than going underneath. What they
    /// carry is the Index store's own answer — SQLite's, in the shipped build —
    /// and a store that names the file it could not read is naming a local
    /// path. The `operation` is what a reader of the log needs from those two
    /// anyway: it says which statement was running, which is the half this
    /// crate put there.
    fn redacted(&self) -> String {
        match self {
            Self::PendingRowsBusy { .. } => "Index::PendingRowsBusy".to_owned(),
            Self::NoCheckpoint => "Index::NoCheckpoint".to_owned(),
            Self::DuplicatePath { path } => {
                format!("Index::DuplicatePath(path_len={})", path.as_str().len())
            }
            Self::DuplicateContainer { container_id } => {
                format!("Index::DuplicateContainer(container={container_id})")
            }
            Self::UnknownContainer { container_id } => {
                format!("Index::UnknownContainer(container={container_id})")
            }
            Self::UnrepresentablePath { operation, .. } => {
                format!("Index::UnrepresentablePath(operation={operation})")
            }
            Self::UnrepresentableValue {
                operation,
                column,
                value,
            } => format!(
                "Index::UnrepresentableValue(operation={operation}, column={column}, \
                 value={value})"
            ),
            Self::UnsupportedSchema { found, supported } => {
                format!("Index::UnsupportedSchema(found={found}, supported={supported})")
            }
            Self::UnreadableCatalog { operation, .. } => {
                format!("Index::UnreadableCatalog(operation={operation})")
            }
            Self::Backend { operation, .. } => {
                format!("Index::Backend(operation={operation})")
            }
        }
    }
}
