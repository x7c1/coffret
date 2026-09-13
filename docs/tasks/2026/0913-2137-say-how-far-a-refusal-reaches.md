---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, concept-alignment, error-type-design, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -qE "LA-9|LA-10|LA-11" docs/spec/entry-path/README.md && grep -rq "a_root_the_system_would_not_answer_about_stops_the_drop" backend/crates/'
assignee: null
branch: task/0913-2137-say-how-far-a-refusal-reaches
created_at: 2026-09-13T21:37:15Z
updated_at: 2026-09-13T22:43:40Z
---

# docs(spec): say how far a refusal reaches, and where a foreign cause stops

## Overview

Three findings about what the register says a refusal does, and one of them has
a matching defect in the code.

### 1. `EP-11` claims a reach it does not govern

`docs/spec/entry-path/README.md`, a sub-bullet of `EP-11`:

> A single writer handed several placements at once — the upload route's
> multipart drop — declines each placement that is one file's business and
> reports it beside what it placed, and fails the request as a whole only
> where the refusal is a mapping's business rather than a file's (EP-13).

That **only** is false. `backend/crates/apps/coffret-server/src/routes/upload/refusal.rs`
names four conditions that stop the whole drop, and a refused root is one of
them:

> A budget this request has outrun, room this device has not for what is still
> coming or could not be asked about at all, a stream that broke before the next
> part, the state of a mapped root the parts are going through — one condition
> gathers them, and it is what the next of them is judged by.

The code is the honest one here. What is wrong is that `EP-11` is a rule about
**placing an Entry at a local path**, and it wrote a sentence about every
refusal a multipart drop can meet. Two of the three it does not govern now have
rules of their own, added since: `LA-10` says a request that passes one of
`LA-9`'s budgets stops where it stands and that it is the request that is
refused and not a file, and `LA-11` says a volume that cannot be asked about at
all refuses the request.

Narrow `EP-11`'s sentence to the refusals it is actually about — those about a
placement — and say where the others are stated, so a reader who arrives at
`EP-11` asking "what stops a drop" is sent on rather than told something false.
Do not weaken what it says about the placement refusals themselves: the split
between one file's business and a mapping's, and the rule that the condition
decides the side rather than the wire kind, are both correct and are what
`refusal.rs` is built on.

### 2. A root the system would not answer about is reported as one file's business

`backend/crates/gateway/coffret-local-fs/src/unix_destinations/vouch.rs` vouches
for a mapped root by reading its marker, and four of its arms hand back an I/O
failure rather than a verdict (`:61`, `:86`, `:100`, `:120`, all through
`refused_by_the_system` at `:152`). Its doc gives the reason, and the reason is
right:

> An I/O refusal rather than a verdict about the root's identity: a permission
> the process does not have says nothing about which folder this is, and reading
> it as a mismatch would send a person to record the mapping again over
> something that is not about the mapping at all.

So it must not become a `RootRefused` variant — that is the misdiagnosis the doc
exists to prevent. The defect is not the type; it is the **reach**. It arrives at
the upload route as an `Error` that is not `Error::RootRefused`, and
`From<Error> for Refusal` sends everything else to `Refusal::Part`:

```
refused @ Error::RootRefused(_) => Self::Request(refused.into()),
other => Self::Part(other.into()),
```

A permission failure on `<root>/.coffret` is a fact about the root every part of
the drop is going through. Reported as one part's business, the drop reads the
next part, meets the same refusal, and reports it again — once per file, for a
condition that was settled before the first one was read.

`Refusal`'s own doc already anticipates this: it says the conversion from
`ApiError` "can never make a refusal about the request" because a wire kind does
not carry reach, and that the `From<Error>` arm is "the one exception, and it is
one because it has a failure kind rather than a wire kind to read". That arm
reads only one kind today.

Make a failure met while vouching a root reach the request, without turning it
into a verdict about the root's identity. Whether that is a new `Error` kind, a
wrapper that carries reach beside the I/O failure, or something else is the
implementation's judgement — what the task fixes is that a fact about the shared
root stops the drop once.

Name the test `a_root_the_system_would_not_answer_about_stops_the_drop`.

### 3. `EL-2`'s grammar has no slot for where a foreign cause stops

