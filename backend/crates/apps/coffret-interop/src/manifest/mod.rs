//! What a fixture set says about itself.
//!
//! A fixture set is a directory of opaque byte strings plus a `manifest.json`
//! that states, for every one of them, the key material needed to open it and
//! the values it must decode to. The manifest states inputs and expectations
//! only — never a length, an offset, a hash, or any other value the format
//! derives — because an expectation the manifest computed itself would prove
//! only that the manifest writer and the reader share a bug.
//!
//! Both implementations write this same schema, so a set produced by either can
//! be checked by the other. Every fixture kind has a module of its own here, so
//! a kind the exchange gains later arrives beside its neighbours rather than in
//! the middle of them.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::hex;
use coffret_model::{MasterKey, Passphrase};

mod argon2_params_fixture;
pub use argon2_params_fixture::Argon2ParamsFixture;

mod body_field;
pub use body_field::BodyField;

mod body_value;
pub use body_value::BodyValue;

mod cbor_map;
pub use cbor_map::check_cbor_map;

mod container_fixture;
pub use container_fixture::ContainerFixture;

mod control_object_fixture;
pub use control_object_fixture::ControlObjectFixture;

mod derived_from_fixture;
pub use derived_from_fixture::DerivedFromFixture;

mod entry_fixture;
pub use entry_fixture::EntryFixture;

mod key_envelope_fixture;
pub use key_envelope_fixture::KeyEnvelopeFixture;

mod payload_fields;
pub use payload_fields::{index_snapshot_fields, journal_record_fields, keyring_fields};

mod recovery_code_fixture;
pub use recovery_code_fixture::RecoveryCodeFixture;

mod stored_master_key_fixture;
pub use stored_master_key_fixture::StoredMasterKeyFixture;

mod wire_container_kind;
pub use wire_container_kind::WireContainerKind;

mod wire_control_object_kind;
pub use wire_control_object_kind::WireControlObjectKind;

mod check_coverage;

/// The manifest schema both implementations write and read.
pub const SCHEMA: u64 = 1;

/// The file a fixture set describes itself in.
pub const MANIFEST_FILE: &str = "manifest.json";

/// The Container fixtures every set carries, whichever side wrote it.
///
/// One of the Packs holds a single Entry, so a kind guessed from the Entry
/// count rather than read from the object (PK-15) fails the exchange.
pub const REQUIRED_CONTAINERS: [&str; 4] =
    ["one-file", "multi-entry", "singleton-pack", "empty-entries"];

/// The control-object fixtures every set carries — one of each kind (FM-11).
///
/// The Journal record and the activation Snapshot are both stored under a
/// `head-` name (FM-12), so a set that carries both is a set no implementation
/// can pass by reading a kind off a name.
pub const REQUIRED_CONTROL_OBJECTS: [&str; 4] = [
    "journal",
    "activation-snapshot",
    "keyring-replica",
    "index-snapshot",
];

/// The Key Envelope fixtures every set carries.
pub const REQUIRED_KEY_ENVELOPES: [&str; 1] = ["key-envelope"];

/// The stored Master Key fixtures every set carries.
pub const REQUIRED_STORED_MASTER_KEYS: [&str; 1] = ["stored-master-key"];

/// The Recovery Code fixtures every set carries.
///
/// Two of them, and the second is written in the grouped printing form: the
/// grouping is presentation and a reader strips it (KD-11), so a set carrying
/// only bare codes would let an implementation that never strips anything pass.
pub const REQUIRED_RECOVERY_CODES: [&str; 2] = ["recovery-code", "recovery-code-grouped"];

/// Everything a fixture set states about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The manifest schema these fields follow.
    pub schema: u64,
    /// Which implementation wrote the set, for the message on a failure.
    pub producer: String,
    /// The Master Key every purpose key in the set derives from, as hex.
    pub master_key: String,
    /// The Passphrase the stored Master Key forms are protected under.
    pub passphrase: String,
    /// The Containers in the set.
    pub containers: Vec<ContainerFixture>,
    /// The control objects in the set.
    pub control_objects: Vec<ControlObjectFixture>,
    /// The Key Envelopes in the set.
    pub key_envelopes: Vec<KeyEnvelopeFixture>,
    /// The stored Master Key forms in the set.
    pub stored_master_keys: Vec<StoredMasterKeyFixture>,
    /// The Recovery Codes in the set.
    pub recovery_codes: Vec<RecoveryCodeFixture>,
}

impl Manifest {
    /// The Master Key the set's purpose keys derive from.
    pub fn master_key(&self) -> Result<MasterKey> {
        Ok(MasterKey::from_bytes(
            hex::decode_array(&self.master_key).context("master_key")?,
        ))
    }

    /// The Passphrase, as the value a user's terminal would have produced.
    ///
    /// A fixture set's Passphrase is published in its own manifest — it is test
    /// data and not a secret — but the type it becomes here is the one the
    /// format crate takes, so the exchange runs the same path a device does.
    pub fn passphrase(&self) -> Passphrase {
        Passphrase::from_bytes(self.passphrase.clone().into_bytes())
    }
}
