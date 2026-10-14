use std::time::Duration;

use super::{BrowserToken, InstanceId};

/// What an exchange answers the page with (spec: LA-16).
#[derive(Debug)]
pub struct Exchanged {
    /// The page's browser token. The only copy: the host keeps its digest.
    pub token: BrowserToken,
    /// How long the token is good for from now.
    pub expires_in: Duration,
    /// The generation the token belongs to.
    pub instance_id: InstanceId,
}
