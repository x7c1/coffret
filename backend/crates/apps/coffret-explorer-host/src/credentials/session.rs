use std::time::Duration;

use super::InstanceId;

/// Which session a token stood for, among those one host issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SessionId(pub(super) u64);

/// A session a browser token was just found to stand for (spec: LA-18).
///
/// It names the session and never the token, so it may be held for as long as
/// a request is being answered.
#[derive(Debug, Clone)]
pub struct Session {
    pub(super) id: SessionId,
    pub(super) instance_id: InstanceId,
    pub(super) expires_in: Duration,
}

impl Session {
    /// The generation the session belongs to.
    pub fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    /// How long the session had left when its token was authenticated.
    pub fn expires_in(&self) -> Duration {
        self.expires_in
    }
}