`docs/spec/event-logging/README.md`:

> **EL-2.** `Redacted` renders one typed error link as `Vocabulary::Variant`,
> with permitted facts appended as comma-separated `key=value` entries in
> parentheses. A permitted typed cause follows `: ` and applies the same grammar
> recursively. The rendering describes identity and evidence rather than
> borrowing a person-facing sentence.

`EL-4` permits something `EL-2` does not describe:

> an unknown foreign cause stops at a fixed identity or an explicit safe summary
> such as error kind

and the code has two shapes `EL-2`'s grammar does not admit, both deliberate and
both documented. `backend/crates/domain/coffret-usecase/src/error.rs:371` renders
`Self::Io { cause }` as `Io(kind={:?})` — a summary rather than a
`Vocabulary::Variant` — and the same impl renders the Storage vocabulary's other
variants as their own messages, which
`backend/crates/domain/coffret-model/src/redacted.rs` calls out by name:

> The second link above is a message rather than an identity, and it is the one
> deliberate exception: what a provider answered is useful diagnostic evidence,
> so the Storage port's vocabulary is rendered as it reads.

A module doc is where that exception is written down, and the register is where
it belongs: `EL-2` is the rule a later vocabulary is read against, and a rule
that describes only one of the three shapes in the tree tells its next reader
that the other two are defects.

Give `EL-2` the slots the code already uses — the summary a foreign cause stops
at, and the message form — and say what keeps each safe, since neither is an
identity a reader can group by. **Two vocabularies use the message form, not
one**: `coffret-usecase`'s Storage arm renders `other.to_string()`, and
`coffret-format`'s renders `Format: {other}`
(`coffret-format/src/error.rs:1049-1052`), and they are safe on different
grounds — the first on the credential redaction `EL-5` obliges, the second on
its sentences being composed about the shape of the bytes it read rather than
lifted out of a payload, which that impl's own doc states at length. Cite the
rule each stands on rather than restating it, and say what admits a later
vocabulary to the shape, so the rule does not read as a closed list of two.

**This one has no gate.** The behaviour is already pinned —
`coffret-usecase/src/error.rs:552` and `google-drive-store/src/error.rs:626`,
`:679` assert the `Io(kind=…)` rendering — so a new test would pin nothing new,
and a `grep` for a phrase would forbid a correct wording. It is no less required
for having no gate.

## Out of scope

- **A rule for the broken multipart stream.** It is the third of `refusal.rs`'s
  four conditions and the one with no register home, but it is a fact about a
  transport rather than about the Library, and giving it a rule is a judgement
  about what the register covers rather than a correction to what it says.
  `EP-11` stops claiming it either way.
- **`EP-11`'s scratch reservation and `OC-8`'s enumeration.** Both were recorded
  as gaps and both are **already closed** in the current register — `EP-11`'s
  scratch sub-bullet reads "Every local writer publishing by rename into one
  takes its scratch names from that prefix", and `OC-8` already lists "the
  staging directory an interrupted attempt at putting a Library on this device
  left". Nothing to do.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `EP-11` no longer claims that a placement refusal's reach is the only
      thing that stops a multipart drop, and points at the rules that state the
      others.
- [x] What `EP-11` says about placement refusals is unchanged in substance: a
      placement that is one file's business is declined and reported beside what
      was placed, one that is a mapping's business fails the request, and the
      condition rather than the wire kind decides which.
- [x] A failure the operating system gives while vouching a mapped root stops
      the whole drop instead of being reported once per part, asserted by a test
      named `a_root_the_system_would_not_answer_about_stops_the_drop`.
- [x] That failure is still not a verdict about the root's identity: nothing
      sends a person to record the mapping again over a permission failure.
- [x] `EL-2` describes every shape `Redacted` renders in this workspace,
      including where a foreign cause stops and the message form both the
      Storage and the format vocabularies use, and cites the rule each stands
      on rather than restating it.
- [x] The register and the code agree on every sentence this change touches:
      each `(spec: …)` citation near an edited site still names a rule that says
      what the citing comment says it says.

### Manual / on-hardware (verified by a human before merge)

- [ ] Nothing here is observable on hardware beyond a drop onto a root the
      process cannot read, which the automated test drives against the gateway's
      own fakes. No manual check is needed.
