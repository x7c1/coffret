use std::error;
use std::fmt;

use coffret_model::{ContainerId, Redacted};

use crate::delete::PackRefusal;

/// Why a repack committed nothing at all (spec: PK-23).
///
/// One Pack the run had to rewrite could not be read and verified, and a
/// repack is all or nothing: one done in part would leave the folder's order
/// half made while looking done. Nothing was committed and nothing on Storage
/// was touched; every Pack stays exactly as it was, and a run made once the
/// cause is fixed does the whole job.
///
/// There is deliberately no `PartialEq`: the reason carries a failure, read
/// from its variant and never by comparing two refusals.
#[derive(Debug)]
pub struct RepackRefusal {
    /// The Pack that could not be read.
    pub container_id: ContainerId,
    /// Why: its key is lost (spec: KL-7, KL-17), or it was read and did not
    /// verify (spec: PK-10).
    pub reason: PackRefusal,
}

impl fmt::Display for RepackRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.reason {
            PackRefusal::KeyLost => write!(
                f,
                "Pack {} cannot be read because the Library records its key as lost, so \
                 nothing was repacked",
                self.container_id,
            ),
            // What did not verify is left to `source`, for the reason every
            // wrapper in this crate leaves its cause there.
            PackRefusal::Unverified(_) => write!(
                f,
                "Pack {} could not be read and verified, so nothing was repacked",
                self.container_id,
            ),
        }
    }
}

impl error::Error for RepackRefusal {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.reason {
            PackRefusal::KeyLost => None,
            PackRefusal::Unverified(unverified) => Some(unverified),
        }
    }
}

impl Redacted for RepackRefusal {
    /// A Container ID and the shape of the reason: nothing here is a name of
    /// the person's.
    fn redacted(&self) -> String {
        match &self.reason {
            PackRefusal::KeyLost => {
                format!("RepackRefusal::KeyLost(container={})", self.container_id)
            }
            PackRefusal::Unverified(unverified) => format!(
                "RepackRefusal::Unverified(container={}): {}",
                self.container_id,
                unverified.redacted(),
            ),
        }
    }
}
