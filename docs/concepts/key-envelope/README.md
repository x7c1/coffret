# Key Envelope

## Definition

**Key Envelope** is a [Container Key](../container/container-key/) wrapped
(encrypted) under the [Master Key](../master-key/). Each
[Container](../container/) has exactly one current Key Envelope; opening a
Container means unwrapping its envelope and decrypting with the recovered
Container Key. Control Storage Objects do not use Key Envelopes.

An envelope is bound to its Container's id, so an envelope cannot be swapped
between Containers. Envelopes live outside the Containers — in the
[Keyring](../keyring/) and in [Journal](../journal/) additions — which is
what keeps Master Key rotation from touching the Containers themselves.

## Examples

- Rotating the Master Key re-wraps every Key Envelope; the Containers on
  Storage stay byte-identical

## Collocations

- unwrap (a Key Envelope into its Container Key)
- re-wrap (every Key Envelope when rotating the Master Key)

## Domain Rules

- One Container, one current Key Envelope. A Container without a reachable
  envelope is unreadable even with the Master Key.
- At every successful commit or `prune` boundary, every current Container's
  envelope exists in a complete Keyring replica set. The configured replica
  count is a redundancy policy, initially three. Keyring replicas establish
  this redundancy before the Journal makes a new Container current.
- Envelopes never travel inside their Container; they are carried by the
  Keyring and by the Journal record that added the Container.

## Related Concepts

- [Container Key](../container/container-key/) — what an envelope wraps
- [Keyring](../keyring/) — where the current envelopes are collected
- [Master Key](../master-key/) — what envelopes are wrapped under
- [Container](../container/) — what an envelope opens
