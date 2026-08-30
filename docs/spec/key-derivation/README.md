# Key Derivation

Rule prefix: `KD`. Where every key in coffret v1 comes from: the random
Master Key and Container Keys, the HKDF purpose keys and their info-string
registry, and the Argon2id protection of the Master Key at rest on a
device — along with the byte form of the device-local files those keys
seal, which no Storage Object format covers, and the transcribable form the
Master Key leaves a device in.

Concept background: [Master Key](../../concepts/master-key/),
[Container Key](../../concepts/container/container-key/),
[Passphrase](../../concepts/passphrase/),
[Key Envelope](../../concepts/key-envelope/),
[Recovery Code](../../concepts/recovery-code/).

## Rules

- **KD-1.** The Master Key is 256 bits drawn from the operating system's
  CSPRNG, and each Master Key epoch draws its own. It is never derived
  from the Passphrase or any other user-chosen input, so the strength of
  the ciphertext on Storage never depends on passphrase quality. *(Form:
  test for size and per-epoch generation; prose for the never-derived
  clause — the generator takes no user input, honored by construction and
  review)*
- **KD-2.** Each Container Key is 256 bits drawn independently from a
  CSPRNG when its Container is built — never derived from the Master Key,
  never shared between Containers. Independent keys are what let one
  Container be replaced or discarded without re-keying any other, and keep
  a future single-Container sharing path open. *(Form: test for size and
  per-Container uniqueness; prose for underivability — no derivation path
  exists by construction)*
- **KD-3.** Purpose keys come from the Master Key through HKDF-SHA-256
  with a zero-length salt, the purpose's info string (KD-4), and a 32-byte
  output. The Master Key is never used directly as an AEAD key: every use
  passes through HKDF, so adding a purpose is adding an info string.
  *(Form: test)*
- **KD-4.** The v1 purpose registry:

  | info string | derived key encrypts |
  | --- | --- |
  | `coffret/v1/container-wrap` | Container Keys, into Key Envelopes (FM-14) |
  | `coffret/v1/control/journal` | Journal record payloads (FM-11) |
  | `coffret/v1/control/keyring` | Keyring replica payloads (FM-11) |
  | `coffret/v1/control/index-snapshot` | ordinary Index Snapshot payloads (FM-11) |
  | `coffret/v1/control/activation-snapshot` | activation Index Snapshot payloads (FM-11) |
  | `coffret/v1/token-cache` | the OAuth token cache on this device (KD-10) |

  A key derived for one purpose is used for no other, and every future
  purpose — metadata keys, search-index keys, a new control-object kind —
  is assigned its own info string (RV-3). What a purpose key protects need
  not be a Storage Object: the token cache is device-local and never
  uploaded, and it is encrypted because the refresh token in it is a bearer
  credential for every object the Library holds. *(Form: test)*
- **KD-5.** The key that protects a device's stored Master Key is derived
  from that device's Passphrase with Argon2id, using a per-device random
  salt. The Argon2id parameters — memory, iterations, parallelism, salt —
  are recorded in the stored form itself. *(Form: test)*
- **KD-6.** Argon2id parameters are a device-local policy, not a format
  constant: initial values are chosen from the OWASP-recommended band
  current at release, and strengthening them later re-derives the
  protection key and re-protects only that device's stored Master Key — no
  Storage Object changes, like a Passphrase change (DK-6). *(Form: test
  for the parameter mechanism; the value choice itself is a design
  decision recorded outside this register)*
- **KD-7.** The stored form encrypts the Master Key and its
  `master_key_epoch` with XChaCha20-Poly1305 under the Passphrase-derived
  key, with the recorded Argon2id parameters bound as associated data, so
  unlocking detects both tampering and parameter downgrade. The stored
  form is self-contained and portable: unlocking needs only it and the
  Passphrase. *(Form: test)*
