# Loopback Access

Rule prefix: `LA`. Who a server serving a Library answers, and how it knows:
the interface it listens on, the key it draws as it starts and publishes into
the Library's own directory, the fences every request passes before a route
sees it, the budgets one request carrying files in is taken within, and the
one server at a time that serves a Library on a device.

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
- **LA-9.** One request carrying files into the Library is taken within three
  budgets of this server's own: at most 64 GiB across the whole request,
  framing included; at most 1 GiB of file content in any one part; and at most
  4096 parts, counting every part the request carries and not only the ones
  that name a file. What they bound is one HTTP request arriving on this
  device's loopback socket, and nothing beyond it. They put no number on how
  large a file may be, and no rule elsewhere puts one there: an Entry larger
  than a Pack's size target is a Pack of its own rather than a file refused
  (PK-3). What the format bounds instead is how much a Container's entry table
  or a Library's checkpoint comes to (FM-2, FM-11), and never the size of any
  one Entry. What this server has that the Library has not is a socket, on
  which somebody who is not the explorer can send whatever they like for as
  long as they like; these budgets bound that. *(Form: test for the three
  budgets, which a case can drive a server within; prose for what they are
  not — that no bound on a file's size exists elsewhere is a claim about the
  rest of the system rather than about any answer this server gives, honored
  by construction and review.)*
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
