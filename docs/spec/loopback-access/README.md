# Loopback Access

Rule prefix: `LA`. Who a server serving a Library answers, and how it knows:
the interface it listens on, the key it draws as it starts and publishes into
the Library's own directory, the fences every request passes before a route
sees it, and the one server at a time that serves a Library on a device.

Concept background: [Library](../../concepts/library/),
[Passphrase](../../concepts/passphrase/).

## Rules

- **LA-1.** A server serving a Library listens on the loopback interface
  alone, so the Library's plaintext is never offered to another machine on
  the network. *(Form: prose — a property of the address the binary binds,
  which no test over the server's own answers can observe; honored by
  construction and review.)*
- **LA-2.** Reaching that socket is not authorization. Every request is
  authorized before any route sees it, reads exactly as mutations are,
  because what a read answers with is the Library. *(Form: test)*
- **LA-3.** The server draws a key as it starts, from the operating system's
  CSPRNG, and writes it owner-only into the Library's own directory on this
  device. A caller shows it in a header. The boundary is therefore the
  operating system's file permissions: a caller that can read the file is a
  process of the account that owns the Library, and a page in a browser is
  not one. *(Form: test)*
- **LA-4.** The key is one running process's. It is drawn afresh at every
  start and nothing carries from one process to the next, so a key that
  leaked is spent when its server stops and a file a killed server left
  behind admits nobody. A second server for the same Library never reaches the
  point of publishing one at all (LA-8). *(Form: test)*
- **LA-5.** A request is admitted only where it names the authority the
  server actually bound, carries that key, and does not admit to coming from
  another site. A key that is shown and is wrong is answered exactly as no
  key at all, and the refusal names neither the key shown nor the one
  expected. It may name the address the server bound, which is an address the
  caller has already reached. *(Form: test)*
- **LA-6.** The key is never on a URL and never in a cookie: a URL is written
  down in referrers, histories and access logs, and a cookie is attached by
  the browser to requests the page never made — which is what LA-5's third
  fence exists against. *(Form: test)*
- **LA-7.** The shown key is compared against the expected one without
  stopping at the first byte that differs. *(Form: prose — a timing property
  of one comparison, which no test over the verdict can observe; honored by
  construction.)*
- **LA-8.** One server at a time serves a Library on a device. A server takes
  an exclusive advisory lock on a file in the Library's own directory before it
  opens the Library, and a start that finds the lock held is refused: the
  server already running keeps its key, its callers and its hold on the
  Library, and nothing about it is disturbed. The lock is the operating
  system's, so it is released however the holding process ends — a killed
  server leaves a lock nobody holds, and the next start takes it (LA-4). The
  refusal names the Library and, where it can, the process already serving it,
  and never the key or the file it is in. *(Form: test)*
