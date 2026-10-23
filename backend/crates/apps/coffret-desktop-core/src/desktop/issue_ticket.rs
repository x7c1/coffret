use zeroize::Zeroizing;

use super::Desktop;
use crate::{Drawn, Error, Result};

impl Desktop {
    /// A one-time launch ticket for the explorer's host, for the shell to hand
    /// the explorer's page in its fragment (spec: LA-17, LA-20).
    pub fn issue_ticket(&self) -> Result<Zeroizing<String>> {
        let opened = self.shared.served().ok_or(Error::NothingOpen)?;
        let ticket = opened
            .credentials
            .issue_ticket()
            .map_err(|cause| Error::NotDrawn {
                what: Drawn::Ticket,
                cause,
            })?;
        Ok(ticket.encoded())
    }
}
