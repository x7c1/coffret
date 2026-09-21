---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [user-experience, completeness, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0922-0330-let-the-command-line-say-what-it-is-doing-and-what-it-found
created_at: 2026-09-21T18:30:45Z
updated_at: 2026-09-21T21:21:49Z
---

# feat(cli): let the command line say what it is doing and what it found

## Overview

Four places where the command line leaves a person guessing. Each was seen in
a real run; none of them is a failure the code detects and hides — they are
silences and sameness a person cannot tell apart from success.

Work them in the order below. The first two change what a running command
prints; the third and fourth change what it refuses.

### 1. A long transfer says nothing until it ends

A `sync` or `fetch` over hundreds of files can run for minutes with no output
at all. The consent step already knows how to speak while it waits — it prints
a line telling the person the browser has it, and where. A transfer does not.

Give the usecase layer a way to report progress to its caller, in the shape
the layer already uses for the things only a shell can do: `enter_passphrase`
and `open_url` are passed in from the shell, and this belongs beside them.
Read how those are declared and threaded before designing this one.

What a person needs from it is what is happening and how far along it is, not
a number per file: how many of how many, updated as the run goes, and which
phase it is in where the phases are distinguishable. Decide what the CLI
prints from it — one line rewritten in place is the usual shape for a terminal,
but a non-terminal stdout (a script, a pipe, CI) must not be filled with
control characters, so check `IsTerminal` and fall back to something a log can
hold.

The server drives the same usecases and does not want a terminal renderer; the
callback must be optional, or no-op for that caller, without the usecase
branching on who is calling.

### 2. Two very different runs print the same summary

`coffret fetch --under nosuch` and a fetch on a device that has mapped nothing
both print `fetched 0, containers 0, skipped 0` and exit 0. The first means
"that prefix holds nothing"; the second means "this device has nowhere to put
anything, so nothing could be fetched even if it were there". Somebody who has
just run `join` sees the second and reads it as "everything is already here".

Tell them apart in what the run prints. A device with no mapping at all is a
state the person has to leave before anything works, and the run knows it:
say so, and say what leaves it. A prefix that matched no Entry is an ordinary
empty answer and should read like one. Do not change the exit status of either
— both are true answers to what was asked, and scripts depend on them.

### 3. `freeze --target` takes a number too small to mean anything

`--target` is a size in bytes. Somebody thinking in gigabytes types
`--target 4`, and the run succeeds with one Entry per Pack, which looks like it
worked. Refuse a target below a floor that cannot be meant seriously — take
the floor from what the format makes sensible rather than inventing one, and
say the unit in the refusal and in the flag's help, since the misreading is
about the unit. `clap`'s `value_parser!` with a range is the idiom here.

### 4. `join` on S3 does not check that the Library is there

On Drive, `join` reads the folder's name and refuses a folder that is not a
Library. On S3 it looks at the shape of the prefix it was given and asks
Storage only whether the bucket exists, so a mistyped Library ID joins
successfully, and every `fetch` and `sync` afterwards reports nothing, exits 0,
and leaves the person believing an empty Library is a working one. The same
command gives a different guarantee depending on the provider.

Ask Storage once whether the prefix holds what a Library keeps at its root —
`head-1.cfrt` is the obvious candidate; check what a freshly `init`ed Library
actually has before choosing. Reading does not break the promise that `join`
writes nothing.

A Library that was `init`ed and never synced has nothing there yet, so absence
cannot be told from a typo. Do not refuse in that case: say plainly that the
Library named has nothing in it yet, so a person who typed it wrong learns
immediately, and a person who really is joining a fresh Library is told why it
looks empty. Whatever you decide the wording is, the two providers should read
as the same command by the time a person has finished the sentence.

### Alongside

- `join_library/run.rs`'s `other => other` catch-all is unreachable —
  `library_of_folder_name` returns one kind. Remove it or make the match
  exhaustive, whichever reads better.
- `init --prefix`'s help does not use the vocabulary the format register
  settled on: it should say "base prefix", which is what FM-18 and the Storage
  concept call it. `join --prefix` is the Library's own prefix and is a
  different thing; make sure the two helps do not read as the same flag.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] a fetch on a device with no mapping prints something a fetch over an
      empty prefix does not, and a test pins both
- [x] `freeze --target` refuses a value below the floor, naming the unit, and
      a test pins the refusal
- [x] an S3 `join` against a prefix that holds no Library head says so, and a
      test pins it for both the mistyped and the freshly-`init`ed case
- [x] the progress callback is exercised by a test that does not need a
      terminal

### Manual / on-hardware (verified by a human before merge)

- [ ] `make e2e-it` is green
- [ ] `make drive-round-trip-it` is green, and the progress output was read on
      a real transfer — that it appears, that it advances, and that it does not
      leave the terminal in a strange state when the run ends or is interrupted
- [ ] the same commands were run with stdout redirected to a file, and the file
      is readable

## Out of scope

- Sharing one account's grant between Libraries on the same device, so that a
  second `join` does not ask for consent again. It changes where a credential
  is cached and who it is scoped to, and it gets its own change
- Joining a Library whose app folder was renamed, which FM-18 allows and
  `join` cannot do because it reads the Library ID out of the name
- The order in which an error's sentence puts advice before its cause
- `coffret_usecase::Error`'s `detail: String`, and the `Display` that makes a
  mistyped bucket end in the port's vocabulary rather than the shell's
