use napi_derive::napi;

/// One thing a drop met and did not take.
#[napi(object)]
pub struct NotTakenRow {
    /// Where it stands relative to the folder it was dropped from.
    pub path: String,
    /// Why, in one word.
    pub reason: String,
}
