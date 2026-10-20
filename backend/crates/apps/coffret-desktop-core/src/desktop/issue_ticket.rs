use anyhow::Context;
use zeroize::Zeroizing;

use super::Desktop;

impl Desktop {
    /// A one-time launch ticket for the explorer's host, for the shell to hand
    /// the explorer's page in its fragment (spec: LA-17, LA-20).
    pub fn issue_ticket(&self) -> anyhow::Result<Zeroizing<String>> {
        let opened = self
            .shared
            .served()
            .context("no Library is open in this app")?;
        let ticket = opened
            .credentials
            .issue_ticket()
            .context("a launch ticket could not be drawn")?;
        Ok(ticket.encoded())
    }
}
