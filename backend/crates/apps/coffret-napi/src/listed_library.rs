use napi_derive::napi;

/// One Library on this device, as the shell is told of it.
#[napi(object)]
pub struct ListedLibrary {
    /// Its ID, in the 16 lowercase hex characters it is spelled in.
    pub id: String,
    /// The name it is listed under: its name, or its ID where nothing has
    /// named it yet.
    pub name: String,
}
