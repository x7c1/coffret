use napi_derive::napi;

/// The Library served, as the shell is told of it.
#[napi(object)]
pub struct ServedLibrary {
    /// Its name, or its ID where nothing has named it yet.
    pub name: String,
    /// Whether it is unlocked, or has locked itself and waits for the
    /// Passphrase again.
    pub unlocked: bool,
}
