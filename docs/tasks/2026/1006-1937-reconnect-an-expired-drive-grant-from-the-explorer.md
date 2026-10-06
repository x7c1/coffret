---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "/api/reconnect" backend/crates/apps/coffret-server/src/router.rs && grep -rq "reconnect" frontend/packages/gateway/api/src && grep -rq "reconnect" frontend/packages/apps/web/src/StatusBar.tsx && grep -rqi "unauthenticated" frontend/packages/gateway/api/src/refusal.ts'
assignee: null
branch: task/1006-1937-reconnect-an-expired-drive-grant-from-the-explorer
created_at: 2026-10-06T19:37:49Z
updated_at: 2026-10-06T20:21:51Z
---

# feat: reconnect an expired Drive grant from the explorer

## Overview

A Drive grant issued by a consent screen in testing status runs out after
seven days. When it does, the server's every reach for Storage fails with
`Unauthenticated` ("the grant has run out"), the explorer says only that
Storage did not answer, and the way back is the command line: stop the server,
`coffret authorize --library <name>`, which asks for the Passphrase, opens the
consent page in a browser and receives the redirect on a loopback listener
(spec: SA-1, SA-2), then start the server again. That is three steps in a
terminal for something that happens every week. This change lets the person do
it from the explorer: a refusal that names the expired grant carries a
*reconnect* button, the running server runs the same consent flow with the keys
it already holds, the page opens the consent URL in a new tab, and when the
redirect lands the server goes on with the renewed grant. No Passphrase is
asked for — the server is unlocked, so the account's key is already in its
memory — and no secret passes through the page: the page sees a URL to open
and a state to poll, nothing else.

**1. The server can tell an expired grant from Storage being unreachable.**
Read `backend/crates/apps/coffret-server/src/api_error/` (`contract.rs`,
`from_error.rs`) and `reported.rs`: a Storage failure travels to the page as
kind `storage` with a message. Make the expired grant distinguishable — a
`reason` of `unauthenticated` on that refusal, in the shape `Reported` and the
page's `refusal.ts` already have for reasons, or whatever the contract already
offers if it does — for the refusals the listing, the reader and the work
answer carry, and in the standing the `/api/work` answer reports for the
catalog (`routes/work/contract.rs` has the fixture `storage()` built from that
very error; `refresh/` and `Standing` say how a catch-up that failed is
reported). Tests pin the reason on each surface.

**2. `POST /api/reconnect` runs the consent flow inside the server.**
A keyed route (spec: LA-2). When the Library is locked it answers the usual
locked refusal. Otherwise it starts the authorization-code flow for the
account the Library references, using the keys the unlocked `OpenLibrary`
holds — `coffret-device`'s `authorize` takes a Passphrase closure and opens the
Library itself (`authorize/mod.rs`, `library_account`); add beside it an entry
that takes the already-open Library (or the `Opened` account it yields) and the
client credentials, so the server never asks for a Passphrase it does not have
— and answers `202` with the consent URL the flow produced (the `open_url`
callback's argument), plus a sentence. The flow itself runs on a task the
server owns and waits for the redirect on its loopback listener (spec: SA-2);
starting a second flow while one is waiting answers the same URL again rather
than a second listener. When the redirect lands, the grant is verified and
cached as `authorize` does today (spec: SA-4, SA-6, and whatever the `SA`
rules say about one grant per account on a device — read `docs/spec/` and cite
them). The outcome — renewed, refused by the person, timed out — is reported in
the `/api/work` answer so the page can show it.

**3. The running server uses the renewed grant.** Today `authorize`'s own
message says a renewed grant is used "from its next run": the server's Drive
store holds the credential it loaded when the Library was opened. Make the
running server adopt the renewed grant without a restart — the store re-reads
the account's cache when the flow completes, or the server swaps the Storage
gateway it holds; read `google-drive-store`'s token handling and
`coffret-device`'s account cache (`accounts.rs`, the `.cftc` file under
`accounts/<name>/`) and choose the smaller change — and then retries the
catch-up (`refresh_catalog`) so the listing is current. A routes test with the
in-memory Storage stand-in that can be told to refuse as unauthenticated and
then accept covers: refusal reported with the reason, reconnect started, flow
completed (drive the flow's completion directly in the test rather than through
a browser), catalog caught up again.

**4. The explorer offers the reconnect where the refusal is.** Wherever the
page shows a Storage refusal whose reason is `unauthenticated` — the status
bar's refusal line, the catch-up notice ("this device has not caught up … the
Library's Storage did not answer"), the reader's refusal — say what happened
("Google Drive's permission for this device ran out; it is renewed every seven
days") and show one button, *reconnect*. It calls `POST /api/reconnect`, opens
the URL it answers with in a new tab (`window.open`), and shows the sentence.
The page then keeps polling `/api/work` as it does; when the answer reports the
grant renewed, the notice says so and the listing is reloaded; when it reports
the person refused or the flow timed out, the button is offered again with that
reason. Unit tests for the refusal rendering and the state transitions in the
style of `StatusBar.test.tsx` and `refresh.test.ts`.

**5. The CLI keeps working as it is**, and its "from its next run" sentence is
left true for the CLI's one-shot case. The server's own startup refusal for a
grant that has already run out when the Library is opened (the `startup`
refusal in the log) keeps its words; the explorer's first screen on such a
server shows the same refusal with the same button, which is covered by 4.

Spec: no rule changes. SA-1 to SA-7 describe the flow this reuses; the rules on
accounts and their grants say which account a Library's grant belongs to. If a
rule forbids renewing a grant from a running server, or requires the Passphrase
for it, stop and report `needs_review` rather than editing the spec.

Out of scope: in-place unlock (waits on a spec decision), mapping folders from
the explorer, the shell's tray (nothing changes there — the flow runs in the
server, so it works under `make dev` and the desktop app alike).

## Acceptance criteria

### Automated (pipeline-verified)
- [x] A Storage refusal caused by an expired grant carries a reason the page can branch on, on the listing, reader and work-answer surfaces, with tests
- [x] `POST /api/reconnect` is a keyed route that starts the consent flow with the unlocked Library's keys and answers `202` with the consent URL; a second call while a flow waits answers the same URL; a routes test drives the flow to completion with the in-memory Storage stand-in and sees the catalog caught up afterwards
- [x] The running server uses the renewed grant without a restart (the routes test above proves it: refused before, served after)
- [x] The explorer shows the expired-grant sentence and a *reconnect* button where the refusal is, opens the URL in a new tab, and reflects the outcome from `/api/work`, with unit tests
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Needs a person: with the development Library's grant expired (or revoked in the Google account's permissions page), the explorer shows the sentence and *reconnect*, the consent tab opens, and after consenting the listing comes back without restarting the server; also under `make desktop-dev`
