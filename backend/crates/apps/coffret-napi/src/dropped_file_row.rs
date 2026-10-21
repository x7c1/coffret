use napi_derive::napi;

/// One file a drop took.
#[napi(object)]
pub struct DroppedFileRow {
    /// Its path relative to the folder it was dropped from.
    pub path: String,
    /// Its length in bytes.
    pub size: f64,
    /// Its modification time, in milliseconds from the Unix epoch.
    pub mtime: i64,
}
