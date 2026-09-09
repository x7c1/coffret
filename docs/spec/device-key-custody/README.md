# Device Key Custody

Rule prefix: `DK`. How a device holds the Master Key between the Passphrase
that unlocks it and the lock that ends its use, and how a secret is entered at
a device in the first place.

Concept background: [Passphrase](../../concepts/passphrase/),
[Master Key](../../concepts/master-key/),
[Recovery Code](../../concepts/recovery-code/).

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
  - Activity is the span of a keyed operation and not the moment a request
    arrived: the hold a piece of work takes on the unlocked Master Key counts
    as somebody wanting the Library from the moment it is taken to the moment
    it is let go — so work that outlasts the interval defers the lock rather
    than meeting it. A request that needs no key is not activity, since an open
    window asking what a device is doing is not a person at the keyboard.
- **DK-5.** An incorrect Passphrase leaves the device locked. *(Form: test)*
- **DK-6.** Each device has its own Passphrase. Changing it re-protects only
  that device's stored Master Key; it changes no other device's Passphrase or
  stored copy. The Master Key itself is unchanged, so no
  [Container](../../concepts/container/) and no control object is rewritten.
  *(Form: test)*
- **DK-7.** After a lock, no readable copy of the Master Key remains in the
  process. *(Form: prose — an absence claim over the whole process image;
  moves, freed allocations, swap, and core dumps put it past what a test can
  observe. It is honored by construction: key material lives in a type that
  overwrites itself when dropped and is never copied into a buffer outside
  that type.)*
  - The claim covers not only the Master Key but everything a device holds that
    carries it, is derived from it or wrapped under it, or unlocks it: the
    Passphrase that unlocked it, the key that Passphrase derives (KD-5), the
    purpose keys (KD-3), the Container Keys those unwrap (KD-2), the Recovery
    Code form the key is written out as (KD-11), and the grouped key sets a run
    works under. Together these are the **secret-bearing inventory**, and a new
    type that comes to hold secret bytes joins it.
  - Inventory membership is the testable half of this rule: every type on the
    list overwrites its bytes when it is dropped and none of them is copyable,
    which one place asserts over the whole list rather than each type
    asserting its own. The absence claim above stays prose; this half is
    *(Form: test)*.
- **DK-8.** The unlocked Master Key never reaches persistent storage in the
  clear. *(Form: prose — an absence claim over an open filesystem; swap,
  hibernation images, crash dumps, and library temporary files are written
  outside coffret's own writes, so no test refutes it. It is honored by
  construction: the key is part of no serialized structure.)*
- **DK-9.** Past the explicit lock (DK-3) and the idle interval (DK-4), how
  long a device stays unlocked is the user's choice, and the exposure of the
  unlocked Master Key follows it. *(Form: prose — a statement about the
  user's own session, which has no test form.)*
- **DK-10.** A secret a device is given to hold — the Passphrase that protects
  its stored Master Key, and the Recovery Code that carries the Master Key
  onto it — is taken from a non-echoing prompt, or, where a script selects
  that explicitly, from one line of standard input. Neither is ever taken from
  a command-line argument, so neither becomes part of the process's argument
  list or of a shell history, and no refusal repeats what was entered.
  *(Form: test)*
  - A Recovery Code is bounded in length whichever way it is given: an entry
    longer than any spelling of a code (KD-11) is refused rather than read on
    as a candidate, and nothing a pipe offers is truncated into a shorter code.
    A Passphrase has no form to be checked against and so no such bound; what
    ends it is the line it was read from.
  - Where both are given on standard input, the Recovery Code is the first
    line and this device's Passphrase the next, and the reader of the first
    takes exactly one line so the second is still there for the reader of the
    Passphrase. Redirected input is never consumed as though the script had
    selected it: an unattended run says so.
