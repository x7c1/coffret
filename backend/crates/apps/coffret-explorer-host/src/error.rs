use std::fmt;
use std::io;
use std::net::SocketAddr;

/// The host could not be built around the socket it was handed.
#[derive(Debug)]
pub enum Error {
    /// The listener could not say which address it is bound to, so there is
    /// no authority to fix (spec: LA-13).
    Unbound(io::Error),
    /// The listener is bound somewhere other than `127.0.0.1`: the host
    /// listens on IPv4 loopback alone (spec: LA-1, LA-13).
    NotLoopback(SocketAddr),
    /// The HTTP client it forwards through refused to start, which is the TLS
    /// backend failing to initialise and nothing a caller can put right by
    /// trying again.
    Client(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unbound(_) => {
                f.write_str("the explorer's host could not read the address it is bound to")
            }
            Self::NotLoopback(address) => write!(
                f,
                "the explorer's host is served on 127.0.0.1 alone, and was handed a socket \
                 bound to {address}"
            ),
            Self::Client(_) => f.write_str("the explorer's host could not start its HTTP client"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unbound(cause) => Some(cause),
            Self::NotLoopback(_) => None,
            Self::Client(cause) => Some(cause),
        }
    }
}
