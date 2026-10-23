use std::fmt;

/// What a random value was being drawn for.
#[derive(Debug, Clone, Copy)]
pub enum Drawn {
    /// A launch ticket for the explorer's host.
    Ticket,
    /// The explorer's credentials.
    Credentials,
    /// A Passphrase window's token.
    DoorToken,
}

impl fmt::Display for Drawn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ticket => "a launch ticket could not be drawn",
            Self::Credentials => "the explorer's credentials could not be drawn",
            Self::DoorToken => "the Passphrase window's token could not be drawn",
        })
    }
}
