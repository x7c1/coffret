# Device Key Custody

Rule prefix: `DK`. How a device holds the Master Key between the Passphrase
that unlocks it and the lock that ends its use.

Concept background: [Passphrase](../../concepts/passphrase/),
[Master Key](../../concepts/master-key/).

## Rules

- **DK-1.** A device holds the Master Key in one of two states. **Locked**:
  only the Passphrase-protected stored form is present. **Unlocked**: the
  Master Key is usable. The correct Passphrase moves locked to unlocked, and
  a lock moves it back. *(Form: test)*
- **DK-2.** While locked, every operation needing the Master Key fails and
  reports that the Passphrase is required; none of them partially succeeds.
  *(Form: test)*
- **DK-3.** An explicit lock is available whenever the device is unlocked,
  and it has taken effect by the time it returns. *(Form: test)*
- **DK-4.** Inactivity for the configured idle interval locks the device. The
  interval is a policy parameter, not a format constant. *(Form: test)*
- **DK-5.** An incorrect Passphrase leaves the device locked. *(Form: test)*
- **DK-6.** Changing the Passphrase re-protects the stored Master Key and
  re-encrypts nothing else: the Master Key itself is unchanged, so no
  [Container](../../concepts/container/) and no control object is rewritten.
  *(Form: test)*
- **DK-7.** After a lock, no readable copy of the Master Key remains in the
  process. *(Form: prose — an absence claim over the whole process image;
  moves, freed allocations, swap, and core dumps put it past what a test can
  observe. It is honored by construction: key material lives in a type that
  overwrites itself when dropped and is never copied into a buffer outside
  that type.)*
- **DK-8.** The unlocked Master Key never reaches persistent storage in the
  clear. *(Form: prose — an absence claim over an open filesystem; swap,
  hibernation images, crash dumps, and library temporary files are written
  outside coffret's own writes, so no test refutes it. It is honored by
  construction: the key is part of no serialized structure.)*
- **DK-9.** Past the explicit lock (DK-3) and the idle interval (DK-4), how
  long a device stays unlocked is the user's choice, and the exposure of the
  unlocked Master Key follows it. *(Form: prose — a statement about the
  user's own session, which has no test form.)*
