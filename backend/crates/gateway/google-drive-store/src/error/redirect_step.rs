use std::fmt;

/// Which call of the loopback redirect a failure happened at.
///
/// The operating system reports what went wrong, never what was being asked
/// of it, and that is what separates a port already taken from a browser that
/// hung up before it said anything.
#[derive(Debug)]
pub enum RedirectStep {
    /// Listening on the loopback port the browser is to be sent back to.
    Bind,
    /// Reading back which port the operating system handed out.
    Port,
    /// Taking the connection the browser arrives on.
    Accept,
    /// Reading the request the browser sent.
    Read,
}

impl fmt::Display for RedirectStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Bind => "could not listen for the redirect",
            Self::Port => "could not read the redirect port",
            Self::Accept => "could not accept the redirect",
            Self::Read => "could not read the redirect",
        })
    }
}
