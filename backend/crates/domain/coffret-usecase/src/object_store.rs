use std::ops::Range;

use async_trait::async_trait;

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_page::ObjectPage;
use crate::object_ref::ObjectRef;
use crate::page_token::PageToken;

/// Everything coffret asks of a Storage provider.
///
/// Storage sits outside the trust boundary and only ever sees ciphertext, so
/// the port is deliberately small: put bytes under a name, get them back,
/// enumerate what is there, and remove things. What providers disagree about —
/// multipart against resumable uploads, keys against minted file IDs, an ETag
/// against an MD5 — stays inside an adapter.
///
/// Two pieces of the contract are load-bearing for the layer above, and an
/// adapter that gets them wrong breaks the Library rather than one call:
///
/// - **Conditional create.** [`reserve_create`](Self::reserve_create) followed
///   by [`put_if_absent`](Self::put_if_absent) is the commit primitive: of
///   several writers spending the same slot, exactly one succeeds and the rest
///   see [`Error::AlreadyExists`](crate::Error::AlreadyExists) (spec: CP-3). A
///   lost race must never be reported as a transport failure, and a transport
///   failure must never be reported as a lost race. A slot is bound to the
///   name it was reserved for, so the exclusion holds however the winner and
///   the losers differ in what they are writing;
///   [`object_at`](Self::object_at) is how a loser reaches the winner's
///   object.
/// - **Two removals.** [`trash`](Self::trash) is recoverable and is what
///   removing a Container means; [`purge`](Self::purge) is irreversible and is
///   what Master Key rotation does to old-epoch control objects, which is why it
///   is only successful once a read-back confirms the object is gone.
///
/// The trait is object safe, so a caller can hold `Arc<dyn ObjectStore>` and be
/// written once against whichever provider a Library is configured for.
#[async_trait]
pub trait ObjectStore: Send + Sync {
    /// Writes `body` under `name`, replacing anything already stored there.
    ///
    /// This is the unconditional write, for Containers and Keyring replicas.
    /// A Container's name is drawn from its own random identifier, so two
    /// writers cannot pick the same one; a Keyring replica's name determines
    /// its content — the mapping its digest binds — so two devices repairing
    /// the same replica write identical bytes and the duplicate is harmless
    /// (spec: KL-14). Either way there is no race to lose. Whether the bytes
    /// travel as one request, as multipart parts, or through a resumable
    /// session is the adapter's business.
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef>;

    /// Reserves a slot for one conditional create of an object called `name`.
    ///
    /// The name is fixed here rather than at the create, so that one
    /// reservation cannot be spent under two names: a control head decides its
    /// successor's name when it hands out the slot, and the writers that start
    /// from that head therefore all aim at the same object (spec: CP-2).
    ///
    /// Reserving creates nothing, and it does not stop another writer from
    /// reserving too: on a name-keyed store it is idempotent per name — every
    /// reservation of one name is the same slot — and on a store that mints
    /// identifiers each call mints a fresh one. Exclusion comes from the single
    /// reservation a head carries, never from two writers reserving the same
    /// name independently, and the race itself is settled by
    /// [`put_if_absent`](Self::put_if_absent), not here.
    async fn reserve_create(&self, name: &str) -> Result<CommitSlot>;

    /// Writes `body` into `slot`, only if the slot is still free.
    ///
    /// The name comes from the slot, so there is no spelling for spending one
    /// reservation under another name.
    ///
    /// Returns [`Error::AlreadyExists`](crate::Error::AlreadyExists) — and
    /// leaves what is stored untouched — when another writer got there first.
    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef>;

    /// The object a slot holds, as this store's own handle for it.
    ///
    /// A writer that lost the race — or never learned whether it won — has to
    /// fetch what is actually at the slot and compare it to its own candidate
    /// (spec: CP-4, CP-5, CK-11). It cannot look the object up by name: on a
    /// store that mints identifiers, names are not unique, so a by-name lookup
    /// could answer with a different object that happens to share the name.
    /// Pass the handle this returns to [`get`](Self::get); an empty slot is
    /// [`Error::NotFound`](crate::Error::NotFound) from there, not from here.
    fn object_at(&self, slot: &CommitSlot) -> Result<ObjectRef>;

    /// Reads an object back, whole or over a half-open byte range.
    ///
    /// A range beyond the end of the object is the provider's to reject; a
    /// missing object is [`Error::NotFound`](crate::Error::NotFound).
    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream>;

    /// Lists one page of the live objects, resuming from `page`.
    ///
    /// Trashed objects are not live and never appear. Pass `None` for the first
    /// page and then the previous page's [`ObjectPage::next`] until it is
    /// `None`; over a listing that nothing else is writing to, that walks every
    /// object exactly once.
    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage>;

    /// Removes an object recoverably.
    ///
    /// It leaves the listing, and the provider keeps the bytes where a person
    /// can still get them back. The [`ObjectRef`] stays valid: a trashed object
    /// is the same object, and [`purge`](Self::purge) still accepts it.
    async fn trash(&self, object: &ObjectRef) -> Result<()>;

    /// Removes an object irreversibly, live or already trashed.
    ///
    /// Master Key rotation is only complete once every reachable old-epoch
    /// control object has been deleted rather than trashed, because those are
    /// exactly what a leaked old Recovery Code could open (spec: MR-3), so this
    /// confirms the deletion by reading back and fails with
    /// [`Error::NotPurged`](crate::Error::NotPurged) if the object is still
    /// there. Purging something already gone succeeds, so a rotation that was
    /// interrupted can simply be run again.
    async fn purge(&self, object: &ObjectRef) -> Result<()>;
}
