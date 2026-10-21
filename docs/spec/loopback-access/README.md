# Loopback Access

Rule prefix: `LA`. Who a server serving a Library answers, and how it knows:
the interface it listens on, the key it draws as it starts and publishes into
the Library's own directory, the fences every request passes before a route
sees it, the budgets one request carrying files in is taken within, the one
server at a time that serves a Library on a device, the drop the desktop app
takes by path and holds for one import, and the account of the
work it runs that it holds for as long as it runs. Then the **explorer host**
in front of it: the process on the same device that serves the explorer's page
and forwards that page's `/api` requests to the server with the server key
attached, the one authority it answers to, the **browser token** it admits a
browser by, and the one-time **launch ticket** a browser exchanges for that
token.

LA-1 to LA-12 are the contract of the server itself, LA-13 to LA-21 the
contract of the explorer host, LA-22 to LA-25 that of the desktop app's
**Passphrase door**: a listener of the app's own that serves the page a
Passphrase is typed into and takes what that page submits, and LA-26 onwards
that of the **import**: the files a drop onto the explorer's window in the
desktop app let go of, walked on the device by their paths, held under a
one-time **drop handle**, and carried into the Library by a request that names
the handle and never a path. The two credentials
never cross: the server key is what the host shows the server, a browser token
is what a browser shows the host, and neither of the two accepts the other's.
Nor does the door's token stand for either.

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
  the ticket file and the fragment of the address that file refreshes to, or
  in the fragment of the URL the app loads its own window from — a fragment no
  request carries, and which the page removes as it reads it (LA-21) — and in
  the `Authorization` header of its exchange (LA-16). *(Form: test)*
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
- **LA-9.** One multipart upload — the request a browser's drop carries
  files into the Library in — is taken within three
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
  they like for as long as they like; these budgets bound that. An import
  carries no file's bytes in its request, and is bounded instead as LA-28
  says. *(Form: test
  for the three budgets, which a case can drive a server within; prose for
  what they are not — that no bound on a file's size exists elsewhere is a
  claim about the rest of the system rather than about any answer this server
  gives, honored by construction and review.)*
- **LA-10.** An upload that passes one of LA-9's budgets stops where it
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
- **LA-11.** Before a part of an upload is taken, the server asks the volume
  this device's mapped folder is on whether it has room for what is still
  coming, and refuses the drop where it has not; an import asks the same of
  each file it takes (LA-28). It is asked of each part that names a
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
  The one recorded exception is a drop's choice of a Pack, which the drop
  writes on the device as a reservation before its first file lands (PK-17): a
  server takes every reservation up once its start-up catch-up has finished,
  and again once the Library is unlocked in place, in the order they were made,
  so a book a stopped server was packing is packed rather than left for a sync
  to carry in one file at a time. Which reservations a drop is still receiving
  into — live, and decided by no other run (PK-17) — is the process's own state
  like the rest, so a server started afterwards holds none live and takes every
  reservation up.
  Every answer about that work carries an identifier the process draws as it
  starts, so a browser holding what one process told it can tell that account
  from the next process's. What such work does that lasts is recorded by the
  work itself, under the rules for it — a materialization record (EP-10), a
  commit (CP-1), a pending row a run leaves for the next settle (OC-2), a
  reservation a drop leaves for its `freeze` (PK-17) — so a
  server that stops loses the account of what this device was doing and none
  of the record of what it has. A server started afterwards finishes what such
  a record proves: the catch-up it makes as it starts, where it lands,
  completes a batch whose record landed and whose refresh did not (OC-7), so
  the files that batch carried in are this device's before the server answers
  anything. *(Form: test for what a server started afterwards answers; prose
  for that account reaching no file — the catalog, the Library's directory,
  Storage — which is an absence claim over the device's files, honored by
  construction: the account is held in memory, in no serialized structure.)*
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
  process — the desktop app's, as it opens the explorer in the system browser
  or in a window of its own (LA-21) — and by no HTTP
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
- **LA-21.** A ticket reaches the explorer's page in one of two ways, and
  neither puts it anywhere another account on the device can read. Where the
  app shows the explorer in the system browser, the ticket reaches the
  browser through a file only the account that
  owns the Library can read. The app writes an HTML page that refreshes to
  `http://127.0.0.1:<port>/#launch=<ticket>` into a directory of mode `0700`,
  as a file of mode `0600` created exclusively under a temporary name and
  renamed into place, and opens that file's `file:` URL in the system
  browser — so the browser's command line, which another account on the
  device can read, carries a path and never a ticket. The file is removed once
  its ticket is exchanged or has expired, and a file that could not be removed
  leaves a ticket that is still spent (LA-17). Where the app shows the
  explorer in a window of its own, it loads that window from
  `http://127.0.0.1:<port>/#launch=<ticket>` itself: the URL goes from the
  app's process to the window's renderer and onto no command line, into no
  file and into no log, and a window whose page has lost its session is
  loaded again from a fresh ticket only when the app is asked to. Either way,
  the page takes the ticket out of
  the fragment, removes it from the address with `history.replaceState`
  before it does anything else, and exchanges it (LA-16). A page that holds no
  token reads no Library data, and says to reload the explorer from the app.
  *(Form: test for the modes, the exclusive creation and the page's handling
  of the fragment; prose for the browser's command line carrying no ticket
  and the window's URL staying within the app, which are properties of how
  the app opens the browser and loads its window, honored by
  construction.)*

