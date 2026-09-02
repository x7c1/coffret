use serde::Deserialize;

/// The `?path=` and `?freeze=` a drop was asked with.
///
/// The folder is spelled the way every route here spells one, for the reason
/// [`PathQuery`](crate::entry_query::PathQuery) gives; `freeze` is the one
/// parameter no other route takes.
///
/// It says which of the two gestures this drop is, and it is stated rather than
/// worked out here. The browser is the half that knows: a drop onto a folder the
/// person made in it a moment ago is a book being brought in, and a drop onto a
/// folder the Library already had is files being added to it. From the server
/// the two look identical — an empty folder is an empty folder, whoever made it —
/// so guessing would mean packing whatever happened to be dropped onto a folder
/// somebody had just emptied.
///
/// Absent is the ordinary drop, so every caller that is not importing a book
/// leaves it out.
#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    pub(super) path: Option<String>,
    #[serde(default)]
    pub(super) freeze: bool,
}
