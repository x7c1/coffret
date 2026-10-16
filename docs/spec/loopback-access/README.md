# Loopback Access

Rule prefix: `LA`. Who a server serving a Library answers, and how it knows:
the interface it listens on, the key it draws as it starts and publishes into
the Library's own directory, the fences every request passes before a route
sees it, the budgets one request carrying files in is taken within, the one
server at a time that serves a Library on a device, and the account of the
work it runs that it holds for as long as it runs. Then the **explorer host**
in front of it: the process on the same device that serves the explorer's page
and forwards that page's `/api` requests to the server with the server key
attached, the one authority it answers to, the **browser token** it admits a
browser by, and the one-time **launch ticket** a browser exchanges for that
token.

LA-1 to LA-12 are the contract of the server itself, and LA-13 onwards the
contract of the explorer host. The two credentials never cross: the server key
is what the host shows the server, a browser token is what a browser shows the
host, and neither of the two accepts the other's.

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
  process of the account that owns the Library. A page in a browser cannot
  read the file and is never handed what is in it; what a browser holds
  instead is a browser token, which only the explorer host accepts and which
  the server refuses like any other wrong key (LA-15). *(Form: test)*
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
- **LA-6.** The server key is never on a URL and never in a cookie: a URL is
  written down in referrers, histories and access logs, and a cookie is
  attached by the browser to requests the page never made — which is what
  LA-5's third fence exists against. It never reaches a browser at all
  (LA-3). A browser's own credential is held to the same line: a browser
  token travels only in an `Authorization` header, and a launch ticket only in
  the ticket file, in the fragment of the address that file refreshes to,
  which no request carries and which the page removes as it reads it (LA-21),
  and in the `Authorization` header of its exchange (LA-16). *(Form: test)*
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
- **LA-9.** One request carrying files into the Library is taken within three
  budgets of this server's own: at most 64 GiB across the whole request,
  framing included; at most 1 GiB of file content in any one **part** — one
  field of the multipart body such a request carries; and at most 4096
  parts, counting every part the request carries and not only the ones
  that name a file. What they bound is one HTTP request arriving on this
  device's loopback socket, and nothing beyond it. They put no number on
  how large a file may be, and no rule elsewhere puts one there: an Entry
  larger than a Pack's size target is a Pack of its own rather than a file
  refused (PK-3). What the format bounds instead is how much a Container's
  entry table or a Library's checkpoint comes to (FM-2, FM-11), and never
  the size of any one Entry. What this server has that the Library has not
  is a socket, on which somebody who is not the explorer can send whatever
  they like for as long as they like; these budgets bound that. *(Form: test
  for the three budgets, which a case can drive a server within; prose for
  what they are not — that no bound on a file's size exists elsewhere is a
  claim about the rest of the system rather than about any answer this server
  gives, honored by construction and review.)*
- **LA-10.** A request that passes one of LA-9's budgets stops where it
  stands. It is the request that is refused and not a file: the parts behind
  it are never looked at, no answer lists them, and the server answers in the
  middle of what the caller is still sending rather than reading out a request
  already known to be more than it takes. What landed before it stays in the
  folder with nothing armed behind it, and the part it stopped at leaves
  nothing — neither a file under a final name nor the scratch name its bytes
  were going to (EP-11). Where one part passed a budget on its own, the
  sentence names that part, because whoever dropped hundreds of files has
  nothing to act on until they know which one; the record beside it says which
  budget was passed and never the name (EL-1). *(Form: test)*
- **LA-11.** Before a part is taken, the server asks the volume this device's
  mapped folder is on whether it has room for what is still coming, and
  refuses the drop where it has not. It is asked of each part that names a
  file, once the scratch its bytes are going to stands (EP-11) and before a
  byte is written to it, so the volume asked about is the one those bytes will
  land on, and a drop that would run the disk out is refused while there is
  still room to refuse it in. What is still coming is what the request
  declared it was bringing less what has landed, and one part's ceiling
  (LA-9) where it declared nothing: a caller that streams without saying how
  much is not refused for that, and is asked for the room one part could take
  instead. It is a courtesy fence and not a quota: nothing is reserved, nothing
  is accounted for, and the answer is only what the volume said a moment ago.
  A volume that cannot be asked at all refuses the request rather than opening
  the fence. The refusal is this device's own state rather than anything the
  caller did, and how much room the disk has stays out of the sentence and
  goes in the record (EL-1). *(Form: test)*