### The Passphrase door

- **LA-22.** The desktop app takes a Passphrase through a door of its own: a
  listener on IPv4 loopback alone, bound before it answers, whose authority
  and origin are fixed from the address it bound as the explorer host's are
  (LA-13). It serves the Passphrase page itself, and the page submits to the
  origin it was served from, so the Passphrase goes from the window to the
  process that unlocks the Library without passing through the shell's own
  process, and no cross-origin request is made or answered: the door sends no
  `Access-Control-Allow-*` header, and an `OPTIONS` request is refused like
  any other method a route does not take. Every path is answered only where
  `Host` is the fixed authority exactly, and refused as `403` where it is
  missing, given more than once, or anything else, and every request brings
  at most 8 KiB of headers, as one to the explorer host does (LA-19),
  refused as `431`. A submission (`POST /submit`) passes, in this order:
  `Origin` given once and equal to the door's own origin, and `Sec-Fetch-Site`
  given once as `same-origin` — both required, since the one client the door
  answers is the app's own window, which sends both — refused as `403`; a
  window's token (LA-23), refused as `401`; a JSON body, refused as `415`
  where `Content-Type` does not say JSON and `400` where it is not a
  submission; and at most 4 KiB of body arriving within 5 seconds, refused as
  `413` — before a byte of it is read where it says it is longer, and where it
  passes the bound otherwise — and as `408` where it takes longer. A look at
  what the window is for (`GET /window`) passes the same `Host`,
  `Sec-Fetch-Site` and token fences, and `Origin` only where one is given,
  which a same-origin `GET` does not send. A refused open or unlock is
  answered with one sentence that names no path.
  *(Form: test)*
- **LA-23.** The door admits a window by a **door token**: one per opening of
  the window, 32 bytes from the operating system's CSPRNG written as unpadded
  base64url, the secret type the explorer host's tickets are (LA-20), held by
  the door only as its digest and compared without stopping at the first byte
  that differs. It reaches the window in the fragment of the URL the app loads
  the window from, `http://127.0.0.1:<port>/#door=<token>`, and the page takes
  it out of the address with `history.replaceState` before it does anything
  else, presenting it as `Authorization: Door <token>`. A token is bound to
  the mode the window was opened in — opening a Library, or unlocking the one
  served in place — which the door holds beside it rather than taking from
  anything the window sends. Each window's token is its own: at most 4 are
  held at once, and opening a fifth window ends the oldest token. Every token
  ends when the app retires its credentials, as it quits and when the server
  stops (LA-18). A missing, unknown, malformed or ended token is one answer,
  `401`, which names none of these and says the window has to be opened
  again. *(Form: test for the token's encoding, its independence and its
  ending; prose for the comparison's timing and the digest being all that is
  held, for the reasons LA-20 gives.)*
