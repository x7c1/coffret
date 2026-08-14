# Key Envelope

## Definition

**Key Envelope** is a [Container Key](../container/container-key/) wrapped
(encrypted) under the [Master Key](../master-key/). Each
[Container](../container/) has exactly one current Key Envelope; opening a
Container means unwrapping its envelope and decrypting with the recovered
Container Key. Control Storage Objects do not use Key Envelopes.

An envelope is bound to its Container's id, so an envelope cannot be swapped
between Containers. Envelopes live outside the Containers in the
[Keyring](../keyring/), which is what keeps Master Key rotation from touching
the Containers themselves; a [Journal](../journal/) record changing Container
membership selects the Keyring generation that owns the matching envelopes
([spec: CP-10](../../spec/commit-protocol/)).

## Examples

- Rotating the Master Key re-wraps every Key Envelope; the Containers on
  Storage stay byte-identical

## Collocations

- unwrap (a Key Envelope into its Container Key)
- re-wrap (every Key Envelope when rotating the Master Key)

## Domain Rules

- One Container, one current Key Envelope. A Container without a reachable
  envelope is unreadable even with the Master Key.
- The committed Keyring is the only Storage representation of envelopes;
  they never travel inside their Container or a Journal record
  ([spec: CP-11](../../spec/commit-protocol/)).

## Related Concepts

- [Container Key](../container/container-key/) — what an envelope wraps
- [Keyring](../keyring/) — where the current envelopes are collected
- [Master Key](../master-key/) — what envelopes are wrapped under
- [Container](../container/) — what an envelope opens
