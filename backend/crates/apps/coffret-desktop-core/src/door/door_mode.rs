/// What a Passphrase window is opened to do, which the door holds beside the
/// window's token rather than taking from anything the window sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorMode {
    /// Choose a Library on this device and open it, as the app starts.
    Open,
    /// Unlock in place the Library this app serves, once it has locked
    /// (spec: DK-1).
    Unlock,
}
