# Purpose Key

## Definition

**Purpose key** is a key derived from the [Master Key](../master-key/) for
exactly one job — one kind of data, on [Storage](../storage/) or on the
device — named by an info string in a fixed registry. The Master Key
is never used as an encryption key itself: every use passes through HKDF
with one info string, so ciphertexts made for different jobs never share a
key, and a new kind of data gets its own key by adding an info string
instead of touching any existing one (spec: KD-3, KD-4).

## Examples

- The `coffret/v1/control/journal` purpose key, which encrypts Journal
  record payloads
- The `coffret/v1/container-wrap` purpose key — wrapping a Container Key
  "under the Master Key" concretely means encrypting it under this key
- The `coffret/v1/account-cache-wrap` purpose key, which wraps the
  account-cache key that opens the grant a device keeps for a Storage account
  into this Library's account-cache key envelope

## Collocations

- derive (a purpose key from the Master Key)
- seal (data under a purpose key, on Storage or on the device)
- open (data sealed under a purpose key)

## Domain Rules

- A key derived for one purpose is used for no other; every future purpose —
  metadata keys, search-index keys, a new control-object kind — is assigned
  its own info string (spec: KD-4, RV-3).
- Purpose keys open control [Storage Objects](../storage-object/) directly,
  with no [Key Envelope](../key-envelope/) in between, which is what keeps
  recovery's bootstrap acyclic (spec: RV-3).
- A purpose may equally protect state that never leaves the device — the
  account-cache key envelope is one — in which case its key opens no Storage
  Object at all (spec: KD-4, KD-12).
- Derivation is deterministic in the Master Key and the info string, so a
  purpose key exists wherever the Master Key is present and changes exactly
  when the Master Key rotates (spec: KD-3).

## Related Concepts

- [Master Key](../master-key/) — what every purpose key is derived from
- [Storage Object](../storage-object/) — control objects are encrypted
  directly with purpose keys
- [Key Envelope](../key-envelope/) — produced under the container-wrap
  purpose key
- [Specification register](../../spec/) — the behavioral rules cited by ID
