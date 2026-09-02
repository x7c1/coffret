//! Who this server answers, and how it knows.
//!
//! Binding `127.0.0.1` says where the socket is, not who may use it. The person
//! at this device runs a browser, the browser runs other people's pages, and a
//! page may aim a request at a loopback port without ever being able to read the
//! answer: a `fetch` it never sees the response of still starts a sync, a form
//! it submits still writes bytes into a mapped folder, and a hostname of its own
//! that resolves to `127.0.0.1` still arrives at this socket. So every request
//! is authorized here, before any route sees it, and reads are held to it
//! exactly as mutations are — the Entry Paths and the plaintext a read answers
//! with are the Library.
//!
//! Three fences, in this order.
//!
//! The `Host` first. A request that reached this socket through a name of
//! somebody else's carries that name here, because the browser sends the name it
//! was given rather than the address it resolved to; a request that came to
//! where this server actually is names that. Nothing about the connection can
//! tell those two apart and the header can, so the header is read before
//! anything else is.
//!
//! Then the key ([`coffret_device::ServerKey`]), in a header of this server's
//! own naming. That is the fence the others stand behind: a page in a browser
//! cannot read a local file, and cannot put a header of this name on a form
//! submission or on a request it may not read the answer to. So a caller with it
//! is a process of the account that owns this Library — which is the boundary
//! this server was always meant to have, now drawn by the operating system's
//! file permissions rather than by the socket's address.
//!
//! And last what the browser says about itself. `Origin` and `Sec-Fetch-Site`
//! are set by the browser and cannot be forged by the page, so a request that
//! admits to coming from another site is refused even when it somehow carries a
//! key. It is a second fence and not a replacement for the first: a caller that
//! is not a browser sends neither header, and every same-machine tool is such a
//! caller.

use axum::http::HeaderMap;

// The middleware the whole router is behind, and the only part of this that
// knows it is one.
mod admit;
pub(crate) use admit::admit;

// Whether the browser says the page that asked belongs to another site.
mod is_another_site;

// Whether the `Host` names where this server is.
mod is_here;

// Which fence a request did not get past, and what it is answered with.
mod refused;
use refused::Refused;

// Whether the request carries the key this run drew.
mod shows_the_key;

#[cfg(test)]
mod tests;

/// The header a caller carries this server's key in.
///
/// A header rather than a query parameter or a cookie. A cookie is attached by
/// the browser to requests the page never made, which is the whole of what this
/// is defending against; a query parameter is on the URL, and the URL is the one
/// part of a request that gets written down everywhere.
pub const CAPABILITY_HEADER: &str = "x-coffret-key";

/// What a caller has to show before any route sees their request.
///
/// One running server's: the address it bound and the key it drew as it
/// started. Both are values rather than settings — there is nothing here to
/// configure, and nothing that outlives the process.
pub struct Admission {
    /// The authority this server is at, as a `Host` spells it.
    authority: String,
    /// The same, as an `Origin` spells it.
    origin: String,
    /// The bare host of [`authority`](Self::authority), for the `Host` that
    /// leaves the port off.
    host: String,
    /// The key this run drew, which a caller reads off this device's disk.
    secret: String,
}

impl Admission {
    /// The rules one server admits by.
    ///
    /// `authority` is the address that was actually bound rather than the one
    /// that was asked for: `--port 0` is answered by the operating system, and a
    /// server holding somebody's typed port would refuse every request that
    /// reached the port it is really on.
    pub fn new(authority: impl Into<String>, secret: impl Into<String>) -> Self {
        let authority = authority.into();
        // The last colon separates the port, except in `[::1]`, whose colons are
        // the address's own and whose closing bracket is what says so.
        let host = match authority.rsplit_once(':') {
            Some((host, port)) if !port.contains(']') => host.to_owned(),
            _ => authority.clone(),
        };
        Self {
            origin: format!("http://{authority}"),
            authority,
            host,
            secret: secret.into(),
        }
    }

    /// What a request's headers come to, before any route reads the request.
    fn verdict(&self, headers: &HeaderMap) -> Result<(), Refused> {
        if !self.is_here(headers) {
            return Err(Refused::Elsewhere);
        }
        if !self.shows_the_key(headers) {
            return Err(Refused::Unkeyed);
        }
        if self.is_another_site(headers) {
            return Err(Refused::AnotherSite);
        }
        Ok(())
    }
}

/// One header as text, and nothing for a header that is not there or is not
/// text.
fn text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}