- **LA-12.** What a server holds about the work it runs — which fill, sync, or
  freeze is running and how far it has got, which folders wait behind it, and
  what the last run of each kind came to — is the running process's own state.
  It lives exactly as long as the process: a server started afterwards answers
  with no run under way, nothing waiting, and no last outcome, and work a
  stopped server had waiting is done only once something asks for it again.
  Every answer about that work carries an identifier the process draws as it
  starts, so a browser holding what one process told it can tell that account
  from the next process's. What such work does that lasts is recorded by the
  work itself, under the rules for it — a materialization record (EP-10), a
  commit (CP-1), a pending row a run leaves for the next settle (OC-2) — so a
  server that stops loses the account of what this device was doing and none
  of the record of what it has. *(Form: test for what a server started
  afterwards answers; prose for that account reaching no file — the catalog,
  the Library's directory, Storage — which is an absence claim over the
  device's files, honored by construction: the account is held in memory, in
  no serialized structure.)*
  - A run's reported status and its refusal go together: a fill, sync, or
    freeze whose status is `stopped` carries the refusal that stopped it —
    whatever refused the run, be it Storage, a Library locked before the run
    began, this device's own disk, or the worker ending without an answer —
    and a run in any other status carries none. The catalog answers the same
    way: it stands `behind` exactly when it carries the refusal that stopped
    the last catch-up. A run a later one displaced from the record is
    one that had stopped, so it is reported as `stopped` and always with its
    refusal. *(Form: test)*
  - A sync, a freeze, and a deletion each own this device's pending work for
    the whole of a run (OC-2). Within one server they take **turns** at it
    rather than refusing each other: a run armed while another holds the turn
    waits, and the turns go in the order the runs ask for them. What the server
    holds of its running work names the run holding the turn and the runs
    waiting for it, and a waiting run is reported as waiting for the holder.
    Across processes there is no queue: of a server's run and a sync or a
    freeze from the command line, whichever comes second is refused at once,
    changing nothing — a server's run as a `conflict` — and goes through when
    it is run again after the other finishes. *(Form: test)*

### The explorer host

- **LA-13.** The explorer host listens on IPv4 loopback alone (LA-1), and its
  authority is fixed before it answers anything: it binds first, then builds
  what answers around the `127.0.0.1:<port>` it actually bound, and its own
  origin is `http://127.0.0.1:<port>`. It never derives either from a request.
  A request is answered only where its `Host` is that authority exactly, and
  is refused where `Host` is missing, given more than once, or malformed, where
  it names `localhost`, another port or another name, and where the request
  target is in absolute form and names an authority other than that one.
  `Forwarded` and every `X-Forwarded-*` header are ignored: nothing in front of
  the host is trusted to speak for the caller. What this closes is DNS
  rebinding, where a name an attacker controls comes to resolve to loopback
  and a page under that name would otherwise be taken for the host's own.
  *(Form: test for what a request is answered with; prose for the bound
  address being IPv4 loopback, for the reason LA-1 gives.)*
- **LA-14.** Every request under `/api`, a path no route answers included,
  passes one layer of fences, in this order, before anything else is done with
  it — one layer, so that no route can be left outside it:
  1. `Host` is the fixed authority (LA-13), and otherwise the request is
     refused as `403 origin_refused`.
  2. `Origin`, where the request carries one, is the fixed origin exactly; an
     `Origin` of `null`, or one given more than once, is refused the same way.
     `Sec-Fetch-Site`, where the request carries it, is `same-origin`. These
     two fences keep another site's page in the person's own browser out; they
     are not authentication, since a client that is not a browser sets both to
     whatever it likes.
  3. `Authorization` carries `Bearer <browser token>`, given once, and the
     token is one this host issued and still holds (LA-18). A missing,
     duplicated or malformed header and an unknown, expired or revoked token
     are all one answer, `401 authentication_required`, which names none of
     them, and the comparison does not stop at the first byte that differs
     (LA-20).
  4. The token's session is of this host's current generation and Library
     (LA-18). While the Library is locked, only the request that unlocks it
     and the minimal status a page needs to say so pass; every other request
     is answered `423 library_locked`.
  5. Only then is the request forwarded. `Authorization`, `Cookie`, the server
     key's header, `Forwarded` and every `X-Forwarded-*` header the caller
     sent are removed, and the server key and the server's own `Host` are
     attached in their place.

  Until a request has passed all five, the host neither reads the server
  key's file nor opens a connection to the server, so a refused request costs
  the server nothing and learns nothing about it. *(Form: test)*
- **LA-15.** The two credentials never stand in for each other. The host
  never forwards a browser token to the server, and the server never accepts
  one; the host never accepts the server key from a browser as a credential of
  its own. A host that cannot read the server key's file, that finds the
  server stopped, or that finds a server of another run behind its port (LA-4)
  fails closed: it forwards nothing, and it revokes every session it holds,
  rather than reading a newer key on behalf of a token issued while an older
  one stood. Nor does it carry anything of the server's answer that would make
  a credential of it: it follows no redirect, passes no `Set-Cookie` through,
  and emits no `Location` that carries a credential. *(Form: test)*
