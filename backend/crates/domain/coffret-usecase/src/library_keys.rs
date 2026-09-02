use coffret_format::{Purpose, PurposeKey};
use coffret_model::{MasterKey, MasterKeyEpoch};
use zeroize::ZeroizeOnDrop;

use crate::commit::ControlKeys;

/// The keys one Master Key epoch's Container-level flows work under.
///
/// A commit needs the four control-object keys and nothing else, which is what
/// [`ControlKeys`] is. Carrying user data in either direction needs a fifth: a
/// sync draws a Container Key of its own per Container and seals it into the
/// envelope the Keyring stores, and a fetch opens that envelope again to decode
/// what it fetched (spec: KD-2, KD-4, FM-14).
///
/// One type for both directions rather than one per flow. The set is decided by
/// the Library's key derivation and not by which way the bytes are going, and
/// two bundles would be two spellings of one derivation — with nothing to stop
/// them drifting apart.
///
/// The two halves are derived together from one Master Key rather than passed
/// separately, for the reason [`ControlKeys`] gives about the epoch: a caller
/// that could hand over the halves independently could hand over halves of two
/// different epochs, and seal a Container's key under one while stamping its
/// record with the other.
///
/// It is on the secret-bearing inventory in [`coffret_model::MasterKey`]'s
/// module: not `Clone`, and wiped when it is dropped — by its [`PurposeKey`]
/// fields, each of which wipes itself, so this type needs no `Drop` of its own.
/// Every flow takes it by reference; a caller that needs one set of keys in two
/// places shares the one value rather than deriving or copying a second.
#[derive(Debug)]
pub struct LibraryKeys {
    control: ControlKeys,
    container_wrap: PurposeKey,
}

impl LibraryKeys {
    /// Derives everything one Master Key epoch's Containers are sealed and
    /// opened with.
    pub fn derive(master_key: &MasterKey, master_key_epoch: MasterKeyEpoch) -> Self {
        Self {
            control: ControlKeys::derive(master_key, master_key_epoch),
            container_wrap: PurposeKey::derive(master_key, Purpose::ContainerWrap),
        }
    }

    /// The control-object keys the commit and catch-up steps work under.
    pub const fn control(&self) -> &ControlKeys {
        &self.control
    }

    /// The key a Container Key is wrapped into, and unwrapped out of, its
    /// envelope under (spec: FM-14).
    pub(crate) const fn container_wrap(&self) -> &PurposeKey {
        &self.container_wrap
    }
}

impl ZeroizeOnDrop for LibraryKeys {}
