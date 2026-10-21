use serde::Deserialize;

/// What a drop by path asks to have added: the folder, whether it goes in as a
/// Pack, the drop handle, and which of the held files go in under what names.
///
/// Every field is named, and a body carrying any other is refused as a whole:
/// there is no field here a path on this device could travel in, and a body
/// that tries to add one is not one this route reads (spec: LA-27).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    /// The folder of the Library, spelled as every route here spells one, and
    /// absent or empty for the Library root.
    pub(super) path: Option<String>,
    /// Whether the person chose to add the drop as a Pack (spec: PK-17), as
    /// [`UploadQuery`](super::super::UploadQuery) says it for a browser's drop.
    #[serde(default)]
    pub(super) freeze: bool,
    /// The one-time handle the drop was held under.
    pub(super) handle: String,
    /// The files to add, in the order they are added.
    pub(super) files: Vec<ImportedFile>,
}

/// One held file, and the name it is to have.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ImportedFile {
    /// Its position in the held set, as the page was told it.
    pub(super) index: usize,
    /// Its path relative to the folder, as it is to stand in the Library —
    /// what the page renumbered it to, where it did.
    pub(super) name: String,
}
