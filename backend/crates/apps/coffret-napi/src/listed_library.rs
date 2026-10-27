use napi_derive::napi;

/// One Library on this device, as the shell is told of it.
#[napi(object)]
pub struct ListedLibrary {
    /// Its ID, in the 16 lowercase hex characters it is spelled in.
    pub id: String,
    /// Its name, or null where nothing has named it yet.
    pub name: Option<String>,
    /// Whether more than one directory on this device holds it, which opening
    /// it is refused for until one of them is gone.
    pub needs_attention: bool,
}
