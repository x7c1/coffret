use serde::Deserialize;

/// The `?path=` and `?freeze=` a drop was asked with.
///
/// The folder is spelled the way every route here spells one, for the reason
/// [`PathQuery`](crate::entry_query::PathQuery) gives; `freeze` is the one
/// parameter no other route takes.
///
/// It says which of the two gestures this drop is, and it is stated rather than
/// worked out here. The browser is the half that knows: it asked the person,
/// once the drop held a folder, whether that folder goes in as a Pack or as its
/// files one by one. From the server the two look identical — a part under a
/// folder is a part under a folder, whoever chose what for it — so guessing
/// would be packing files somebody asked to have added one at a time.
///
/// Absent is the ordinary drop, so every caller that is not importing a book
/// leaves it out.
#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    pub(super) path: Option<String>,
    #[serde(default)]
    pub(super) freeze: bool,
}