- **KD-8.** Nothing Passphrase-derived reaches Storage: coffret never
  uploads the stored form, the Passphrase-derived protection key, or any
  verifier of the Passphrase, so the Storage provider never holds a
  target for offline Passphrase guessing. *(Form: prose — an absence
  obligation toward an external counterparty; honored by construction:
  uploads take Storage Objects only, and none of these values is one. A
  user's own backup of the stored form to a different provider happens
  outside coffret's writes and outside this rule.)*
- **KD-9.** The stored form is one self-describing byte string:

  ```text
  offset  size  field
  ------  ----  -----
  0       5     magic = "CFMK1"
  5       1     format version = 0x01
  6       1     reserved = 0x00
  7       1     salt length S
  8       4     Argon2id memory cost in KiB
  12      4     Argon2id iterations
  16      4     Argon2id parallelism
  20      S     Argon2id salt (per device, random)
  20+S    24    nonce (random)
  44+S    40    ciphertext of Master Key(32) ‖ epoch(8)
  84+S    16    tag
  ```

  Integers are big-endian. Everything before the ciphertext is the
  associated data of KD-7's encryption. A reader follows the recorded salt
  length rather than its own build's policy, and rejects an unknown magic
  or version, a non-zero reserved byte, or a total length that disagrees
  with S. *(Form: test)*
  - The Argon2id version is 0x13 (v1.3). The stored form records no Argon2
    version field, so the version is a format constant, not a recorded
    parameter: an implementation using a different version would derive a
    different key from the same recorded parameters, and a stored form
    written by either would never unlock under the other.
- **KD-10.** A sealed OAuth token cache is one self-describing byte string:

  ```text
  offset  size  field
  ------  ----  -----
  0       5     magic = "CFTC1"
  5       1     format version = 0x01
  6       1     reserved = 0x00
  7       24    nonce (random, drawn per write)
  31      N     ciphertext of the cache's N plaintext bytes
  31+N    16    tag
  ```

  The encryption is XChaCha20-Poly1305, the construction every Storage
  Object also uses (FM-1), under the `coffret/v1/token-cache` purpose key
  (KD-3, KD-4) — not under anything Passphrase-derived, which is why no
  Argon2id parameters appear here and there is nothing in the form to
  downgrade. Everything before the ciphertext is the associated data. The
  nonce is drawn fresh on every write, since one key covers every cache a
  device ever writes. A reader rejects an unknown magic, an unknown
  version, a non-zero reserved byte, or a total length short of the fixed
  part and one tag; a file that fails any of these checks, or fails to
  authenticate, is reported as an unreadable cache and never as an empty
  one, and its bytes are never read as an unsealed cache. *(Form: test)*
  - The form seals opaque bytes: what the plaintext holds is the business of
    the adapter that keeps the cache, so an adapter may change what it
    caches without changing this rule.
  - The file is device-local and never uploaded — it is not a Storage
    Object — so KD-8 is untouched by it: nothing here is Passphrase-derived
    and nothing here reaches Storage.
- **KD-11.** A Recovery Code is the Master Key and its epoch written as one
  Bech32m string (BIP-350) — the form a user reads off paper and types into
  a new device. The payload the string carries is 41 bytes:

  ```text
  offset  size  field
  ------  ----  -----
  0       1     format version = 0x01
  1       8     master_key_epoch, big-endian
  9       32    Master Key
  ```

  The epoch precedes the key so that a later version byte may change
  everything after it. The human-readable part is `coffret` and the
  separator is `1`, so a code is `coffret1` followed by those 41 bytes
  regrouped into 66 five-bit characters — leaving two padding bits, which
  are zero — and a 6-character Bech32m checksum: 80 characters in all, well
  inside Bech32's 90-character limit. A writer emits lowercase.

  A reader first strips ASCII whitespace and `-`, which is how a code
  written down by hand comes back. It then rejects, each with an answer
  naming the check that failed: a string that is neither entirely lowercase
  nor entirely uppercase; a character outside the Bech32 alphabet, or no
  separator to divide the string at; a Bech32m checksum that does not verify;
  a human-readable part that is not `coffret`; a data part that is not the 66
  characters 41 payload bytes take; non-zero padding bits; a version byte
  other than `0x01`; and an epoch of 0, which numbers no epoch (FM-13). A
  code that fails any of these yields no key material at all — a mistyped
  code never opens a Library under the wrong key. *(Form: test)*
  - Printing groups everything after `coffret1` in fours — the 66 data
    characters and the checksum alike, 18 groups in all — separated by single
    spaces (`coffret1 qpzr y9x8 …`). The grouping is presentation and not part
    of the form: the reader strips it along with any other whitespace, so two
    spellings that strip to the same string are the same code.
  - The pair travelling here is the pair KD-9's stored form protects, so
    whoever holds either holds a Library's key and the epoch that says what
    it opens. Nothing here is Passphrase-derived and nothing here reaches
    Storage, so KD-8 is untouched by it.
