# Container Key

## Definition

**Container Key** is the encryption key unique to one [Container](../) —
per-Container keys are what let one Container be replaced or discarded
without re-keying any other. Everything inside a Container is encrypted with
its Container Key; control [Storage Objects](../../storage-object/) use
purpose-specific keys derived from the Master Key instead.
The Container Key itself travels outside the Container as a
[Key Envelope](../../key-envelope/) — the key encrypted under the
[Master Key](../../master-key/) — and the [Keyring](../../keyring/) collects
those envelopes.

## Collocations

- wrap (a Container Key under the Master Key) — encrypt one key with another
- unwrap (a Container Key from its Key Envelope)

## Domain Rules

- Each Container has its own Container Key; keys are never shared between
  Containers (spec: KD-2).
- Rotating the Master Key re-wraps every Container Key and refreshes the
  compact control objects — a few MB — but never touches the Containers
  themselves (spec: MR-1).
  - The other routine cheap operation is changing the
    [Passphrase](../../passphrase/), which touches only the device-local
    protection of the Master Key.

## Related Concepts

- [Container](../) — what the key encrypts
- [Key Envelope](../../key-envelope/) — the key's wrapped, travelling form
- [Master Key](../../master-key/) — what the key is wrapped under
- [Specification register](../../../spec/) — the behavioral rules cited by ID