- **LA-16.** What the host answers without a browser token is a closed list:
  - `/` and the explorer's bundled assets, to a request whose `Host` is the
    fixed authority, with framing by any other page forbidden;
  - `POST /auth/launch`, with the `Host` and `Origin` fences of LA-14 and a
    launch ticket in `Authorization: Launch <ticket>`, which answers a new
    browser token, how many seconds it is valid for as `expires_in`, and the
    host's `instance_id` (LA-18). Its body is JSON: a body that is not JSON is
    `400`, and a request whose `Content-Type` is not JSON is `415`;
  - `GET /auth/session`, with a browser token, which answers when its session
    expires and whether the Library is locked or unlocked, and nothing else;
  - `POST /auth/session/revoke`, with a browser token, which revokes that
    token's own session and no other.

  `/auth` and everything under it is the host's own and is never forwarded,
  and a path under it that is not on this list is a fixed `404`. The host
  answers no cross-origin request: it sends no `Access-Control-Allow-*`
  header, and an `OPTIONS` request is refused like any other method a route
  does not take. *(Form: test)*
- **LA-17.** A launch ticket is issued only by a call within the host's own
  process — the desktop app's, as it opens the explorer — and by no HTTP
  request whatever. It is good for one exchange, and it expires 60 seconds
  after it was issued. At most 4 tickets are live at once, and issuing a fifth
  expires the oldest one not yet exchanged. Of concurrent exchanges of one
  ticket, exactly one succeeds. A ticket that was exchanged, expired, never
  issued, or issued before the host's generation changed is refused as one
  answer, `401`, that names none of these. Exchanges are limited across the
  whole host, not per caller, to 10 a second with a burst of 20, and past that
  are answered `429` with a `Retry-After` saying when the next is let through.
  *(Form: test for what an exchange is answered with; prose for no HTTP
  request reaching the issuing call, which is honored by construction: the
  call is on a handle no request handler is given.)*
- **LA-18.** A browser token stands for one **session**, of one Library and
  one generation of the host: the run of the host and the server behind it
  during which neither restarted and the Library served did not change. The
  host draws an `instance_id` for each generation and hands it to the page with
  every token, so that a page can tell one generation from the next; it is
  public, and admits nothing. At most 8 sessions are held at once, and an
  exchange that would make a ninth is not refused: the host revokes the
  session whose token was used least recently — used meaning authenticated
  by any request, `/auth/session` included, or else issued — and admits the
  new one. A cap that refused would refuse the owner, since closing a tab ends
  no session; one that evicts is safe, since an exchange needs a ticket, which
  only the owner's account can come by (LA-17, LA-21). The eviction is
  recorded as an event by its kind, a fixed reason and a count alone (EL-1).
  A session expires 12 hours after its token was issued, and nothing
  refreshes it: a page whose session ended exchanges a new ticket, which only
  the app can give it. Every ticket and token is discarded when the host or the
  server restarts and when the Library it serves changes; a Library that locks
  keeps its sessions, which are then refused data (LA-14). A session is
  revoked singly by a request carrying its own token (LA-16) — which the
  explorer offers no control to make — and all of them at once by the app,
  which spends every ticket not yet exchanged as well, so that no ticket makes
  a session after every session was revoked. *(Form: test)*
- **LA-19.** What the host reads of a request is bounded before it is read. A
  request to `/auth` brings at most 1 KiB of body, any request at most 8 KiB
  of headers, and reading a request to `/auth` stops 5 seconds after it began.
  A request carrying a large body — an upload — that is refused by LA-14 is
  refused without its body being read, and the connection it came on is
  closed rather than drained. *(Form: test)*
- **LA-20.** A launch ticket and a browser token are each 32 bytes from the
  operating system's CSPRNG, written as unpadded base64url — 43 characters —
  and anything presented as either is checked for that length and alphabet
  before it is looked up. The host holds each only as its SHA-256 digest, and
  holds those digests in memory alone: never on disk, and never as an
  identifier in an event (EL-1). A presented credential's digest is compared
  against those held without stopping at the first byte that differs.
  *(Form: test for the encoding and its validation; prose for the rest — that
  no copy reaches a disk or an event is an absence claim, and the comparison's
  timing is what no test over the verdict observes, as with LA-7; both are
  honored by construction: the secret lives in a type that overwrites its
  bytes when dropped and renders as redacted, and only its digest is kept.)*
- **LA-21.** A ticket reaches the browser through a file only the account that
  owns the Library can read. The app writes an HTML page that refreshes to
  `http://127.0.0.1:<port>/#launch=<ticket>` into a directory of mode `0700`,
  as a file of mode `0600` created exclusively under a temporary name and
  renamed into place, and opens that file's `file:` URL in the system
  browser — so the browser's command line, which another account on the
  device can read, carries a path and never a ticket. The file is removed once
  its ticket is exchanged or has expired, and a file that could not be removed
  leaves a ticket that is still spent (LA-17). The page takes the ticket out of
  the fragment, removes it from the address with `history.replaceState`
  before it does anything else, and exchanges it (LA-16). A page that holds no
  token reads no Library data, and says to open the explorer from the app.
  *(Form: test for the modes, the exclusive creation and the page's handling
  of the fragment; prose for the browser's command line carrying no ticket,
  which is a property of how the app opens the browser, honored by
  construction.)*
