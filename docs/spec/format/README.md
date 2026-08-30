# Storage Object Format

Rule prefix: `FM`. The byte-level form of every Storage Object: the
Container v1 header and its chunked AEAD framing, the encrypted meta
section, size padding, the common control-object framing and the payload
schemas the control kinds carry inside it, the Key Envelope form, and the
names objects carry on Storage — including the name of the one folder a
Library's objects live in.

Concept background: [Storage Object](../../concepts/storage-object/),
[Container](../../concepts/container/),
[Entry](../../concepts/container/entry/),
[Key Envelope](../../concepts/key-envelope/),
[Journal](../../concepts/journal/),
[Index Snapshot](../../concepts/index-snapshot/).

Every key named here is produced under the
[Key Derivation](../key-derivation/) rules. Multi-byte integers are
big-endian throughout.

## Rules

- **FM-1.** Every AEAD operation in format v1 — the Container meta section
  and chunks, control-object payloads, and Key Envelopes — is
  XChaCha20-Poly1305 with a 256-bit key and a 24-byte nonce. A message that
  fails authentication is rejected whole, and no plaintext from an
  unauthenticated message is released downstream. *(Form: test)*
- **FM-2.** A Container is one object laid out as:

  ```
  offset  size  field
  ------  ----  -----
  0       5     magic = "CFRT1"
  5       1     format version = 0x01
  6       2     reserved = 0x0000
  8       16    Container ID
  24      4     chunk size (plaintext bytes per chunk)
  28      4     meta section length M (padded ciphertext bytes)
  32      M     meta section (encrypted, FM-9)
  32+M    ...   chunk sequence (encrypted, FM-5)
  ```

  An object with an unknown magic or format version is rejected without
  attempting decryption; reserved bytes must be zero. The meta section length
  is the length of the padded meta section exactly as it appears in the object,
  tag included, so it reveals no more about the entry table than FM-9's padding
  leaves visible. The header carries no key material — Key Envelopes live in
  the Keyring only (CP-11) — which is what lets Master Key rotation leave
  Containers byte-for-byte unchanged (MR-1). *(Form: test)*
- **FM-3.** The Container ID is 128 bits drawn from a CSPRNG, and the
  Container's object name is the ID as 32 lowercase hex characters
  followed by `.cfrt`. The name therefore says nothing about the content.
  *(Form: test for the name's derivation from the ID; prose for the
  independence claim — the ID generator takes no content input, honored by
  construction and review)*
- **FM-4.** The plaintext stream of a Container is the concatenation of
  every Entry's plaintext in entry-table order (FM-9), followed by zero
  padding up to the next Padmé bucket boundary; the meta section's
  `pad_len` records exactly that padding length. *(Form: test)*
  - Padmé (from the PURBs work) rounds an unpadded length L up to the next
    multiple of 2^(E−S), where E = ⌊log₂ L⌋ and S = ⌊log₂ E⌋ + 1; a stream
    short enough that E − S ≤ 0 is stored unpadded. Overhead is bounded at
    about 12% and is typically a few percent.
  - The padding is all zero bytes, and a decoder verifies it: any non-zero
    byte in the padding tail fails decode.
  - Padding blunts fingerprinting of known content by exact size; what the
    provider still observes is listed under
    [Storage Object](../../concepts/storage-object/), and how padding
    interacts with the Pack size target is governed by PK-6.
- **FM-5.** The padded plaintext stream is cut into consecutive chunks of
  exactly the header's chunk size, the final chunk keeping the remainder,
  and each chunk is encrypted separately with the Container Key as
  ciphertext ‖ tag(16). Decryption authenticates each chunk before
  releasing its plaintext, so a reader never needs the whole Container in
  memory and never passes on unauthenticated bytes (FM-1). *(Form: test)*
  - The writing side is independent of memory for the same reason: the cut is
    fixed by the header's chunk size alone (FM-6), so a writer emits each chunk
    as the plaintext stream passes it and never needs the whole Container in
    memory either.
  - Consecutive chunks covering one plaintext extent form a **chunk run**. Its
    ciphertext extent follows from the header and the meta section alone — the
    chunk size and the meta section length place every chunk (FM-2), and the
    entry table gives the extent to round out to chunk boundaries (FM-9) — so a
    reader can name the bytes covering one Entry before any of them arrive
    (PK-16).
  - An empty padded stream — every Entry empty and no padding added (FM-4)
    — is encoded as exactly one empty final chunk: a message of tag alone.
    The chunk sequence is never empty, so every object still ends with the
    final-chunk domain marking the end of the stream (FM-7).