- **LA-24.** A token ends when its window's submission opens or unlocks the
  Library, and after 5 refused submissions. Every submission refused once its
  token was admitted counts — a body over its bound or not a submission, a
  Passphrase that does not open the Library, another Library already open —
  and none refused before its token is looked at does. The answer to a
  refusal says whether the token has ended with it; once it has, the window
  says it has to be opened again from the app. Submissions are taken one at a
  time across every window: of two at once, the second is taken once the
  first has its answer, and sees what the first did — the Library the first
  opened is answered as open, and another is refused. *(Form: test)*
- **LA-25.** No event the door records carries a door token, its digest, the
  door's URL or its address (EL-1): a window's token being issued, a
  submission refused, and a token ending are recorded by their kind, a fixed
  reason and the counts they left. Nor does anything the app tells its shell
  carry one: the shell is told what happened — a Library opened or unlocked,
  an unlock asked for, the server stopped — and asks for a URL or a ticket
  through a call of its own. *(Form: test)*

### The import

- **LA-26.** The desktop app takes a drop onto the explorer's window by the
  paths of what was let go of, and holds the regular files it took under a
  **drop handle**: 32 bytes from the operating system's CSPRNG, written as
  lowercase hex. A drop is held for 10 minutes; at most 4 are held at once,
  and holding a fifth ends the oldest. A handle is used once: the import that
  names it takes the drop, whatever that request then comes to, and the drop
  is held no longer. An import refused before its handle is read — the
  Library locked (DK-2), or a body this route does not read (LA-27) — takes
  nothing, and the drop stays held. An import naming a handle that was never
  drawn, was used, was ended by a later drop or has expired is refused as a
  whole as `400`, before anything is read, with one sentence that names none
  of these.
  What the page is told of a drop is the handle and, per file taken, its path
  relative to the folder the dropped item stood in, its size and its
  modification time, beside what was not taken and why — never a path on the
  device. Only the explorer's own window may hand a drop over, from the
  explorer's origin, as a list of the items' paths. No event records a
  handle, a path on the device or a dropped file's name (EL-1): walking,
  holding and taking a drop, and importing it, are recorded by counts and
  refusal kinds. *(Form: test)*
- **LA-27.** An import (`POST /api/import`) is a JSON body naming the folder of
  the Library, whether the drop goes in as a Pack (PK-17), the handle, and per
  file its index in the held set and the name it is to have relative to the
  folder. No path on the device is accepted over HTTP: a body carrying any
  other field is refused as a whole as `400`, a name is an Entry Path relative
  to the folder or that file is refused as one (EP-2), and an index outside the
  held set refuses the request as a whole as `400` before anything lands. The
  request passes the same fences every route does (LA-2, LA-5), and is answered
  in the upload's shape: what was written, and each file refused with its
  refusal. *(Form: test)*
- **LA-28.** An import is bounded by three things and not by LA-9's budgets,
  which are about what one socket carries: the room the volume a file lands on
  has for what is still coming, asked of each file before it is taken as LA-11
  asks it of an upload's part, what is still coming being the lengths the walk
  stated less what has been taken; at most as many files per request as LA-9
  allows an upload parts, refused as LA-10 refuses an upload past it; and the
  held set, which no request adds to. A file is held to the length the walk
  stated: one that grew since it was dropped is refused part way through, as a
  `conflict`, and leaves neither a file under its final name nor its scratch
  (EP-11). *(Form: test)*
- **LA-29.** The walk of a drop follows no symbolic link and takes none: a link
  to a file and a link to a folder are each reported as neither, as is anything
  else that is not a regular file or a folder, and nothing under a linked
  folder is walked (EP-8). A dropped folder is walked depth first, its children
  in the byte order of their names; an empty folder contributes nothing. A name
  coffret keeps for itself inside a mapped folder — the scratch prefix and the
  management area, in the one rule a scan and an upload read them by (EP-11,
  EP-14) — is reported as refused and not walked. An empty path, a path that is
  not there, a name that is not text (EP-1), and one the operating system will
  not read are reported, not left out. The import reads each file again from
  the folder its dropped item stood in, descending without following a link, so
  a file replaced by a link since it was dropped is refused rather than
  followed. *(Form: test)*
