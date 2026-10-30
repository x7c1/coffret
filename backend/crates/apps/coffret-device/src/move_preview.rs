use coffret_model::EntryPath;
use coffret_usecase::rename::RenamePreview;

/// What moving a set of files and folders would do, as
/// [`preview_move`](crate::OpenLibrary::preview_move) works it out: the
/// rename's own preview, and the files under the set that the Library does not
/// hold yet.
///
/// A move is refused where the rename is, and also where any of this device's
/// files in the set is edited, or where anything under it is still on its way
/// into the Library: a file that cannot follow its Entry leaves this device
/// (spec: EP-10), and what the Library does not hold would be lost with it.
#[derive(Debug, Clone, Default)]
pub struct MovePreview {
    /// The rename's plan, and what this device's files of the renamed Entries
    /// would do (spec: FM-15, EP-10).
    pub rename: RenamePreview,
    /// The files under the set that this device has and the Library does not
    /// hold yet — dropped and not yet committed, or reserved for a Pack a
    /// freeze has not built (spec: PK-17) — in Entry Path order.
    pub uploading: Vec<EntryPath>,
}
