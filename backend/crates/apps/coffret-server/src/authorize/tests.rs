//! What each of the three fences lets through, header by header.
//!
//! The verdict rather than the response, because what is worth stating one case
//! at a time is which fence a request did not get past. That every route is
//! behind all three, and what a refusal looks like on the wire, is stated over
//! the router itself in `tests/routes.rs`.

use axum::http::HeaderMap;

use super::is_another_site::SITE_HEADER;
use super::{Admission, Refused, CAPABILITY_HEADER};

/// The address the server in these cases bound.
const AUTHORITY: &str = "127.0.0.1:8787";

/// The key it drew as it started.
const KEY: &str = "8f14e45fceea167a5a36dedd4bea2543a1b2c3d4e5f60718293a4b5c6d7e8f90";

fn admission() -> Admission {
    Admission::new(AUTHORITY, KEY)
}

/// One request's headers, built from what a case wants to say about it.
fn headers(named: &[(&str, &str)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in named {
        headers.insert(
            axum::http::HeaderName::from_bytes(name.as_bytes()).expect("a case names real headers"),
            value.parse().expect("a case sends text"),
        );
    }
    headers
}

/// What the explorer sends, with whatever the case adds to it.
fn as_the_explorer(extra: &[(&str, &str)]) -> HeaderMap {
    let mut named = vec![("host", AUTHORITY), (CAPABILITY_HEADER, KEY)];
    named.extend_from_slice(extra);
    headers(&named)
}

/// What the fences came to, or nothing where the request was let through.
fn verdict(headers: &HeaderMap) -> Option<Refused> {
    admission().verdict(headers).err()
}

// The whole of the legitimate path: the address this server is at, and the key
// it drew. Every other case here is one thing taken away from this one.
#[test]
fn the_explorer_is_let_through() {
    assert_eq!(verdict(&as_the_explorer(&[])), None);
}

// A same-machine tool is a caller like any other: it reads the key off the disk
// the way the explorer's proxy does, and sends neither of the headers a browser
// puts on a request.
#[test]
fn a_tool_on_this_device_is_let_through() {
    assert_eq!(
        verdict(&headers(&[
            ("host", AUTHORITY),
            (CAPABILITY_HEADER, KEY),
            ("user-agent", "curl/8.7.1"),
        ])),
        None,
    );
}

// The port may be left off, since one authority has two spellings and a caller
// reaching a server on port 80 would write neither of them the same way.
#[test]
fn the_host_may_leave_the_port_off() {
    assert_eq!(
        verdict(&headers(&[("host", "127.0.0.1"), (CAPABILITY_HEADER, KEY)])),
        None,
    );
}

// A name that resolved to this socket carries the name here, and that is the
// whole of how a rebound hostname is told from the explorer. The key would not
// be shown by such a request either, but the `Host` is read first: it costs
// nothing, and it is the answer whatever else the request carries.
#[test]
fn a_request_that_arrived_by_somebody_elses_name_is_refused() {
    assert_eq!(
        verdict(&headers(&[
            ("host", "coffret.example.com:8787"),
            (CAPABILITY_HEADER, KEY),
        ])),
        Some(Refused::Elsewhere),
    );
    // Including `localhost`, which is a name like any other: nothing here can
    // tell one somebody else's DNS answered from one this device's did.
    assert_eq!(
        verdict(&headers(&[
            ("host", "localhost:8787"),
            (CAPABILITY_HEADER, KEY),
        ])),
        Some(Refused::Elsewhere),
    );
}

// Another server of this device's — the explorer's own preview, say — is not
// this one, and a request forwarded without its `Host` being put right is a
// request that reached here by accident.
#[test]
fn another_port_on_this_device_is_refused() {
    assert_eq!(
        verdict(&headers(&[
            ("host", "127.0.0.1:4173"),
            (CAPABILITY_HEADER, KEY),
        ])),
        Some(Refused::Elsewhere),
    );
}

// The same server on the loopback address the other family spells: the port
// comes off the end and the address's own colons stay where they are.
#[test]
fn an_address_of_the_other_family_is_read_as_one_authority() {
    let admission = Admission::new("[::1]:8787", KEY);
    let asked = |host: &str| {
        admission
            .verdict(&headers(&[("host", host), (CAPABILITY_HEADER, KEY)]))
            .err()
    };

    assert_eq!(asked("[::1]:8787"), None);
    assert_eq!(asked("[::1]"), None);
    assert_eq!(asked("[::2]:8787"), Some(Refused::Elsewhere));
}

// HTTP/1.1 requires one. A caller that sends none is not the explorer, and the
// fence would be no fence at all if leaving the header out got past it.
#[test]
fn a_request_with_no_host_at_all_is_refused() {
    assert_eq!(
        verdict(&headers(&[(CAPABILITY_HEADER, KEY)])),
        Some(Refused::Elsewhere),
    );
}

// The fence the rest stand behind.
#[test]
fn a_request_with_no_key_is_refused() {
    assert_eq!(
        verdict(&headers(&[("host", AUTHORITY)])),
        Some(Refused::Unkeyed),
    );
}

// A key of the right shape and the wrong value, which is what a caller guessing
// sends — and a key from a run of this server that has already stopped.
#[test]
fn a_key_that_is_not_this_run_s_is_refused() {
    let other = "0000000000000000000000000000000000000000000000000000000000000000";
    assert_eq!(
        verdict(&headers(&[("host", AUTHORITY), (CAPABILITY_HEADER, other)])),
        Some(Refused::Unkeyed),
    );
    // A prefix of the real one, which is what a comparison that stopped at the
    // first difference would be measured with.
    assert_eq!(
        verdict(&headers(&[
            ("host", AUTHORITY),
            (CAPABILITY_HEADER, &KEY[..KEY.len() - 1]),
        ])),
        Some(Refused::Unkeyed),
    );
    // And the real one with something after it.
    assert_eq!(
        verdict(&headers(&[
            ("host", AUTHORITY),
            (CAPABILITY_HEADER, &format!("{KEY}0")),
        ])),
        Some(Refused::Unkeyed),
    );
}

// The key is never read off the URL, so a caller that puts it there is a caller
// that showed nothing. Stated here because the verdict reads the headers and
// only the headers: there is no query to be read, and this is what keeps it so.
#[test]
fn a_key_on_the_query_string_is_not_a_key() {
    assert_eq!(
        verdict(&headers(&[("host", AUTHORITY), ("referer", KEY)])),
        Some(Refused::Unkeyed),
    );
}

// The second fence. A page on another site cannot set the key header, but if
// anything ever hands it one, the browser's own account of where the request
// came from still refuses it.
#[test]
fn a_page_on_another_site_is_refused_even_holding_the_key() {
    assert_eq!(
        verdict(&as_the_explorer(&[("origin", "https://elsewhere.example")])),
        Some(Refused::AnotherSite),
    );
    assert_eq!(
        verdict(&as_the_explorer(&[(SITE_HEADER, "cross-site")])),
        Some(Refused::AnotherSite),
    );
    // A sibling of the same registrable domain is another site here: nothing
    // this server is served from has one.
    assert_eq!(
        verdict(&as_the_explorer(&[(SITE_HEADER, "same-site")])),
        Some(Refused::AnotherSite),
    );
}

// What the explorer's own page sends, once the proxy in front of it has put its
// own name to what it forwards.
#[test]
fn the_explorer_s_own_page_is_let_through() {
    assert_eq!(
        verdict(&as_the_explorer(&[
            ("origin", "http://127.0.0.1:8787"),
            (SITE_HEADER, "same-origin"),
        ])),
        None,
    );
    // And a person typing the address, which is `none` rather than an origin.
    assert_eq!(verdict(&as_the_explorer(&[(SITE_HEADER, "none")])), None);
}
