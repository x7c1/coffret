# Container Key

## Definition

**Container Key** is the encryption key unique to one [Container](../).
Everything inside a Container is encrypted with its Container Key. The key
itself travels inside the Container, wrapped (encrypted) under the
[Master Key](../../master-key/) — this is what makes Containers
self-describing.

## Collocations

- wrap (a Container Key under the Master Key)
- unwrap (a Container Key with the Master Key)

## Domain Rules

- Each Container has its own Container Key; keys are never shared between
  Containers.
- Rotating the Master Key re-wraps every Container Key and rebuilds every
  Container: a Container's contents are authenticated against its header,
  which includes the wrapped key, so changing the wrap changes the whole
  object. The file contents never need decrypting, but the cost is a full
  pass over the Library. The routine cheap operation is changing the
  [Passphrase](../../passphrase/), which touches only the device-local
  protection of the Master Key.

## Related Concepts

- [Container](../) — what the key encrypts and where it travels
- [Master Key](../../master-key/) — what the key is wrapped under
