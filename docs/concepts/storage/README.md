# Storage

## Definition

**Storage** is the remote object store that holds a [Library](../library/)'s
[Storage Objects](../storage-object/) — Google Drive first, other services
such as S3 later. Storage sits outside the user's trust boundary, so coffret
hands it only ciphertext. [Containers](../container/) have opaque names; the
recognizable names of control objects, and of the app folder they all live
in, are an explicit, limited exception needed for recovery.

## Mental Model

### The grant on a device

A **grant** is what a person's consent leaves on a device: the credential the
device reaches the provider with. A Storage **account** is the provider
identity a person consents as. What a grant reaches is decided by the account
that consented and the OAuth client it consented to, and no Library takes
part in that, so a grant belongs to **a device and an account**:

| kept by | what | sealed under |
| --- | --- | --- |
| each account, once on a device | the grant, in a sealed cache | the account's **account-cache key**, a random key of its own |
| each Library that references the account | an **account-cache key envelope**: that key, wrapped | the Library's own [purpose key](../purpose-key/), bound to the account's name |

Unlocking a Library opens its envelope, the envelope yields the account-cache
key, and that key opens the account's cache. The envelope is device-local
and is not a [Key Envelope](../key-envelope/), which wraps a Container Key
and lives in the Keyring on Storage (spec: KD-12). The person tells accounts
apart by a **device-local account name** they give each one on the device,
the way they name a Library there, because coffret cannot: the one
permission it asks for names no account (spec: SA-3).

## Examples

- A Google Drive folder containing a few thousand opaque encrypted objects

## Collocations

