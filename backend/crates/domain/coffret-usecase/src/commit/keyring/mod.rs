//! The Keyring a commit writes and the one it reads: examining and repairing
//! the committed set before a write, writing and proving the candidate set a
//! head selects, and reading the committed set for whoever needs its key table
//! (spec: KL-1, KL-2, KL-6, KL-11, KL-13, KL-14).
//!
//! Each of those is a module of its own; what they share — the framing of one
//! replica, its read-back, and the name a position is stored under — is here,
//! so that there is one reading of what a replica object is.

use coffret_format::{
    decode_keyring, encode_control_object, keyring_set_digest, ControlEncodeRequest, ControlPayload,
};
use coffret_model::{
    ControlObjectKind, ControlObjectName, KeyTable, KeyringCommitment, ObjectRef, ReplicaPosition,
};

use crate::byte_stream::ByteStream;
use crate::commit::commit_error::{CommitError, CommitResult, UnusableReplica};
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_object;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

// The walk a commit makes over the committed set before it writes, and the
// repair it performs there (spec: KL-11, KL-13).
mod examined;
pub(super) use examined::{examine, Examined};

// The read of the committed set for a flow that needs its key table
// (spec: KL-6).
mod read_keyring;
pub(crate) use read_keyring::read_committed;

// Writing the candidate set a head selects (spec: CP-8, KL-2).
mod replicate;
pub(super) use replicate::{replicate, write_generation};

mod short_keyring;
pub use short_keyring::ShortKeyring;

mod short_report;
pub(crate) use short_report::ShortReport;

/// The name one position of a committed generation is stored under
/// (spec: FM-12).
fn replica_name(commitment: &KeyringCommitment, index_of: u16) -> CommitResult<ControlObjectName> {
    let replica = ReplicaPosition::new(index_of, commitment.replica_count())?;
    Ok(ControlObjectName::keyring_replica(
        commitment.generation(),
        commitment.set_digest(),
        replica,
    )?)
}

/// Frames one replica of a generation and writes it (spec: FM-11, KL-14).
///
/// The write is unconditional, because a replica at
/// `(generation, set_digest, index)` has exactly one valid content: two devices
/// writing that name write the same key table under the same digest, so there is
/// no race whose loser would need reporting and a duplicate is benign. The
/// objects still differ, each sealed with a random nonce of its own — what is
/// identical is the content the digest covers.
///
/// Shared by the two writers of a Keyring replica, because they are writing the
/// same thing for different reasons: [`replicate`](replicate::replicate) materializes a candidate
/// generation, and the repair in [`examined`] re-materializes a position of a committed one
/// (spec: KL-13). A second copy of the framing would be a second reading of
/// what a replica object is.
async fn write_replica(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    retry: &RetryPolicy,
    name: &ControlObjectName,
    payload: &ControlPayload,
) -> CommitResult<ObjectRef> {
    let object = encode_control_object(&ControlEncodeRequest::new(
        name,
        ControlObjectKind::Keyring,
        keys.of_kind(ControlObjectKind::Keyring),
        payload,
    ))?;
    let spelling = name.to_string();
    let uploaded = retry
        .run("put", || {
            store.put(&spelling, ByteStream::from(object.bytes().to_vec()))
        })
        .await?;
    Ok(uploaded.object_ref)
}

/// Reads one replica back and decides whether it is valid (spec: KL-1).
///
/// Validity is three things and the framing already checks two of them: the
/// object opens and authenticates, and its header's kind, generation, and
/// replica position agree with the name it was fetched under (spec: FM-11,
/// FM-12). The third is this crate's to check — the digest of the key table
/// inside is the digest the name carries — because that is what binds a name to
/// one content and a commitment to one key table (spec: CP-10, KL-3, KL-14).
///
/// A failure comes back as an [`UnusableReplica`] rather than as a
/// [`CommitError`], because the caller is what knows whether it means "this
/// replica is unreadable, try the next one" or "the candidate set is
/// incomplete, stop". Which of the two it is decides the variant the reason
/// ends up in, and the reason itself travels as a value either way.
///
/// The fetch and the open are driven as two steps rather than through
/// [`control_object::read`], because the reason has to say which of them
/// failed: Storage refusing the object leaves the replica's content unknown,
/// while an object that arrived and would not open is a replica this Library
/// definitively cannot read a key table from (spec: KL-1, KL-5). Neither decides
/// anything differently here — the caller's step-over or stop is the same
/// either way — so what the split changes is only how precise the value is.
async fn read_replica(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    retry: &RetryPolicy,
    name: &ControlObjectName,
    object: &ObjectRef,
    expected: &str,
) -> std::result::Result<KeyTable, UnusableReplica> {
    let bytes = control_object::fetch(store, retry, name, object)
        .await
        .map_err(|error| unfetchable(error.into()))?;
    let decoded = control_object::open(keys, name, &bytes).map_err(unreadable)?;
    if decoded.kind != ControlObjectKind::Keyring {
        return Err(UnusableReplica::KindNotAdmitted {
            found: decoded.kind,
        });
    }
    let key_table = decode_keyring(&decoded.payload).map_err(|error| unreadable(error.into()))?;
    let actual = keyring_set_digest(&key_table).map_err(|error| unreadable(error.into()))?;
    if actual != expected {
        return Err(UnusableReplica::DigestMismatch {
            expected: expected.to_owned(),
            actual,
        });
    }
    Ok(key_table)
}

/// What Storage reported, as the reason the replica never arrived.
fn unfetchable(error: CommitError) -> UnusableReplica {
    UnusableReplica::Unfetchable(Box::new(error))
}

/// What the format layer reported, as the reason the replica that did arrive is
/// not one a key table may be read from.
fn unreadable(error: CommitError) -> UnusableReplica {
    UnusableReplica::Unreadable(Box::new(error))
}
