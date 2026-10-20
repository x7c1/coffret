use coffret_desktop_core as core;
use napi_derive::napi;

/// What a Passphrase window is opened to do.
#[napi(string_enum)]
pub enum DoorMode {
    /// Choose a Library on this device and open it.
    Open,
    /// Unlock in place the Library served, once it has locked.
    Unlock,
}

impl From<DoorMode> for core::DoorMode {
    fn from(mode: DoorMode) -> Self {
        match mode {
            DoorMode::Open => Self::Open,
            DoorMode::Unlock => Self::Unlock,
        }
    }
}