- upload (a Storage Object to Storage)
- fetch (a Storage Object from Storage)
- create (a Library's app folder, under the place the user configured)
- discover (a Library's app folder, by listing the `coffret-` names at a
  Storage location)
- scan (Storage to rebuild the Index)
- salvage (decryptable Container contents when control state is incomplete)
- name (an account, on this device)
- renew (an account's grant on this device)

## Domain Rules

- **Storage is the source of truth for committed Library state.** Together
  with the [Master Key](../master-key/), intact required control state
  reconstructs exactly which Containers are current. Their contents open
  only where the committed Keyring supplies reachable envelopes; a key-lost
  Container remains current but locked. Local state — the
  [Index](../index/), caches — remains expendable
  (spec: RV-1, RV-2, RV-7).
- If required control state (defined in
  [Storage Object](../storage-object/)) is missing, scanning Storage can
  salvage contents from decryptable Containers, but salvage cannot prove
  which Containers are current, and it never authorizes automatic deletion or
  mutation (spec: RV-4).
  - If the loss is the Keyring itself — every committed valid replica — the
    intact Journal and checkpoints still prove which Containers are current,
    but those Containers become unreadable; coffret enumerates and
    reports them, and after a rebuild carries them with explicit key-lost
    markers, present but locked (spec: RV-7, RV-8).
- One Library's objects live flat in one **app folder** of the Storage
  location, named after the **Library ID** — a Drive folder, or the matching
  key prefix on a store that keys objects by name. One name identifies one
  object within it, and coffret only ever creates the folder under, and works
  inside, the place the user configured: where it sits is the user's
  arrangement of their own Storage, and it is the folder's name that a device
  recovering with only a Recovery Code enumerates for (spec: FM-18).
  - A user may name the folder something else, and coffret then reaches the
    Library at the name it is configured with. What that costs is the
    enumeration: a renamed folder is not found by it, so the user who renamed
    it is the one who tells a recovering device where the Library is. Nothing
    about the Library's contents changes, the name being outside every object
    rather than a field inside one (spec: FM-18).
- **Where in Storage a Library was configured is the person's own arrangement,
  not evidence.** The bucket or provider folder they chose, the base prefix
  under it, and the endpoint a device reaches it at are theirs rather than
  anything coffret or a provider minted, so no diagnostic event composes a
  field from one, and provider text is retained only after the bucket, the
  prefix and the chosen folder have been taken out of it. The app folder
  coffret creates inside that location is named after the **Library ID**, and
  that name — like a Container's opaque name and a control object's
  recognizable one — stays evidence an event may keep (spec: EL-1, EL-5).
- `object_ref` is Storage's own identifier for an object, the same value
  whichever device reads it, carried in control state as a cache so a device can
  fetch without listing Storage first. It is never evidence of membership,
  because a listing re-derives it and only the control state says what is current
  (spec: FM-15, FM-16).
- Authenticating Storage Objects proves their integrity, not their freshness:
  Storage can replay a coherent earlier Library state by withholding newer
  objects, and detecting that rollback is an accepted non-goal
  (spec: RV-6).
- Reaching Storage takes a grant the device keeps for an account — for
  Google Drive, an OAuth refresh token — and it is a bearer credential for
  every object this application created in that account: whoever holds it can
  read and write every object of every Library kept there, though not open any
  of them, since Storage only ever sees ciphertext. The grant is therefore
  sealed, never leaves the device, and opens only through a Library the device
  has unlocked (spec: SA-7, SA-9, KD-10, KD-12).
  - That grant is verified rather than assumed: the provider has to say it
    granted exactly the one narrow permission coffret asked for — on Google
    Drive, the one that reaches only files this application itself created —
    so the credential reaches coffret's own
    objects and nothing else in that account. A wider grant, a different one,
    or an answer naming no grant at all is refused, and nothing is cached
    (spec: SA-3, SA-4).
  - The check is on the authorization that produces the credential, which is
    the only moment anything is cached. Later runs mint short-lived access
    tokens from what was cached and add no permission to it, so a grant the
    person later narrows or withdraws shows up as the provider refusing rather
    than as a check here (spec: SA-6).
- **A device keeps one grant per account, however many Libraries use it.**
  Two Libraries of one account were never apart on the provider's side — each
  one's grant reached the other's objects as far — so keeping the grant once
  adds no reach and leaves one copy of the credential rather than several
  (spec: SA-7, SA-8).
  - Unlocking any one Library that references an account opens that account's
    grant, and with it the other Libraries of that account on Storage — their
    ciphertext, never their contents, which each one's own
    [Master Key](../master-key/) guards (spec: SA-7, SA-9).
  - Removing a Library from the device removes its envelope; once the last
    one is gone the account's cache can no longer be opened, and the device
    discards it (spec: SA-8).
- The **device-local account name** is chosen by the person, never written to
  Storage, and never in a diagnostic event, like the device-local Library name
  it stands beside. It is optional while the device holds one account — an
  account left unnamed is called `default` — and required once a second is
  added, since from then on it is the only thing that says which grant a new
  Library should use (spec: SA-8, EL-1).
  - Joining a Library tries each grant the device already holds and takes the
    account whose Storage has the named app folder, so the person consents
    only for an account this device does not hold yet (spec: SA-8).
  - One account name stands for one OAuth client on the device, because the
    same account consenting to another client is another grant with another
    reach (spec: SA-8).
- A grant does not last forever: a provider may expire it, and the person may
  withdraw it at any time. So a device **renews** an account's grant by
  running the authorization again for that account — an ordinary act rather
  than a repair, which replaces only the account's cache, changes nothing
  else the device or Storage holds, is guarded by the same check, and serves
  every Library that references the account from its next run
  (spec: SA-4, SA-6, SA-8).

## Related Concepts

- [Storage Object](../storage-object/) — what Storage holds
- [Container](../container/) — a Storage Object holding user data
- [Index Snapshot](../index-snapshot/), [Journal](../journal/), and
  [Keyring](../keyring/) — the specially named objects on Storage
- [Library](../library/) — what Storage can restore, and what references an
  account on a device
- [Purpose Key](../purpose-key/) — wraps the account-cache key into each
  Library's envelope
- [Specification register](../../spec/) — the behavioral rules cited by ID
