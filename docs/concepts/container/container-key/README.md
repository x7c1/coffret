# Container Key

## Definition

**Container Key** is the encryption key unique to one [Container](../).
Everything inside a Container is encrypted with its Container Key. The key
itself travels outside the Container as a
[Key Envelope](../../key-envelope/) — its form wrapped under the
[Master Key](../../master-key/) — collected in the
[Keyring](../../keyring/).

## Collocations

- wrap (a Container Key under the Master Key)
- unwrap (a Container Key with the Master Key)

## Domain Rules

- Each Container has its own Container Key; keys are never shared between
  Containers.
- Rotating the Master Key re-wraps every Container Key — a rewrite of the
  Keyring, a few MB — and never touches the Containers themselves. The
  other routine cheap operation is changing the
  [Passphrase](../../passphrase/), which touches only the device-local
  protection of the Master Key.

## Related Concepts

- [Container](../) — what the key encrypts
- [Key Envelope](../../key-envelope/) — the key's wrapped, travelling form
- [Master Key](../../master-key/) — what the key is wrapped under
