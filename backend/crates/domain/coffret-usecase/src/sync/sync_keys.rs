use coffret_format::{Purpose, PurposeKey};
use coffret_model::{MasterKey, MasterKeyEpoch};

use crate::commit::ControlKeys;

/// The keys one Master Key epoch's sync works under.
///
/// A commit needs the four control-object keys and nothing else, which is what
/// [`ControlKeys`] is. A sync needs a fifth: every Container it encodes gets a
/// Container Key of its own, and that key reaches the Keyring wrapped under the
/// container-wrap purpose key (spec: KD-2, KD-4, FM-14).
///
/// The two are derived together from one Master Key rather than passed
/// separately, for the reason [`ControlKeys`] gives about the epoch: a caller
/// that could hand over the two halves independently could hand over halves of
/// two different epochs, and seal a Container's key under one while stamping
/// its record with the other.
#[derive(Debug, Clone)]
pub struct SyncKeys {
    control: ControlKeys,
    container_wrap: PurposeKey,
}

impl SyncKeys {
    /// Derives everything a sync of one Master Key epoch seals with.
    pub fn derive(master_key: &MasterKey, master_key_epoch: MasterKeyEpoch) -> Self {
        Self {
            control: ControlKeys::derive(master_key, master_key_epoch),
            container_wrap: PurposeKey::derive(master_key, Purpose::ContainerWrap),
        }
    }

    /// The control-object keys the commit step works under.
    pub const fn control(&self) -> &ControlKeys {
        &self.control
    }

    /// The key a Container Key is wrapped into its envelope under (spec: FM-14).
    pub(super) const fn container_wrap(&self) -> &PurposeKey {
        &self.container_wrap
    }
}
