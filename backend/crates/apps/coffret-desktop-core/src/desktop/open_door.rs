use zeroize::Zeroizing;

use super::Desktop;
use crate::door::{DoorMode, DoorUrl};
use crate::{Drawn, Error, Result};

impl Desktop {
    /// A fresh token for one Passphrase window opened in `mode`, as the URL
    /// the shell loads that window from (spec: LA-23).
    ///
    /// Refused in the unlock mode while nothing is served, since there is
    /// nothing a window could unlock.
    pub fn open_door(&self, mode: DoorMode) -> Result<DoorUrl> {
        if mode == DoorMode::Unlock && self.shared.served().is_none() {
            return Err(Error::NothingToUnlock);
        }
        let token = self
            .shared
            .windows
            .issue(mode)
            .map_err(|cause| Error::NotDrawn {
                what: Drawn::DoorToken,
                cause,
            })?;
        let mut url = Zeroizing::new(format!("http://{}/#door=", self.door));
        // Room for the token before it is written, so that the buffer never
        // grows and leaves an unwiped copy of it behind.
        url.reserve_exact(token.len());
        url.push_str(&token);
        Ok(DoorUrl::new(url))
    }
}
