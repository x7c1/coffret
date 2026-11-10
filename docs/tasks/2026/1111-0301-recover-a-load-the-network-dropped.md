---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1111-0301-recover-a-load-the-network-dropped
created_at: 2026-11-10T08:31:28Z
updated_at: 2026-11-10T20:36:41Z
---

# fix: let the explorer recover from a load the network dropped, and keep the e2e Storage restart off the host's interfaces

## Overview

The e2e journey `lose Storage mid-book, reload, and pack the book again`
(`frontend/packages/apps/e2e/journeys/07-freeze.spec.ts`) failed once in CI
(run 42780980051, PR #433, head 7a6df21) at line 188,
`expect(inTree(page, STRANDED)).toBeVisible()`. The run's `e2e-diagnostics`
artifact names the cause, and it is neither the Index nor the server:

- After the reload, the page had no tree at all. Its main region said
  "the coffret server did not answer" with a "try again" button; the Activity
  panel still said the book was packed.
- The trace's network log shows the reload's four `/api/list` requests
  (root, `runs`, `runs/album-000`, `runs/album-000/stopped-in-the-browser`)
  failing with `net::ERR_NETWORK_CHANGED`, with no response. The other
  `/api/*` requests of the same load got 200. The console logged the same
  four failures.
- The reload came about 1–1.7 s after `startStorage` ran `docker start` on the
  MinIO container. That container runs on Docker's default bridge network
  with a published port (`minio_run` in `scripts/lib/minio.sh`), so starting it
  adds a veth interface to the host, whose IPv6 link-local address arrives a
  second or two later. Chromium sees the host's interfaces change and aborts
  the requests in flight. A reload that lands in that window loses its
  listings; one that does not, passes — which is why 85 local iterations
  never failed.
- For the 30 s after the failed load, the explorer made no request at all: it
  stayed on the "did not answer" screen until someone pressed "try again".

So there are two fixes, and this task makes both.

1. **The explorer recovers on its own from a request the network dropped.**
   When a listing the explorer loads fails without an answer from the server
   (the `fetch` rejects; there is no HTTP status), it does not sit on the
   "did not answer" screen waiting for a press of "try again". It tries again
   by itself, a bounded number of times with a short backoff, and draws the
   tree once the server answers; only when those attempts are spent does it
   show the sentence and the button as today. An HTTP answer that refuses
   (4xx, 5xx, a locked Library) is not retried this way — only a request that
   never got an answer. The same thing happens to a person whose Wi-Fi or VPN
   switches while the explorer loads. The places that render the sentence
   and the button are around `TRY_AGAIN` in
   `frontend/packages/apps/web/src/Region.tsx` and `TreeTable.tsx`; find the
   load that feeds them and put the retry there, not in each view. If a spec
   rule describes the load failure, write the new behaviour into it.
2. **The journeys' Storage outage does not change the host's network
   interfaces.** Stopping and starting MinIO in `journeys/storage.ts` must not
   add or remove an interface the browser can see — for example by running
   the e2e MinIO container with `--network host` and MinIO listening on
   `127.0.0.1:<port>`, instead of a bridge network with a published port.
   Keep `make e2e-it` working where it works today, and keep `make local` and
   the other callers of `scripts/lib/minio.sh` as they are unless the change
   is needed there too. Say in a comment why the container is set up this
   way.

Do not raise a timeout, add a retry to the journey, or re-run it to make it
pass. Do not change `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test makes the explorer's first listing reject as a network
      failure once (then answer), and the tree is drawn without "try again"
      being pressed; it fails on the code before the fix.
- [x] A unit test makes the listing answer with an HTTP error, and the
      explorer shows "the coffret server did not answer" / its refusal and
      "try again" as before, without retrying on its own.
- [x] A unit test makes every attempt fail as a network failure, and the
      explorer ends on the sentence and "try again" after its bounded
      attempts.
- [x] The e2e MinIO container is started so that `stopStorage` /
      `startStorage` add or remove no host network interface, with the reason
      in a comment.
- [x] `make check` and `make e2e-it` pass, the journey
      `lose Storage mid-book, reload, and pack the book again` included, with
      no timeout raised and no journey skipped.
- [x] The PR body names the cause (the `net::ERR_NETWORK_CHANGED` from the
      veth the restarted container added) and links CI run 42780980051.
