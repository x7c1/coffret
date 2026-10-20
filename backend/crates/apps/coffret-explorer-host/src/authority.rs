use std::net::{Ipv4Addr, SocketAddr};

use axum::http::header::{HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderValue, Uri};

use crate::Error;

/// What a request reaches the host as: `127.0.0.1:<port>` as a `Host` spells
/// it, and `http://127.0.0.1:<port>` as an `Origin` does (spec: LA-13).
///
/// Built from the address a listener actually bound and from nothing else, so
/// that no request can say where the host is: a request that names anywhere
/// else is one that reached this socket through somebody else's name.
///
/// Public for another loopback listener in the same app that is fenced the
/// same way: the desktop app's Passphrase door (spec: LA-22).
pub struct Authority {
    host: HeaderValue,
    origin: HeaderValue,
}

/// The header a browser says where a request came from in.
const SEC_FETCH_SITE: &str = "sec-fetch-site";

/// Which of the origin fences a request did not get past, as the fixed word an
/// event records it by (spec: EL-1).
pub(crate) type Fence = &'static str;

impl Authority {
    /// The authority of a listener bound to `bound`, which has to be IPv4
    /// loopback (spec: LA-1, LA-13).
    pub fn of(bound: SocketAddr) -> Result<Self, Error> {
        match bound {
            SocketAddr::V4(v4) if *v4.ip() == Ipv4Addr::LOCALHOST => {}
            _ => return Err(Error::NotLoopback(bound)),
        }
        let host = HeaderValue::from_str(&bound.to_string())
            .expect("a socket address is a valid header value");
        let origin = HeaderValue::from_str(&format!("http://{bound}"))
            .expect("a socket address is a valid header value");
        Ok(Self { host, origin })
    }

    /// Whether `origin` is the fixed origin, exactly.
    pub fn is_origin(&self, origin: &HeaderValue) -> bool {
        origin.as_bytes() == self.origin.as_bytes()
    }

    /// Whether a request names this authority and nothing else: one `Host`,
    /// exactly this one, and a target in absolute form only where it names the
    /// same (spec: LA-13).
    ///
    /// `Forwarded` and `X-Forwarded-*` are not read at all: nothing in front
    /// of the host is trusted to say where a request was meant for.
    pub fn is_named_by(&self, headers: &HeaderMap, uri: &Uri) -> bool {
        let mut hosts = headers.get_all(HOST).iter();
        let (Some(host), None) = (hosts.next(), hosts.next()) else {
            return false;
        };
        if host.as_bytes() != self.host.as_bytes() {
            return false;
        }
        match (uri.scheme_str(), uri.authority()) {
            (None, None) => true,
            (Some("http"), Some(authority)) => {
                authority.as_str().as_bytes() == self.host.as_bytes()
            }
            _ => false,
        }
    }

    /// Whether what the browser says about a request admits it as one the
    /// host's own page made (spec: LA-14).
    ///
    /// `Origin`, where there is one, is the fixed origin exactly; `null` and
    /// one given more than once are not. `Sec-Fetch-Site`, where there is one,
    /// is `same-origin`. A request carrying neither is not refused here: a
    /// client that is not a browser sends neither, and sets both to whatever
    /// it likes besides, so these keep another site's page out and are not
    /// authentication.
    pub(crate) fn admits_origin(&self, headers: &HeaderMap) -> Result<(), Fence> {
        match at_most_one(headers, ORIGIN.as_str()) {
            Err(()) => return Err("origin"),
            Ok(Some(origin)) if !self.is_origin(origin) => return Err("origin"),
            Ok(_) => {}
        }
        match at_most_one(headers, SEC_FETCH_SITE) {
            Err(()) => Err("fetch_site"),
            Ok(Some(site)) if site.as_bytes() != b"same-origin" => Err("fetch_site"),
            Ok(_) => Ok(()),
        }
    }
}

/// The one value of `name`, `None` where there is none, and an error where
/// there are more than one.
fn at_most_one<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a HeaderValue>, ()> {
    let mut values = headers.get_all(name).iter();
    let first = values.next();
    match values.next() {
        None => Ok(first),
        Some(_) => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authority() -> Authority {
        Authority::of("127.0.0.1:5173".parse().unwrap()).unwrap()
    }

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(*name, HeaderValue::from_static(value));
        }
        headers
    }

    // LA-13: the host is IPv4 loopback alone.
    #[test]
    fn only_ipv4_loopback_is_an_authority() {
        assert!(Authority::of("127.0.0.1:1".parse().unwrap()).is_ok());
        for elsewhere in ["0.0.0.0:1", "127.0.0.2:1", "[::1]:1", "192.168.0.1:1"] {
            assert!(
                matches!(
                    Authority::of(elsewhere.parse().unwrap()),
                    Err(Error::NotLoopback(_))
                ),
                "{elsewhere}"
            );
        }
        assert!(authority().is_origin(&HeaderValue::from_static("http://127.0.0.1:5173")));
    }

    // LA-13: one `Host`, exactly the bound authority, and an absolute target
    // naming the same.
    #[test]
    fn a_request_names_the_authority_exactly() {
        let here = authority();
        let path: Uri = "/api/list".parse().unwrap();
        assert!(here.is_named_by(&headers(&[("host", "127.0.0.1:5173")]), &path));
        let absolute: Uri = "http://127.0.0.1:5173/api/list".parse().unwrap();
        assert!(here.is_named_by(&headers(&[("host", "127.0.0.1:5173")]), &absolute));

        for host in [
            &[][..],
            &[("host", "127.0.0.1:5173"), ("host", "127.0.0.1:5173")][..],
            &[("host", "localhost:5173")][..],
            &[("host", "127.0.0.1")][..],
            &[("host", "127.0.0.1:5174")][..],
            &[("host", "evil.invalid:5173")][..],
            &[("host", "127.0.0.1:5173 ")][..],
        ] {
            assert!(!here.is_named_by(&headers(host), &path), "{host:?}");
        }
        for target in [
            "http://evil.invalid:5173/api/list",
            "https://127.0.0.1:5173/api/list",
            "http://127.0.0.1:5174/api/list",
        ] {
            let target: Uri = target.parse().unwrap();
            assert!(
                !here.is_named_by(&headers(&[("host", "127.0.0.1:5173")]), &target),
                "{target}"
            );
        }
    }

    // LA-14: the browser's word on where a request came from.
    #[test]
    fn the_origin_fences_admit_the_hosts_own_page_and_no_other() {
        let here = authority();
        assert_eq!(here.admits_origin(&headers(&[])), Ok(()));
        assert_eq!(
            here.admits_origin(&headers(&[
                ("origin", "http://127.0.0.1:5173"),
                ("sec-fetch-site", "same-origin"),
            ])),
            Ok(())
        );
        for origin in [
            &[("origin", "null")][..],
            &[("origin", "http://localhost:5173")][..],
            &[("origin", "https://127.0.0.1:5173")][..],
            &[
                ("origin", "http://127.0.0.1:5173"),
                ("origin", "http://127.0.0.1:5173"),
            ][..],
        ] {
            assert!(
                matches!(here.admits_origin(&headers(origin)), Err("origin")),
                "{origin:?}"
            );
        }
        for site in ["same-site", "cross-site", "none"] {
            assert!(
                matches!(
                    here.admits_origin(&headers(&[("sec-fetch-site", site)])),
                    Err("fetch_site")
                ),
                "{site}"
            );
        }
    }
}
