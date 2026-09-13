---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "CK-12" docs/spec/checkpoint-and-prune/README.md && grep -q "UndeclaredObjectLength" backend/crates/gateway/google-drive-store/src/http/transport_error.rs && grep -q "AnswerTooLong" backend/crates/gateway/google-drive-store/src/http/transport_error.rs && grep -rq "an_object_answered_without_a_length" backend/crates/gateway/google-drive-store/ && grep -rq "holds_no_more_than_it_will_replay" backend/crates/domain/coffret-usecase/src/commit/'
assignee: null
branch: task/0913-1926-hold-a-storage-answer-to-what-it-is
created_at: 2026-09-13T19:26:32Z
updated_at: 2026-09-13T20:29:22Z
---

# fix(backend): hold a Storage answer to what it is

## Overview

Two places bound what Storage hands back by a number that is about something
else. Neither is a missing check; both apply a real ceiling to the wrong thing,
and each states the assumption that makes it look right in a comment nothing
verifies.

### 1. An object's bytes are held to a ceiling meant for JSON

`backend/crates/gateway/google-drive-store/src/answer_ceiling.rs` sets
`MAX_DOCUMENT_LEN` to 1 MiB and explains it exactly right:

> Every answer that is *not* a Storage Object's bytes is a small structured
> document — a file resource, a page of a listing, a minted-id set, an OAuth
> token, an error envelope — and reading one means holding it.

and says the exception plainly:

> A Storage Object's bytes are the exception, and they are not bounded here:
> they are as large as the files they carry, they arrive with a length Drive
> declares, and what they are held against is the port's own reckoning of what
> the caller asked for.

That exception is not implemented. `HttpRequest::new` starts every request at
`MAX_DOCUMENT_LEN`, and `.within()` raises it in exactly one place —
`google_drive.rs`, for a listing page. **`ObjectStore::get` does not raise it**:
it builds its request with `HttpRequest::new(Method::Get, &url)` and nothing
else, so an object fetch carries the 1 MiB document ceiling.

Whether that ceiling is reached depends on a second assumption, stated as fact
in `reqwest_transport.rs`:

> Drive declares a length on every answer that carries a Storage Object.
> Anything else is a document short enough to collect, and collecting it is what
> lets the port keep its promise that a stream knows how long it is.

Nothing enforces or verifies this. An answer that carries object bytes and no
`Content-Length` — chunked transfer encoding is the ordinary way that happens —
falls into the branch for documents and is drained under `answer_within`. A
Container past 1 MiB then fails. Containers are Packs of photographs; 1 MiB is
below almost all of them.

The module's own documentation says why this assumption deserves no trust: a
provider having a bad day, a proxy on the path, or something standing in for
Drive entirely is **outside the trust boundary**. The comment trusts it anyway.

Give the object path the ceiling its own doc describes — the caller's reckoning
of what it asked for — rather than the document ceiling it inherits. Where an
answer carrying object bytes declares no length, the refusal must say that,
because a caller cannot act on a limit meant for JSON.

### 2. That refusal reaches the caller as a broken connection

`http/transport_error.rs`:

```
    /// The connection broke while the body was moving.
    Body {
```

`reqwest_transport.rs` raises that same variant when an answer ran past the
ceiling:

```
        return Err(TransportError::Body {
            detail: format!(
                "an answer carrying no length ran past the {ceiling} bytes this call takes in"
            ),
        });
```

Two unrelated conditions, one variant, and a doc covering only the first. A
person whose file was refused for its size is told the connection broke — they
check their network and find nothing wrong.

Either the doc covers both conditions honestly, or they become two variants.
Judge which: they reach the same callers and mean different things about whose
fault it is and what to do next.

### 3. Catch-up holds every record it walked past

`backend/crates/domain/coffret-usecase/src/commit/catch_up.rs` walks down from
the newest head looking for a checkpoint it can adopt, keeping each decoded
object it passes:

```
    let mut fetched: BTreeMap<Generation, DecodedControlObject> = BTreeMap::new();
```

The reason it keeps them is sound and stated: the replay then comes back up from
the checkpoint over exactly those generations, so keeping them avoids fetching
each twice. What is missing is any limit on how many that is.

`CK-8` is explicit that the stretch has no bound — it describes its threshold as
**"a trigger, not a bound"** and names three ways it is exceeded: "the record
that crosses it, a Snapshot upload that fails, or a single oversized record".
`CK-9` then describes the replay without saying what holding it costs. Per
record the format allows 256 MiB (`FM-11`), and the walk's only companion limit
is `control_listing.rs`'s 100,000 pages, which bounds how many listing pages are
read and not how many decoded records are retained.

So the bound is a checkpoint-policy parameter about upload cadence, reached
through a Storage the device does not control. Bound the retention itself, and
state the rule in the register — this is the pair `FM-2`'s ceiling and its rule
already make elsewhere.

Add the rule as `CK-12`. It is a new rule rather than a sub-bullet of `CK-9`
because it is an obligation on a device catching up, not a clarification of how
catch-up chooses its starting point.

## Out of scope

- **Whether Drive ever actually answers object bytes without a length.** This
  change stops treating the answer as a document either way; measuring Drive's
  behaviour is a separate question and does not change what the code should do
  about an answer outside the trust boundary.
- **The TypeScript control path's missing per-kind ceilings** and the absent
  Rust test for `layout.rs`'s writer-side refusal. Both are real and recorded;
  both belong with the vocabulary work rather than here.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] An answer carrying a Storage Object's bytes is not held against the
      ceiling for JSON documents. A test named
      `an_object_answered_without_a_length_is_not_held_to_the_document_ceiling`
      drives an answer with object bytes and no `Content-Length` and asserts it
      is not refused for passing 1 MiB.
- [x] Where such an answer is refused, the refusal says what it was measured
      against, and a caller can tell it from a connection that broke.
- [x] `TransportError::Body`'s documentation covers every condition that raises
      it, or the conditions are separated into variants that each cover one.
- [x] Catch-up holds a bounded number of decoded control objects while it looks
      for a starting point, asserted by a test named
      `holds_no_more_than_it_will_replay`.
- [x] `CK-12` states that bound, says what a device does when a catch-up would
      pass it, and carries a `(Form: …)` annotation that matches what the tests
      actually check.
- [x] The constants and the code that enforce them cite the rules that state
      them.

### Manual / on-hardware (verified by a human before merge)

- [ ] A real `coffret fetch` against Drive for a Container larger than 1 MiB,
      confirming it succeeds. The automated tests drive the gateway's own fakes;
      this is the one check that the assumption about `Content-Length` was
      never load-bearing in the first place.
