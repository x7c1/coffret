# Storage Authorization

Rule prefix: `SA`. How a device obtains the credential it reaches a Storage
provider with: the one-time authorization flow and its PKCE and loopback
redirect, the single permission it asks for, the width of the grant it
verifies before anything is cached, what later runs mint from what was
kept, and where on a device a grant is kept — once per account, opened
through an envelope each Library that uses the account holds.

Concept background: [Storage](../../concepts/storage/),
[Library](../../concepts/library/),
[Purpose Key](../../concepts/purpose-key/).

## Rules

- **SA-1.** Authorization is the authorization-code flow with PKCE: the
  challenge is the SHA-256 of a per-run verifier, the verifier is sent only
  with the token exchange, and the plain challenge method is never used — it
  would put the secret in the very request the challenge protects.
  *(Form: test)*
- **SA-2.** The redirect comes back to a loopback address on a port the
  operating system hands out, and the flow accepts only a redirect carrying
  the opaque `state` value this run drew. Anything else that arrives on the
  port is answered and ignored rather than mistaken for the redirect, and a
  redirect naming a refusal is reported rather than waited out.
  *(Form: test)*
- **SA-3.** coffret asks for exactly one provider permission — a `scope`, in
  the protocol's word and the code's — and asks for no other: on Google
  Drive, the one that reaches only files this application itself created.
  Every Storage Object in a Library was written by coffret, so a wider
  permission is access no part of the design uses. *(Form: test)*
- **SA-4.** The grant is verified before anything is cached. What the token
  endpoint says it granted must be exactly the permission SA-3 asked for and
  nothing besides — not a set that merely contains it, which would wave
  through a grant reaching the whole account. A wider set, a different set,
  or a response naming no scope at all is refused, and nothing reaches the
  token cache. *(Form: test)*
  - An endpoint that says nothing verifies nothing. RFC 6749 §5.1 permits the
    field to be omitted when the grant equals the request, and refusing that
    silence is deliberate: the cost is that a provider which stopped sending
    the field fails this flow closed, with a refusal that says exactly why.
- **SA-5.** A refusal names what was granted, so the person can go and look
  at the consent they gave, and never names a token. *(Form: test)*
- **SA-6.** What the flow leaves behind is one long-lived refresh token in
  the sealed device-local cache of the account it was run for (SA-8, KD-10).
  Later runs mint short-lived access tokens from it without a person, keep
  them in memory only, and cache nothing further — so SA-4's check belongs to
  the one moment something is kept. *(Form: test for the minting a later run
  does with nobody at a browser; prose for what the token is minted from and
  what is kept once it has been, honored by construction — the minting path
  reads the account's cache and never writes it, and holds the token it mints
  in memory — and for what the provider does with a grant afterwards, which
  is observable only as that provider refusing.)*
- **SA-7.** The credential an account's cache holds is a bearer credential
  for every object this application created in that account on that
  provider — the objects of every Library kept in that account, whether or
  not a Library on this device references it (SA-8). That reach is the
  provider's and was never the Library's: a grant a Library once kept for
  itself reached another Library of the same account just as far, so keeping
  one cache per account adds no reach and removes a copy. What SA-4 keeps it
  from being is a credential for the rest of that account. *(Form: prose — a
  scoping statement about a credential's reach, honored by SA-3 and SA-4
  together rather than by an observation of its own.)*
- **SA-8.** A grant belongs to a device and an account. A device keeps one
  sealed cache per account (KD-10), under that account's own account-cache
  key (KD-12), and never a second: a Library that uses the account
  **references** it by its device-local account name, and every Library on
  the device that references one account reaches Storage through that one
  cache. *(Form: test)*
  - The **device-local account name** is the person's, because coffret
    cannot tell accounts apart by what it was granted: the one permission it
    asks for names no account (SA-3). Like a device-local Library name, it is
    chosen by the person, is never written to Storage, and never reaches a
    diagnostic event (EL-1). It is optional while the device holds one
    account and required when a second is added, so creating a Library on a
    device holding more than one account refuses to start without it. An
    account the person leaves unnamed is named `default`, the name the
    promotion below also gives, so every envelope has a name to be bound to
    (SA-9).
  - One account name binds to one OAuth client id on the device: the
    account's grant is renewed, and consented to, only through the client the
    account first consented to, since a token minted through another client
    is a different grant with a different reach.
  - Joining a Library tries each grant the device holds, and the joined
    Library references the account whose Storage has the app folder the
    person named; the authorization flow runs only when none does, and its
    grant becomes a new account on the device.
  - Renewal is per account: it replaces the account's one cache, so every
    Library that references the account uses the renewed grant from its next
    run, and none of them is renewed on its own (SA-4, SA-6).
  - A cache no Library on the device references any longer — the last
    envelope that opened it removed with its Library — can no longer be
    opened, and the device discards it the next time it opens the directory
    it keeps accounts in.
  - A Library found holding its own per-Library cache, sealed under the
    `coffret/v1/token-cache` purpose key (KD-4, KD-10), and no account-cache
    key envelope, has that cache **promoted** the first time it is opened —
    the one path by which a Library leaves its previous shape. The
    refresh token is resealed under a fresh account-cache key into an account
    named `default`, the Library gains its envelope (SA-9) and references
    `default`, and only then is the per-Library cache removed, so an
    interrupted promotion leaves the previous shape to promote again. Where
    the device already holds an account named `default`, the Library
    references it if that account's grant reaches the Library's app folder —
    the choice joining makes — and its own cache is removed unused;
    otherwise the promotion stops, says so, and asks the person to name an
    account for this Library.
- **SA-9.** A Library that references an account holds, in its own directory
  on the device, one **account-cache key envelope**: that account's
  account-cache key sealed under the Library's
  `coffret/v1/account-cache-wrap` purpose key (KD-3, KD-4), with the
  device-local account name bound as associated data, in the form KD-12 lays
  down. It is one envelope per Library, so unlocking any one Library that
  references an account opens that account's cache, and a Library's directory
  still holds everything the device keeps for that Library alone.
  *(Form: test)*
  - An envelope opened under a name other than the one it was sealed with
    fails to authenticate, so an envelope cannot be carried over to stand for
    another account; renaming an account on the device re-seals every
    envelope of that account.
  - The purpose key is derived from the Master Key and changes when it
    rotates (KD-3), so a device that moves a Library to a new Master Key
    epoch re-seals that Library's envelope under the new epoch's purpose key
    in the same step.
  - When the envelope of a Library that references an account is missing,
    malformed, or fails to authenticate, it is reported as an unreadable
    envelope, never as a Library that references no account, so a damaged
    envelope is not quietly answered with a second consent.
