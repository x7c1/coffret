use axum::extract::rejection::JsonRejection;
use coffret_device::Redacted;

/// A body the JSON extractor would not read, kept as the value it refused with.
///
/// An event is told its status, which says which of the ways a body fails it
/// was. The extractor's own words are not told: they may quote what was sent,
/// and what is sent to the map route is a local path.
pub(crate) struct Json(pub(crate) JsonRejection);

impl Redacted for Json {
    fn redacted(&self) -> String {
        format!("JsonRejection(status={})", self.0.status().as_u16())
    }
}