- **FM-6.** The chunk size is a per-Container format parameter recorded in
  the header, not a format constant; new Containers may adopt a different
  size without a format version change, and a reader honors the recorded
  value. The initial value is 1 MiB. *(Form: test for the parameter
  mechanism; the value choice itself is a design decision recorded outside
  this register)*
- **FM-7.** The nonce of every Container AEAD message is deterministic:
  domain(1) ‖ counter(8, big-endian) ‖ zero(15), with domain 0x01 for the
  meta section (counter 0), 0x02 for a non-final chunk, and 0x03 for the
  final chunk (counter = chunk index, counted from 0 across all chunks).
  One Container Key encrypts exactly one Container (KD-2), so these nonces
  never repeat under a key; the counter and the final-chunk domain make
  reordering, truncation, and extension of the chunk sequence fail
  authentication. *(Form: test)*
- **FM-8.** The associated data of the meta section and of every chunk is
  the full 32-byte Container header, so altering the format version,
  Container ID, chunk size, or meta section length fails decryption.
  *(Form: test)*
- **FM-9.** The meta section is one CBOR map encrypted as a single AEAD
  message under the Container Key. Container-level fields: `schema` (= 1);
  `kind` — `one-file` or `pack`, carrying the explicit kind PK-15 defines;
  `pad_len` (FM-4); and `entries`, the entry table. Each entry records its
  `path` (an Entry Path, EP-1), `offset` and `size` in the plaintext
  stream, `mtime`, and `hash` — the BLAKE3-256 of the Entry's plaintext,
  used for end-to-end verification after decryption and for change
  detection — plus optional `derived_from` (the parent's Container ID and
  Entry Path, for derived data such as thumbnails) and optional `mime`.
  *(Form: test)*
  - `derived_from` is itself a CBOR map with two fields: `container_id`,
    the parent Entry's Container ID as a 16-byte byte string, and `path`,
    the parent's Entry Path (EP-1) as a text string.
  - The meta section's plaintext is that CBOR map followed by zero padding up
    to the next Padmé bucket boundary (FM-4), so the meta section length in the
    header (FM-2) is not a proxy for the Entry count or the total Entry Path
    length while the content stream beside it is size-blurred. CBOR is
    self-delimiting, so no length field is added: a decoder reads one CBOR
    item, and rejects the object unless every remaining plaintext byte is zero
    and the plaintext is exactly the Padmé bucket of the map's own length —
    the same two conditions FM-11 puts on a control payload. Without the
    length condition an unpadded map would pass, and its length would leak
    what the padding is there to blur.
  - The maps are forward-open: a reader ignores fields it does not know,
    and adding a field only increments `schema`. A reader accepts any
    `schema` of 1 or above and rejects anything lower.
  - `mtime` is a signed count of whole seconds from the Unix epoch;
    sub-second precision is not recorded, and negative values (before
    1970) are legal.
  - The entry table tiles the plaintext stream exactly: entries are
    contiguous from offset 0, without gaps or overlaps, and their sizes
    sum to the stream's unpadded length. A decoder rejects a table that
    does not.
  - `offset` and `size` place an Entry against chunk boundaries, which is
    what lets a client range-read one Entry of a Pack as a step in fetching
    its Container (PK-16).
