/// The Library a desktop serves, as the shell is told of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedLibrary {
    /// Its name, and `None` where nothing has named it yet (spec: FM-15).
    pub name: Option<String>,
    /// Whether it is unlocked, or has locked itself after the idle interval
    /// and waits for the Passphrase again (spec: DK-1, DK-4).
    pub unlocked: bool,
}
