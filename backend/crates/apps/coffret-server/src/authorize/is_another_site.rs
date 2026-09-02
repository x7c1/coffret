use axum::http::header::ORIGIN;
use axum::http::HeaderMap;

use super::{text, Admission};

/// What the browser says about where a request came from.
pub(super) const SITE_HEADER: &str = "sec-fetch-site";

impl Admission {
    /// Whether the browser says this request came from somewhere else.
    ///
    /// Absent headers are not an assertion of anything: a same-machine tool
    /// sends neither, and a browser omits `Origin` on the ordinary reads. What
    /// is refused is a browser saying, in either header, that the page asking
    /// belongs to another site.
    ///
    /// `Origin` is measured against this server's own, which is what the page
    /// the explorer is served from reaches it as: the dev and preview servers
    /// proxy `/api` here, and each rewrites the `Origin` of a request its own
    /// page made and leaves every other one as it found it.
    pub(super) fn is_another_site(&self, headers: &HeaderMap) -> bool {
        let elsewhere = match text(headers, ORIGIN.as_str()) {
            Some(origin) => origin != self.origin,
            None => false,
        };
        let cross_site = match text(headers, SITE_HEADER) {
            // `none` is a person typing the address or opening a bookmark, and
            // `same-origin` is the explorer's own page asking.
            Some(site) => site != "same-origin" && site != "none",
            None => false,
        };
        elsewhere || cross_site
    }
}
