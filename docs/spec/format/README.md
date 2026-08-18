# Storage Object Format

Rule prefix: `FM`. The byte-level form of every Storage Object: the
Container v1 header and its chunked AEAD framing, the encrypted meta
section, size padding, the common control-object framing, the Key Envelope
form, and the names objects carry on Storage.

Concept background: [Storage Object](../../concepts/storage-object/),
[Container](../../concepts/container/),
[Entry](../../concepts/container/entry/),
[Key Envelope](../../concepts/key-envelope/).

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
  28      4     meta section length M (ciphertext bytes)
  32      M     meta section (encrypted, FM-9)
  32+M    ...   chunk sequence (encrypted, FM-5)
  ```

  An object with an unknown magic or format version is rejected without
  attempting decryption; reserved bytes must be zero. The header carries no
  key material — Key Envelopes live in the Keyring only (CP-11) — which is
  what lets Master Key rotation leave Containers byte-for-byte unchanged
  (MR-1). *(Form: test)*
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
  - The maps are forward-open: a reader ignores fields it does not know,
    and adding a field only increments `schema`.
  - `offset` and `size` place an Entry against chunk boundaries, which is
    what lets a client range-read one Entry of a Pack as a step in fetching
    its Container (PK-16).
- **FM-10.** The entry table of every Container — one-file or Pack — lists
  at least one Entry. A Container exists only to hold user data: no
  operation writes an empty Container, and control state never travels in
  one (Journal records, Keyrings, and Index Snapshots are control objects,
  FM-11). This holds for every Container, not only for Packs (PK-3's
  no-empty-Pack clause). *(Form: test)*
- **FM-11.** A control object is one object laid out as:

  ```
  offset  size  field
  ------  ----  -----
  0       5     magic = "CFCTL"
  5       1     format version = 0x01
  6       1     kind (0x01 Journal / 0x02 Keyring / 0x03 Index Snapshot)
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
- **FM-12.** Control objects carry recognizable object names, because
  recovery discovers them by name before any index exists (RV-1 to RV-3):
  `jrn-<generation>.cfrt` for Journal records, `idx-<generation>.cfrt` for
  Index Snapshots, and
  `key-<generation>-<set_digest>-r<index>-of-<count>.cfrt` for Keyring
  replicas (KL-14). An object whose name-encoded kind, generation, or
  replica position disagrees with its header is rejected. Journal records
  and Index Snapshots use replica index 0, count 1. *(Form: test)*
- **FM-13.** Every control-object payload carries `master_key_epoch`, the
  number of the Master Key epoch that encrypted it: 1 for the Library's
  first epoch, incremented by 1 at each rotation. The epoch is distinct
  from the header's `generation`, which counts that object kind's own
  updates within an epoch (CP-13, KL-10). *(Form: test)*
- **FM-14.** A Key Envelope is nonce(24) ‖ ciphertext(32) ‖ tag(16) — 72
  bytes: the Container Key encrypted under the container-wrap purpose key
  (KD-4) with a fresh random nonce, with the 16-byte Container ID as
  associated data. An envelope presented for a different Container fails
  to unwrap, so envelopes cannot be swapped between Containers.
  *(Form: test)*
