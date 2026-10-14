use std::sync::Arc;

use crate::authority::Authority;
use crate::credentials::Gate;
use crate::forward::Forwarder;
use crate::page::Page;
use crate::LibraryLock;

/// Everything a request handler of the host reads: where the host is, the
/// credentials it admits by, whether the Library is locked, where it forwards
/// to, and what page it serves.
///
/// It holds a [`Gate`] and never the [`Credentials`](crate::credentials::Credentials)
/// that issue tickets, so no request can issue one (spec: LA-17).
pub(crate) struct Host {
    pub(crate) authority: Authority,
    pub(crate) gate: Gate,
    pub(crate) lock: Arc<dyn LibraryLock>,
    pub(crate) forwarder: Forwarder,
    pub(crate) page: Page,
}
