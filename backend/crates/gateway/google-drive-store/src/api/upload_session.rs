use std::{error, fmt};

use url::Url;

use crate::api::endpoints::Endpoints;

/// The query parameter Drive names a resumable session by.
const UPLOAD_ID: &str = "upload_id";

/// Where an upload's bytes are sent: a session URI Drive answered with, held
/// to being one at Drive's own upload endpoint.
///
/// Opening a resumable session is answered with a `Location`, and the bytes
/// and the bearer token go wherever it says. Taken as it came, an answer that
/// had been replaced on its way back — by an intermediary that misbehaved or
/// was compromised — would carry both somewhere else: another host, plain
/// HTTP, a port on this machine. So the answer is checked before anything is
/// attached to it, and a value of this type is the only thing the upload sends
/// to. It can only be made by [`accept`](Self::accept), which takes a URL
/// only where
///
/// - its scheme is `https`;
/// - its host and port are those of [`Endpoints::upload`];
/// - its path is that endpoint's path;
/// - it carries no userinfo and no fragment;
/// - its query carries a non-empty `upload_id`.
///
/// Every session URI seen from real Drive has that shape. What it is checked
/// against is read off [`Endpoints`], so where the gateway talks to stays
/// stated once.
pub struct UploadSession {
    url: Url,
}

impl UploadSession {
    /// Takes `location` as the session to send bytes to, if it is one at the
    /// upload endpoint of `endpoints`.
    pub fn accept(endpoints: &Endpoints, location: &str) -> Result<Self, SessionRefusal> {
        let endpoint = endpoints.upload_url();
        let url = Url::parse(location).map_err(SessionRefusal::NotAUrl)?;

        if url.scheme() != "https" {
            return Err(SessionRefusal::Scheme {
                found: url.scheme().to_owned(),
            });
        }
        // Checked before the host is reported on, so a refusal never repeats
        // a credential somebody put in front of it.
        if !url.username().is_empty() || url.password().is_some() {
            return Err(SessionRefusal::Userinfo);
        }
        if url.host_str() != endpoint.host_str() {
            return Err(SessionRefusal::Host {
                found: url.host_str().unwrap_or_default().to_owned(),
                expected: endpoint.host_str().unwrap_or_default().to_owned(),
            });
        }
        if url.port_or_known_default() != endpoint.port_or_known_default() {
            return Err(SessionRefusal::Port {
                found: url.port_or_known_default(),
                expected: endpoint.port_or_known_default(),
            });
        }
        if url.path() != endpoint.path() {
            return Err(SessionRefusal::Path {
                found: url.path().to_owned(),
                expected: endpoint.path().to_owned(),
            });
        }
        if url.fragment().is_some() {
            return Err(SessionRefusal::Fragment);
        }
        let names_a_session = url
            .query_pairs()
            .any(|(name, value)| name == UPLOAD_ID && !value.is_empty());
        if !names_a_session {
            return Err(SessionRefusal::NoUploadId);
        }

        Ok(Self { url })
    }

    /// The session URI, query and all.
    ///
    /// The query carries the session's `upload_id`, which is a capability: it
    /// is for the request and never for a log (spec: EL-1).
    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }
}

/// Deliberately says nothing past where the session points: the query is a
/// capability.
impl fmt::Debug for UploadSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UploadSession")
            .field("host", &self.url.host_str())
            .field("path", &self.url.path())
            .finish_non_exhaustive()
    }
}

/// Which rule a session URI Drive answered with broke.
///
/// What it says is which rule, and the scheme, host, port or path it found
/// where one of those is what broke it — never the query, which is where a
/// session's capability lives, and never the userinfo or fragment, which are
/// refused without being repeated.
///
/// It is an error of its own, so the port's failure can carry it whole as the
/// value it was classified from rather than as a sentence.
#[derive(Debug)]
pub enum SessionRefusal {
    /// It is not a URL at all.
    NotAUrl(url::ParseError),
    /// It is not `https`.
    Scheme {
        /// The scheme it named.
        found: String,
    },
    /// It carries a username or a password.
    Userinfo,
    /// It names another host than the upload endpoint's.
    Host {
        /// The host it named; empty where it named none.
        found: String,
        /// The upload endpoint's host.
        expected: String,
    },
    /// It names another port than the upload endpoint's.
    Port {
        /// The port it named, or its scheme's default.
        found: Option<u16>,
        /// The upload endpoint's port.
        expected: Option<u16>,
    },
    /// It names another path than the upload endpoint's.
    Path {
        /// The path it named.
        found: String,
        /// The upload endpoint's path.
        expected: String,
    },
    /// It carries a fragment.
    Fragment,
    /// Its query names no session.
    NoUploadId,
}

impl fmt::Display for SessionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The parse error is the next link of the chain, not part of
            // this one.
            Self::NotAUrl(_) => f.write_str("it is not a URL"),
            Self::Scheme { found } => write!(f, "its scheme is {found:?}, not \"https\""),
            Self::Userinfo => f.write_str("it carries userinfo"),
            Self::Host { found, expected } => {
                write!(f, "its host is {found:?}, not {expected:?}")
            }
            Self::Port { found, expected } => {
                write!(f, "its port is {}, not {}", port(*found), port(*expected))
            }
            Self::Path { found, expected } => {
                write!(f, "its path is {found:?}, not {expected:?}")
            }
            Self::Fragment => f.write_str("it carries a fragment"),
            Self::NoUploadId => write!(f, "its query names no {UPLOAD_ID}"),
        }
    }
}

