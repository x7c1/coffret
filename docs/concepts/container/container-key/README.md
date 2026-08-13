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
- Rotating the Master Key only requires re-wrapping Container Keys; the
  encrypted data itself is untouched.

## Related Concepts

- [Container](../) — what the key encrypts and where it travels
- [Master Key](../../master-key/) — what the key is wrapped under
