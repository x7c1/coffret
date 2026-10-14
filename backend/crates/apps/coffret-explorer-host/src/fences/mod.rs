//! What a request passes before the host does anything with it (spec: LA-13,
//! LA-14, LA-19).
//!
//! Each fence is a layer rather than a check named in each route, so that no
//! route, and no path that no route answers, can be left outside one:
//!
//! - [`fixed_host`] over every path: the headers are within their bound and
//!   `Host` is the authority the listener bound;
//! - [`same_origin`] over `/api` and `/auth`: what the browser says about where
//!   the request came from;
//! - [`admit`] over `/api`: a browser token of this host's current generation,
//!   and a Library that is unlocked or a request that unlocks it.
//!
//! Every refusal is answered before the body is read, and nothing here reads
//! the server key's file or reaches the server (spec: LA-14).

mod admit;
pub(crate) use admit::admit;

mod credential;
pub(crate) use credential::{bearer, launch_ticket};

mod fixed_host;
pub(crate) use fixed_host::fixed_host;

mod same_origin;
pub(crate) use same_origin::same_origin;
