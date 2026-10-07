use serde::Deserialize;

/// What a page asks to have mapped.
#[derive(Debug, Deserialize)]
pub struct MapRequest {
    /// The folder on this device, as a whole path.
    pub(super) local_root: String,
    /// The top-level folder of the Library it is to hold, and `null` for the
    /// Library root (spec: EP-9).
    pub(super) prefix: Option<String>,
}