impl error::Error for SessionRefusal {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            // A parse error's own words describe the shape of the input, not
            // the input.
            Self::NotAUrl(error) => Some(error),
            Self::Scheme { .. }
            | Self::Userinfo
            | Self::Host { .. }
            | Self::Port { .. }
            | Self::Path { .. }
            | Self::Fragment
            | Self::NoUploadId => None,
        }
    }
}

/// A port as a refusal names it.
fn port(port: Option<u16>) -> String {
    port.map_or_else(|| "none".to_owned(), |port| port.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capability every case's query carries, which no refusal may repeat.
    const CAPABILITY: &str = "AEnB2Uo-capability";

    /// Whether a refusal names the rule a case broke.
    type Broke = fn(&SessionRefusal) -> bool;

    fn accept(location: &str) -> Result<UploadSession, SessionRefusal> {
        UploadSession::accept(&Endpoints::default(), location)
    }

    #[test]
    fn a_session_drive_hands_back_is_accepted_as_it_came() {
        let location = format!(
            "https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&upload_id={CAPABILITY}"
        );

        let session = accept(&location).expect("Drive's own session URI must be accepted");

        assert_eq!(session.as_str(), location);
        assert!(
            !format!("{session:?}").contains(CAPABILITY),
            "the debug form keeps the capability out: {session:?}"
        );
    }

    #[test]
    fn a_session_anywhere_else_is_refused_by_the_rule_it_breaks() {
        let cases: [(&str, Broke); 11] = [
            ("https://collector.invalid/upload", |r| {
                matches!(r, SessionRefusal::Host { .. })
            }),
            ("http://collector.invalid/upload", |r| {
                matches!(r, SessionRefusal::Scheme { .. })
            }),
            ("http://127.0.0.1:49123/upload", |r| {
                matches!(r, SessionRefusal::Scheme { .. })
            }),
            (
                "http://www.googleapis.com/upload/drive/v3/files?upload_id=x",
                |r| matches!(r, SessionRefusal::Scheme { .. }),
            ),
            (
                "https://www.googleapis.com:8443/upload/drive/v3/files?upload_id=x",
                |r| matches!(r, SessionRefusal::Port { .. }),
            ),
            (
                "https://user:pw@www.googleapis.com/upload/drive/v3/files?upload_id=x",
                |r| matches!(r, SessionRefusal::Userinfo),
            ),
            (
                "https://www.googleapis.com.evil.example/upload/drive/v3/files?upload_id=x",
                |r| matches!(r, SessionRefusal::Host { .. }),
            ),
            (
                "https://www.googleapis.com/upload/drive/v3/files?upload_id=x#frag",
                |r| matches!(r, SessionRefusal::Fragment),
            ),
            (
                "https://www.googleapis.com/drive/v3/files?upload_id=x",
                |r| matches!(r, SessionRefusal::Path { .. }),
            ),
            ("https://www.googleapis.com/upload/drive/v3/files", |r| {
                matches!(r, SessionRefusal::NoUploadId)
            }),
            ("not a url", |r| matches!(r, SessionRefusal::NotAUrl(_))),
        ];

        for (location, broke) in cases {
            let refusal = accept(location).expect_err(location);
            assert!(broke(&refusal), "{location}: refused as {refusal:?}");
        }
    }

    #[test]
    fn an_empty_upload_id_names_no_session() {
        let refusal = accept("https://www.googleapis.com/upload/drive/v3/files?upload_id=")
            .expect_err("an empty upload_id must be refused");

        assert!(matches!(refusal, SessionRefusal::NoUploadId), "{refusal:?}");
    }

    #[test]
    fn the_default_port_written_out_is_the_endpoints_own() {
        accept("https://www.googleapis.com:443/upload/drive/v3/files?upload_id=x")
            .expect("port 443 is the endpoint's port, written out");
    }

    #[test]
    fn a_refusal_never_repeats_the_query_userinfo_or_fragment() {
        let locations = [
            format!("https://collector.invalid/upload?upload_id={CAPABILITY}"),
            format!("http://www.googleapis.com/upload/drive/v3/files?upload_id={CAPABILITY}"),
            format!("https://www.googleapis.com:8443/upload/drive/v3/files?upload_id={CAPABILITY}"),
            format!("https://{CAPABILITY}:pw@www.googleapis.com/upload/drive/v3/files?upload_id=x"),
            format!("https://www.googleapis.com/upload/drive/v3/files?upload_id=x#{CAPABILITY}"),
            format!("https://www.googleapis.com/drive/v3/files?upload_id={CAPABILITY}"),
            format!("https://www.googleapis.com/upload/drive/v3/files?uploadType={CAPABILITY}"),
            format!("{CAPABILITY} is not a url?upload_id={CAPABILITY}"),
        ];

        for location in &locations {
            let refusal = accept(location).expect_err(location);
            for rendered in [refusal.to_string(), format!("{refusal:?}")] {
                assert!(
                    !rendered.contains(CAPABILITY),
                    "{location}: the refusal repeated what it must not: {rendered}"
                );
            }
        }
    }
}
