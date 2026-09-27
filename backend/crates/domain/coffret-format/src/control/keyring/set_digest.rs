use coffret_model::{lowercase_hex, KeyringMapping};

use super::encode::mapping_value;
use crate::control::cbor::write_body;
use crate::error::Result;

/// The digest binding one Keyring generation's mapping (FM-17).
///
/// It is the BLAKE3-256 of the `mapping` array alone — the array exactly as the
/// payload carries it, in Container ID order — and it is deliberately not a
/// field of that payload: a digest carried inside the thing it covers would
/// have to cover itself.
///
/// One definition therefore serves three readers. A replica's object name
/// carries this value (FM-12), a commit selects a replica set by it (CP-10,
/// KL-3), and a reader recomputes it from a decoded mapping to decide whether
/// the replica it fetched is the one that name promised (KL-1). Two devices
/// preparing or repairing one generation land on the same digest because they
/// land on the same bytes (KL-14).
///
/// The result is the lowercase hex text those three carry it in, not the raw
/// 32 bytes: the name grammar spells it that way, and one digest with one
/// spelling is what keeps a commitment comparable as it travels.
pub fn set_digest(mapping: &KeyringMapping) -> Result<String> {
    Ok(lowercase_hex::encode(
        blake3::hash(&digest_input(mapping)?).as_bytes(),
    ))
}

/// The bytes the digest is taken over: the `mapping` array, encoded.
///
/// Named apart from the hash so that what FM-17 makes normative has somewhere
/// to be examined. Everything else this crate writes is one valid CBOR spelling
/// among several — a reader takes any of them — while these bytes are the
/// spelling, because a second one would be a second digest for one mapping.
pub(super) fn digest_input(mapping: &KeyringMapping) -> Result<Vec<u8>> {
    write_body(&mapping_value(mapping))
}