- **FM-10.** The entry table of every Container — one-file or Pack — lists
  at least one Entry. A Container exists only to hold user data: no
  operation writes an empty Container, and control state never travels in
  one (Journal records, Keyrings, and Index Snapshots — ordinary and
  activation — are control objects, FM-11). This holds for every Container,
  not only for Packs (PK-3's no-empty-Pack clause). *(Form: test)*
- **FM-11.** A control object is one object laid out as:

  ```
  offset  size  field
  ------  ----  -----
  0       5     magic = "CFCTL"
  5       1     format version = 0x01
  6       1     kind (0x01 Journal / 0x02 Keyring / 0x03 Index Snapshot
                      / 0x04 activation Index Snapshot)
  7       1     reserved = 0x00
  8       8     generation
  16      2     replica index (0-based)
  18      2     replica count
  20      24    nonce (random)
  44      ...   CBOR payload ciphertext ‖ tag(16)
  ```

  The payload is encrypted with the purpose key of the header's kind
  (KD-4) and the header's random nonce; the associated data is the full
  44-byte header. A future control-object kind is assigned a new kind byte
  and its own purpose key. *(Form: test)*
  - The payload plaintext is the CBOR map followed by zero padding up to the
    next Padmé bucket boundary (FM-4). A control object is one AEAD message,
    so its stored length is its payload's length: unpadded, the length of a
    Journal record, a Keyring, or an Index Snapshot would count out for the
    provider the Entries or Containers it lists, which is what the same
    padding keeps the meta section beside it from doing (FM-9). CBOR is
    self-delimiting, so no length field is added: a decoder reads one CBOR
    item, and rejects the object unless every remaining plaintext byte is zero
    and the plaintext is exactly the length this rule gives that item.
  - An activation Index Snapshot (0x04) carries the same checkpoint content
    as an ordinary one (CK-1, CK-2, CK-3) and, beyond it, the fields activation
    needs. It is a kind of its own — with the info string of its own that
    every kind has (KD-4) — so that an ordinary Snapshot presented as a head,
    or a head presented as an ordinary checkpoint, is refused by FM-12's
    admission table and by the key, before any payload is read, and so that
    recovery and old-epoch cleanup (MR-3) can classify an object from its
    plaintext header without opening it.
- **FM-12.** Control objects carry recognizable object names, because
  recovery discovers them by name before any index exists (RV-1, RV-2, RV-3). A
  name states the object's **role** — its place in the Library's control
  state — and its kind rides in the authenticated header (FM-11):

  | name | role | kinds it admits |
  | --- | --- | --- |
  | `head-<generation>.cfrt` | a link in the control-head chain | Journal, activation Index Snapshot |
  | `idx-<generation>.cfrt` | the ordinary checkpoint of one head (CK-10) | Index Snapshot |
  | `key-<generation>-<set_digest>-r<index>-of-<count>.cfrt` | one Keyring replica (KL-14) | Keyring |

  An object whose header declares a kind the name it is presented under does
  not admit is rejected before decryption, as is one whose generation or
  replica position disagrees with that name. Heads and Index Snapshots use
  replica index 0, count 1. *(Form: test)*
  - The head chain is named without regard to kind because both its kinds
    compete for one position: a head's successor is created by conditional
    create against a single slot (CP-2, CP-3), and naming the two kinds
    differently would leave two names — and, on a Storage that keys objects
    by name, two slots — where the commit protocol needs one.
  - `<generation>`, `<index>`, and `<count>` are spelled in decimal with no
    sign and no leading zeros, so one object has exactly one name: a reader
    that accepted `head-007.cfrt` as generation 7 would let two names claim
    the same object.
  - `<set_digest>` is a non-empty string of lowercase hex digits. What the
    digest is computed over is the Keyring's business (FM-17, KL-1); the name
    only needs a single spelling per digest and a token that cannot swallow
    the `-` separators the rest of the name is parsed on.
  - Discovery follows the roles: recovery lists `head-*` for the newest head
    and `idx-*` for the newest ordinary checkpoint. A `head-`-named object
    whose header says activation Index Snapshot is a checkpoint candidate
    alongside the `idx-*` objects (CK-9, RV-1); one whose header says Journal
    record is not.
- **FM-13.** Every control-object payload carries `master_key_epoch`, the
  number of the Master Key epoch that encrypted it: 1 for the Library's
  first epoch, incremented by 1 at each rotation. The epoch is distinct
  from the header's `generation`, which places the object in the Library's
  control history and never restarts at a rotation — so an object name
  (FM-12) is never reused across epochs (CP-13, KL-10). *(Form: test)*
  - Journal records and activation Index Snapshots form one control-head
    chain: the Library's first head is written as generation 0, and every
    successor — whichever kind wins the head's commit slot (CP-2) — carries
    the head's generation plus 1, so chain generations are unique across
    both kinds (CP-6, MR-2).
    - `head-0` is a Journal record. A Library's first epoch is the one it was
      created in, so there is no earlier epoch for an activation Snapshot to
      supersede before the first commit.
  - An ordinary Index Snapshot carries the generation of the head it
    represents (CK-10), so `idx-<generation>` names that head's checkpoint
    and nothing else.
  - A Keyring's generation is the Keyring's own counter: its first set is
    generation 0, and each later set increments it by 1 (KL-10).
- **FM-14.** A Key Envelope is nonce(24) ‖ ciphertext(32) ‖ tag(16) — 72
  bytes: the Container Key encrypted under the container-wrap purpose key
  (KD-4) with a fresh random nonce, with the 16-byte Container ID as
  associated data. An envelope presented for a different Container fails
  to unwrap, so envelopes cannot be swapped between Containers.
  *(Form: test)*
- **FM-15.** A Journal record's payload (kind `0x01`) is a CBOR map with:
  `schema` (= 1); `prev`, the generation of the control head this record
  succeeds, omitted at generation 0 where there is none; `next_commit_slot`
  and `snapshot_slot`, each the Storage's own opaque token for the slot the
  record reserves — the successor's slot (CP-2) and this head's ordinary
  Index Snapshot slot (CK-10) — present as a text string where the Storage
  mints identifiers and absent where the name is the slot (CP-15);
  `keyring_generation`, `keyring_replica_count`, and `keyring_set_digest`,
  the committed Keyring tuple this commit selects (CP-10, KL-3) — the digest
  as the lowercase hex text a replica's name carries it in (FM-12) rather
  than as the bytes that text spells, so one digest has one spelling wherever
  it travels; `additions`, an array of the Containers the batch added; and
  `removals`, an array of 16-byte Container IDs the batch removed (CP-14).
  The header carries the record's own generation and the payload carries
  `master_key_epoch` (FM-11, FM-13), so neither is repeated here.
  *(Form: test)*
  - `prev` is the record's own statement of the head it was built on, so a
    replay can confirm that record *g* follows head *g − 1* from the
    authenticated payload rather than from the name the object was fetched
    under. A record above generation 0 carries `prev` equal to its header
    generation minus one; the record at generation 0 succeeded no head and
    carries none. A reader rejects anything else — a `prev` naming another
    head, a missing one above generation 0, and a `prev` on the record at
    generation 0.
  - Each element of `additions` is a map of `id` (the 16-byte Container ID),
    `kind` (`one-file` or `pack`, spelled as FM-9's `kind` spells it, PK-15),
    `ciphertext_hash` (the BLAKE3-256 of the stored object, as a byte
    string), `ciphertext_len`, optional `object_ref` (below), and `entries` —
    the Container's entry table, each element exactly FM-9's entry map (`path`,
    `offset`, `size`, `mtime`, `hash`, optional `mime`, optional
    `derived_from`). That is the meta section's own vocabulary, which is what
    lets a device replay a record without opening a Container (CP-11).
  - `object_ref` is the Storage's own identifier for the Container's object —
    the same value whichever device reads it — carried as a cache so that a
    device can fetch the Container without listing Storage first. It is never
    evidence of membership: a listing re-derives it, and a reader that cannot
    open the object it names falls back to the listing rather than failing the
    replay. FM-16 carries the same field in the same sense.
  - No Key Envelope ever appears in a record: the committed Keyring is the
    only Storage home of the keys that open Containers (CP-11, CP-12).
  - `additions` is ordered by Container ID, compared as the 16 raw bytes, and
    `removals` likewise; the `entries` of an addition keep the order of the
    Container's own entry table, which is the plaintext stream order FM-9
    fixes. One Library state therefore has one encoding: two devices
    committing the same batch produce the same map, and a record does not
    change its bytes because a writer held its additions in a different
    order.
  - A reader verifies those orders and rejects a payload that is not in them
    rather than sorting it into shape: a record that arrived out of order was
    written by something that does not follow this rule, and repairing it
    would hide that while leaving the two encodings of one state in
    circulation.
  - The maps are forward-open on FM-9's terms: a reader ignores fields it
    does not know, adding a field only increments `schema`, and a reader
    accepts any `schema` of 1 or above and rejects anything lower.
- **FM-16.** An Index Snapshot's payload is a CBOR map with: `schema` (= 1);
  the checkpoint the Snapshot stands at — `head_generation`,
  `journal_generation`, `next_commit_slot` (the same opaque token FM-15
  spells, absent where the name is the slot), `keyring_generation`,
  `keyring_replica_count`, and `keyring_set_digest`, spelled as FM-15 spells
  them (CK-1, CK-2, CK-3); `containers`, an array of the current Containers,
  each element `id`, `kind`, `ciphertext_hash`, `ciphertext_len`, and optional
  `object_ref`, as FM-15's additions spell those fields; and `entries`, an
  array of every current Entry, each element FM-9's entry map plus
  `container`, the 0-based index of the owning element of `containers`. The
  fourth member of the Keyring tuple the checkpoint names, `master_key_epoch`
  (CK-3), is the field FM-13 gives every payload, so it is not repeated
  inside. The ordinary kind (`0x03`) and the activation kind (`0x04`) share
  this schema. *(Form: test)*
  - An Entry names its Container by index rather than by ID because a Library
    holds far more Entries than Containers, and the 16-byte ID would
    otherwise be repeated once per Entry.
  - `containers` is ordered by Container ID and `entries` by the canonical
    UTF-8 bytes of the Entry Path (EP-3), so one Library state has one
    encoding and a reader can answer a prefix range over `entries` by binary
    search instead of a scan. A reader verifies both orders, verifies that
    every `container` index names an element of `containers`, and rejects an
    out-of-order or dangling payload rather than repairing it, for the reason
    FM-15 gives.
  - An activation Snapshot additionally carries `base_head_generation`, the
    generation of the head whose commit slot the activation consumed and
    whose writers it thereby fenced (CP-3, MR-2), and `activation_slot`, the
    Storage's opaque token for that slot in the form FM-15 gives every slot.
    The two fields are the activation kind's alone: an ordinary
    Snapshot carrying either is rejected, and an activation Snapshot lacking
    `base_head_generation` is rejected. `activation_slot` is not what tells
    the two kinds apart, because a name-keyed Storage persists no token at
    all (CP-2); the kind rides in the authenticated header (FM-11), and
    `base_head_generation` is the payload field that must agree with it.
  - `activation_slot` is the Snapshot's evidence that it consumed the base
    head's slot: on a Storage that mints identifiers a reader compares it with
    that head's `next_commit_slot` and rejects a mismatch, which is what makes
    the fencing checkable after the fact rather than only at the moment of the
    conditional create (MR-2). On a name-keyed Storage the field is absent and
    the head-chain name carries the same evidence (CP-2, FM-12). The comparison
    needs the base head, which the payload alone does not carry, so it belongs
    to the caller that fetched both and not to a decoder.
  - A Snapshot carries no device state (CK-7): no local root mappings, no
    local paths, no record of which Entries a device has materialized, and no
    record of which checkpoint object this Index adopted. That last one is
    the Index's own provenance rather than Library content, so it is never
    encoded and a decoded Snapshot reports none.
  - The maps are forward-open on FM-9's terms, as FM-15's are.
- **FM-17.** A Keyring replica's payload (kind `0x02`) is a CBOR map with:
  `schema` (= 1); and `mapping`, the generation's complete mapping from every
  current Container to the key status the committed control state records for it
  (KL-7). Each element of `mapping` is a map of `id` — the 16-byte Container ID
  as a byte string — and exactly one of `envelope`, that Container's 72-byte Key
  Envelope (FM-14) as a byte string, or `key_lost` (= `true`), the explicit
  key-lost marker recording that the committed control state has no reachable
  envelope for it (KL-7). The header carries the Keyring's generation and the
  replica position, and the payload carries `master_key_epoch` (FM-11, FM-13),
  so none of the three is repeated here — which is why every replica of one
  generation carries an identical payload (KL-6) and two of them differ only in
  their header and their nonce. *(Form: test)*
  - A reader rejects an element carrying both `envelope` and `key_lost`, and one
    carrying neither. A Container is mapped to exactly one of the two: the marker
    records that no envelope is reachable, so an element carrying one beside an
    envelope contradicts itself, and an element carrying neither maps its
    Container to no state at all. A `key_lost` spelled `false` is rejected as
    well, and is never read as an absent marker: the marker's presence is what
    records the loss, so `false` is a spelling of the field this rule does not
    define, and taking it for an absence would give an envelope's presence two
    spellings.
  - `mapping` is ordered by Container ID, compared as the 16 raw bytes. A reader
    verifies that order and rejects a payload not in it rather than sorting it,
    for the reason FM-15 gives. The same check catches a repeated ID: one
    Container has one element, so an ID that does not *strictly* follow its
    predecessor names a Container twice.
  - `set_digest` is not a field of this payload. It is the BLAKE3-256 of the
    `mapping` array alone — the array exactly as it appears in the payload, in
    Container ID order — spelled as the lowercase hex text that a replica's
    object name (FM-12) and the commitment tuple a commit selects (CP-10, KL-3)
    both carry it in. A reader computes it from the decoded `mapping` and
    compares it with the name it fetched the replica under, which is the digest
    half of KL-1's validity check.
    - The bytes hashed are the deterministic CBOR encoding of that array as
      RFC 8949 §4.2.1 defines it: definite-length arrays and maps, shortest-form
      arguments, and the keys of each element ordered by their encoded bytes —
      which puts `id` before either `envelope` or `key_lost`. Nothing else about
      the payload is byte-normative (a reader takes any valid CBOR spelling of
      the fields, FM-9), so this rule fixes the encoding the digest is taken
      over rather than leaving it to a serializer's choice.
    - Putting the digest inside the payload would make it cover itself. Leaving
      it out is what lets one definition serve the name, the commitment, and
      KL-1 at once.
  - The order is what makes that shared definition work at all: one mapping is
    one byte string and therefore one `set_digest`, whichever device wrote it.
    Two devices preparing or repairing the same generation therefore write the
    same `mapping` bytes, and so name the set by the same digest (KL-14) — the
    objects they store still differ, each sealed with a random nonce of its own
    (FM-11). A commitment thereby names an exact content rather than a
    particular writer's spelling of it (KL-1, KL-3).
  - That `mapping` covers every current Container and no other is KL-7's
    obligation and not a shape this rule can check: a reader of one replica sees
    a mapping, not the Library the mapping is supposed to cover. What this rule
    checks is what makes those bytes a mapping at all.
  - The maps are forward-open on FM-9's terms, as FM-15's are.
- **FM-18.** A Library's Storage Objects live flat in one **app folder**, whose
  name is `coffret-` followed by the **Library ID**: 64 bits drawn from a
  CSPRNG when the Library is created, spelled as 16 lowercase hex characters.
  The folder holds every object of that Library — Containers (FM-3) and
  control objects (FM-12) alike — each directly in it, so one name identifies
  one object within it. The one thing nested inside is the trash a Storage with
  no trash of its own keeps in a reserved segment of the folder, which no live
  name is ever looked for in. On a Storage that keys objects by name rather
  than holding folders, the app folder is the key prefix
  `coffret-<library id>/`, placed under the base prefix its Storage location
  configures — either empty or ending in `/`. *(Form: test for the folder's
  name and prefix and their derivation from the ID; prose for the override
  below — a permission over the user's own Storage mandates no behavior a test
  could require)*
  - The ID is drawn independently of the [Master Key](../../concepts/master-key/)
    and of everything derived from it, so rotating the Master Key leaves the
    folder's name alone (MR-1): a name that moved at a rotation would strand
    every other device configured against it, and one derived from key material
    would put a function of that material in a name Storage can read.
  - The ID is configuration a device keeps for the Library, not key material,
    so it is not carried in the Recovery Code (KD-11) — which is what keeps a
    Recovery Code short enough to write down.
  - A device holding nothing but a Recovery Code therefore finds the folder by
    listing the `coffret-*` names at the Storage location and keeping the one
    whose Keyring authenticates under that code's Master Key (KL-1, RV-1). That
    is the whole purpose of the `coffret-` prefix; where the folder itself sits
    — which parent folder, which base prefix — is the location's configuration,
    and coffret never reads that parent for anything but creating its folder
    under it.
  - A user may name the app folder something else, and coffret then reaches the
    Library at the name it is configured with. Discovery by enumeration does not
    find such a folder, so the user who renamed it is the one who tells a
    recovering device where it is; nothing about the Library's contents changes,
    since the name is outside every object rather than a field inside one.
