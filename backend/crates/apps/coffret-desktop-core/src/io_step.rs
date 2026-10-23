use std::fmt;

/// Which step of setting up a socket or the runtime failed.
#[derive(Debug, Clone, Copy)]
pub enum IoStep {
    /// Starting the async runtime.
    Runtime,
    /// Binding the Passphrase door's listener.
    DoorBind,
    /// Reading the address the door is bound to.
    DoorAddress,
    /// Making the door's listener non-blocking.
    DoorNonBlocking,
    /// Handing the door's listener to the runtime.
    DoorHandOver,
    /// Binding the explorer's listener.
    ExplorerBind,
    /// Reading the address the explorer is bound to.
    ExplorerAddress,
}

impl fmt::Display for IoStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Runtime => "the async runtime could not be started",
            Self::DoorBind => "nothing could listen for the Passphrase window on 127.0.0.1",
            Self::DoorAddress => "the address the Passphrase window is served at could not be read",
            Self::DoorNonBlocking => {
                "the Passphrase window's listener could not be made non-blocking"
            }
            Self::DoorHandOver => {
                "the Passphrase window's listener could not be handed to the runtime"
            }
            Self::ExplorerBind => "nothing could listen for the explorer on 127.0.0.1",
            Self::ExplorerAddress => "the address the explorer is served at could not be read",
        })
    }
}
