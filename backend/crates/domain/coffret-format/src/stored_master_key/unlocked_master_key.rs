use coffret_model::{MasterKey, MasterKeyEpoch};
use zeroize::ZeroizeOnDrop;

/// What a stored Master Key yields once the Passphrase opens it.
///
/// The epoch travels with the key because a key alone does not say which epoch
/// it belongs to, and every control object the device reads names the epoch that
/// wrote it.
///
/// It is on the secret-bearing inventory in [`coffret_model::MasterKey`]'s
/// module, so it is not `Clone`. It carries no `Drop` of its own: the only
/// secret in it is the [`MasterKey`], which wipes itself, and the epoch is a
/// counter rather than key material — every control object carries one, and
/// knowing it opens nothing (spec: FM-13). A `Drop` here would buy
/// nothing and cost the one thing callers want — moving the key out of the pair
/// into whatever holds it next.
#[derive(Debug)]
pub struct UnlockedMasterKey {
    /// The Master Key this device holds.
    pub master_key: MasterKey,
    /// The epoch that key belongs to.
    pub epoch: MasterKeyEpoch,
}

impl ZeroizeOnDrop for UnlockedMasterKey {}
