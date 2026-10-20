use anyhow::Context;
use zeroize::Zeroizing;

use super::Desktop;
use crate::door::{DoorMode, DoorUrl};

impl Desktop {
    /// A fresh token for one Passphrase window opened in `mode`, as the URL
    /// the shell loads that window from (spec: LA-23).
    ///
    /// Refused in the unlock mode while nothing is served, since there is
    /// nothing a window could unlock.
    pub fn open_door(&self, mode: DoorMode) -> anyhow::Result<DoorUrl> {
        if mode == DoorMode::Unlock && self.shared.served().is_none() {
            anyhow::bail!("no Library is open in this app to unlock");
        }
        let token = self
            .shared
            .windows
            .issue(mode)
            .context("the Passphrase window's token could not be drawn")?;
        let mut url = Zeroizing::new(format!("http://{}/#door=", self.door));
        // Room for the token before it is written, so that the buffer never
        // grows and leaves an unwiped copy of it behind.
        url.reserve_exact(token.len());
        url.push_str(&token);
        Ok(DoorUrl::new(url))
    }
}
