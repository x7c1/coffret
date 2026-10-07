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
  - An unlock is held by one process, and that process ending is a lock: a
    one-shot command that took the Passphrase to do its work is locked by the
    time it has exited, however it exits, and the next command starts locked
    and asks for the Passphrase again (DK-2). Such a process has no idle
    lock, and needs none — the unlocked Master Key lives only in its memory
    (DK-8), so nothing outlives it for a later lock to end. DK-4 stands as
    written: a process that stays unlocked across more than one piece of
    work, such as a server serving a Library, owes it, and a one-shot one
    meets it by ending. This is said here rather than as a scope on DK-4
    because it is a fact about the two states, which it keeps exhaustive for
    every process. The same holds for a server: stopping it ends its hold on
    the keys, which is why there is no lock to ask a running one for. A
    running server that has locked may be unlocked in place, but only with
    the Passphrase taken from a prompt that does not echo (DK-10). Two
    surfaces are told apart here: the explorer's page in the system browser,
    which never takes or carries a secret (the server key and every secret
    stay off it, LA-3, LA-6), and the desktop app's own window, a page the
    app itself ships and renders, which is the one surface that takes the
    Passphrase. The prompt is that window, not the explorer's page.
    Whatever brings the window forward — a press of *unlock*, the tray, or
    the explorer asking on its own once it sees the Library locked — decides
    only when it is shown; the Passphrase's path from the window to the
    unlock is the same. A server started from the command line has no such
    prompt, and is unlocked by starting it again with the Passphrase.
    *(Form: test for the next process starting locked,
    and for a running server unlocked in place — served again, and locked
    again by DK-4 after the interval; prose for the unlock ending with the
    process, honored by construction: the key is part of no serialized
    structure and of nothing a process leaves behind.)*
- **DK-2.** While locked, every operation needing the Master Key fails and
  reports that the Passphrase is required; none of them partially succeeds.
  *(Form: test)*
- **DK-4.** Inactivity for the configured idle interval locks the device. The
  interval is a policy parameter, not a format constant. *(Form: test)*
  - Activity is the span of a keyed operation and not the moment a request
    arrived: the hold a piece of work takes on the unlocked Master Key counts
    as somebody wanting the Library from the moment it is taken to the moment
    it is let go — so work that outlasts the interval defers the lock rather
    than meeting it. A request that needs no key is not activity, since an open
    window asking what a device is doing is not a person at the keyboard.
  - A device that has locked itself says so when asked what it is doing, so
    that a window left open over what it decrypted can give up that plaintext
    rather than hold it until its next request is refused. Asking remains no
    part of the activity that defers the lock, so the interval runs out under
    the very asking that reports it. *(Form: test)*
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
    purpose keys (KD-3), the Container Keys those unwrap (KD-2), the
    account-cache keys a Library's envelope unwraps (KD-12), the Recovery
    Code form the key is written out as (KD-11), and the grouped key sets a run
    works under. Together these are the **secret-bearing inventory**, and a new
    type that comes to hold secret bytes joins it.
  - For a secret entered in the desktop app's window (DK-1), the claim begins
    where the app's own command handler receives it: from that moment it is
    held in a type on the inventory, with no copy outside it. The copies the
    webview and its IPC make before then — the field's value, the page
    script's string, the serialized message — are outside anything coffret
    can overwrite, and outside the claim. The app shortens their life — the
    window's field is emptied after every submission and whenever the window
    is hidden — without guaranteeing their erasure.
  - Inventory membership is the testable half of this rule: every type on the
    list overwrites its bytes when it is dropped and none of them is copyable,
    which one place asserts over the domain's types rather than each type
    asserting its own; a type on the list that only a shell holds, out of
    sight of the domain crates, is asserted by the same two checks in that
    shell's crate. The absence claim above stays prose; this half is
    *(Form: test)*.
- **DK-8.** The unlocked Master Key never reaches persistent storage in the
  clear. *(Form: prose — an absence claim over an open filesystem; swap,
  hibernation images, crash dumps, and library temporary files are written
  outside coffret's own writes, so no test refutes it. It is honored by
  construction: the key is part of no serialized structure.)*
- **DK-9.** Past the idle interval (DK-4) and the process ending (DK-1), how
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
