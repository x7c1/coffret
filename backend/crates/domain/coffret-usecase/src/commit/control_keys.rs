use coffret_format::{Purpose, PurposeKey};
use coffret_model::{ControlObjectKind, MasterKey, MasterKeyEpoch};
use zeroize::ZeroizeOnDrop;

/// The keys one Master Key epoch opens and seals control objects with.
///
/// A commit touches four kinds — it writes Journal records, Keyring replicas,
/// and ordinary Index Snapshots, and it may meet an activation Snapshot while
/// catching up — and each kind has a purpose key of its own (spec: KD-4,
/// FM-11). Deriving them once and choosing by kind is what keeps the flow from
/// ever presenting a key to the wrong payload; the encoder and decoder refuse
/// that anyway, so what this really buys is that the refusal is impossible to
/// reach rather than merely handled.
///
/// The epoch travels with the keys because it is not a separate decision: the
/// Master Key those keys came from *is* an epoch, and every control payload
/// records which one sealed it (spec: FM-13, CP-13). Carrying the two apart
/// would let a caller seal an object under one epoch's key and stamp it with
/// another's number.
///
/// It is on the secret-bearing inventory in [`coffret_model::MasterKey`]'s
/// module, and keeps the list's two promises the way [`LibraryKeys`] does: not
/// `Clone`, and wiped on drop through its [`PurposeKey`] fields.
///
/// [`LibraryKeys`]: crate::LibraryKeys
#[derive(Debug)]
pub struct ControlKeys {
    master_key_epoch: MasterKeyEpoch,
    journal: PurposeKey,
    keyring: PurposeKey,
    index_snapshot: PurposeKey,
    activation_snapshot: PurposeKey,
}

impl ControlKeys {
    /// Derives every control-object key of one Master Key epoch.
    pub fn derive(master_key: &MasterKey, master_key_epoch: MasterKeyEpoch) -> Self {
        Self {
            master_key_epoch,
            journal: PurposeKey::derive(master_key, Purpose::ControlJournal),
            keyring: PurposeKey::derive(master_key, Purpose::ControlKeyring),
            index_snapshot: PurposeKey::derive(master_key, Purpose::ControlIndexSnapshot),
            activation_snapshot: PurposeKey::derive(master_key, Purpose::ControlActivationSnapshot),
        }
    }

    /// Which Master Key epoch these keys belong to (spec: FM-13).
    pub const fn master_key_epoch(&self) -> MasterKeyEpoch {
        self.master_key_epoch
    }

    /// The key that seals and opens payloads of one kind (spec: FM-11, KD-4).
    pub(super) const fn of_kind(&self, kind: ControlObjectKind) -> &PurposeKey {
        match kind {
            ControlObjectKind::Journal => &self.journal,
            ControlObjectKind::Keyring => &self.keyring,
            ControlObjectKind::IndexSnapshot => &self.index_snapshot,
            ControlObjectKind::ActivationSnapshot => &self.activation_snapshot,
        }
    }
}

impl ZeroizeOnDrop for ControlKeys {}
