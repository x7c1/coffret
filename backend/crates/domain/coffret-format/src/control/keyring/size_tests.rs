//! What a Keyring of a real-sized Library costs (FM-17).
//!
//! The Keyring is the one control object a Library pays for twice over: its
//! payload grows with the Container count, and every generation is stored R
//! times (KL-8). It is also rewritten whole at every Master Key rotation, which
//! is the trade the whole design rests on — rotation rewrites compact control
//! objects instead of every Container (MR-1).
//!
//! So the per-Container cost is the number the schema was shaped around, and
//! almost all of it is content the format cannot compress away: a 16-byte
//! Container ID and a 72-byte Key Envelope. What is pinned here is what the
//! *schema* spends beyond those 88 bytes — the two field names, the CBOR
//! headers, and the element map itself. A field added per Container shows up in
//! that number and nowhere else.

use coffret_model::{ContainerId, KeyringEntry, KeyringMapping};

use super::encode;
use super::testing::{envelope, mapping_epoch, mapping_of};

/// Containers in the synthetic Library.
const CONTAINERS: usize = 10_000;

/// The ceiling this format was sized on: 110 bytes per Container before
/// padding, of which 88 are the ID and the envelope themselves.
const DESIGN_BUDGET: usize = 110;

/// What one Container costs beyond its own ID and envelope.
///
/// Everything the schema itself spends: the `id` and `envelope` keys, the CBOR
/// headers of the two byte strings and of the element map, and this Container's
/// share of the array. It is pinned rather than bounded loosely, because it is
/// the number a change to the schema moves — and every rotation and every
/// repair pays it R times over.
const PINNED_COST_BEYOND_THE_ID_AND_ENVELOPE: usize = 16;

/// The bytes one Container contributes as content rather than as schema: its
/// ID (FM-3) and its Key Envelope (FM-14).
const ID_AND_ENVELOPE: usize = ContainerId::BYTE_LEN + coffret_model::KeyEnvelope::BYTE_LEN;

// FM-17: a Keyring of ten thousand Containers stays inside the per-Container
// cost the schema was shaped around.
#[test]
fn ten_thousand_containers_stay_inside_the_design_budget() {
    let payload = encode(&library(), mapping_epoch()).expect("encoding a whole Library succeeds");
    let per_container = payload.body.len() / CONTAINERS;
    assert!(
        per_container <= DESIGN_BUDGET,
        "a Keyring of {CONTAINERS} Containers costs {per_container} bytes per Container, \
         past the {DESIGN_BUDGET}-byte design budget"
    );
}

// The part of that cost the schema is answerable for: what is left once the ID
// and the envelope are taken out.
#[test]
fn the_cost_beyond_the_id_and_envelope_is_pinned() {
    let payload = encode(&library(), mapping_epoch()).expect("encoding a whole Library succeeds");

    let beyond = (payload.body.len() - CONTAINERS * ID_AND_ENVELOPE) / CONTAINERS;
    assert_eq!(
        beyond, PINNED_COST_BEYOND_THE_ID_AND_ENVELOPE,
        "one Container costs {beyond} bytes beyond its ID and envelope; the pinned figure is \
         {PINNED_COST_BEYOND_THE_ID_AND_ENVELOPE}. Moving it is a decision about what every \
         rotation and every repair rewrites R times over, not a number to follow the code."
    );
}

/// A Library of [`CONTAINERS`] Containers, every one of them openable.
///
/// No key-lost marker among them: a marker is the cheaper element of the two,
/// so a Library that had any would understate what a Keyring costs.
fn library() -> KeyringMapping {
    mapping_of(
        (0..CONTAINERS)
            .map(|index| KeyringEntry::envelope(synthetic_id(index), envelope(index as u8)))
            .collect(),
    )
}

/// A Container ID that differs between Containers and orders by index.
fn synthetic_id(index: usize) -> ContainerId {
    let mut bytes = [0u8; ContainerId::BYTE_LEN];
    for (position, byte) in bytes.iter_mut().enumerate() {
        *byte = (index as u8)
            .wrapping_mul(31)
            .wrapping_add(position as u8 * 7);
    }
    bytes[..4].copy_from_slice(&(index as u32).to_be_bytes());
    ContainerId::from_bytes(bytes)
}
