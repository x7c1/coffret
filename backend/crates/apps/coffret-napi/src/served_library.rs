use napi_derive::napi;

/// The Library served, as the shell is told of it.
#[napi(object)]
pub struct ServedLibrary {
    /// The name it has on this device.
    pub name: String,
    /// Whether it is unlocked, or has locked itself and waits for the
    /// Passphrase again.
    pub unlocked: bool,
}
