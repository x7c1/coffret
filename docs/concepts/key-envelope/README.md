# Key Envelope

## Definition

**Key Envelope** is a [Container Key](../container/container-key/) wrapped
(encrypted) under the [Master Key](../master-key/). The committed
[Keyring](../keyring/) maps each current [Container](../container/) to one of
two things — normally its current Key Envelope, or an explicit key-lost
marker if no copy of the key survives (spec: KL-7); opening a
Container means unwrapping its envelope and decrypting with the recovered
Container Key. Only Containers are opened through envelopes; control
[Storage Objects](../storage-object/) are opened with keys derived directly
from the Master Key (spec: RV-3).

An envelope is bound to its Container's id, so an envelope cannot be swapped
between Containers. Envelopes live outside the Containers in the
Keyring, which is what keeps Master Key rotation from touching
the Containers themselves; a [Journal](../journal/) record that adds or
removes Containers selects the Keyring generation that owns the matching
envelopes (spec: CP-10).

## Examples

- Rotating the Master Key re-wraps every Key Envelope; the Containers on
  Storage stay byte-identical

## Collocations

- unwrap (a Key Envelope into its Container Key)
- re-wrap (every Key Envelope when rotating the Master Key)

## Domain Rules

- A Container never has more than one current Key Envelope, and without a
  reachable envelope it is unreadable even with the Master Key — a state the
  committed Keyring records as an explicit key-lost marker
  (spec: KL-7, RV-7).
- The committed Keyring is the only Storage representation of envelopes;
  they never travel inside their Container or a Journal record
  (spec: CP-11).

## Related Concepts

- [Container Key](../container/container-key/) — what an envelope wraps
- [Keyring](../keyring/) — where the current envelopes are collected
- [Master Key](../master-key/) — what envelopes are wrapped under
- [Container](../container/) — what an envelope opens
- [Specification register](../../spec/) — the behavioral rules cited by ID
