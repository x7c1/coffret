# Storage Authorization

Rule prefix: `SA`. How a device obtains the credential it reaches a Storage
provider with: the one-time authorization flow and its PKCE and loopback
redirect, the single permission it asks for, the width of the grant it
verifies before anything is cached, what later runs mint from what was
kept, where on a device a grant is kept — once per account, opened
through an envelope each Library that uses the account holds — and what a
page polling a served Library is told of a reconnect that renews a grant.

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
  the opaque `state` value this run drew. Any other request that arrives on
  the port is answered and ignored rather than mistaken for the redirect, a
  connection that sends nothing before the server stops waiting for it is
  closed without an answer, and a redirect naming a refusal is reported
  rather than waited out. *(Form: test)*
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
- **SA-10.** A grant a served Library's Storage stopped accepting is renewed
  by a **reconnect**: the explorer asks the server to run the flow again, and
  the consent page goes back to the request that asked and to no one else.
  The work answer a page polls reports where the last reconnect stands, and
  while one waits it carries `flow`, a digest of the consent page's URL — the 32-bit
  FNV-1a of its UTF-8 bytes, as eight lowercase hex digits — so that a window
  holding the page its own request was answered with can tell whether that
  page is still the one waited on. `flow` is present only while the reconnect
  is waiting and absent once it has ended, and the work answer never carries
  the consent page or its URL: a second tab asking what the server is doing is
  told a page is open, not handed one to open. *(Form: test)*
