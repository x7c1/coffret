# Derived-origin transition

Status: adopted direction; implementation deferred. This document distinguishes
the target from the current [FM-9 schema](README.md). It changes no current
wire rule and does not authorize interpreting old bytes as a new schema.
Concept vocabulary lives in [Entry](../../concepts/container/entry/).

## Adopted target

| Meaning | Current encoding | Planned encoding |
| --- | --- | --- |
| Source name at generation time | `derived_from.original_path` | `derived_from.original_path`, still immutable provenance |
| Source identity used by the origin | `derived_from.container_id` | `derived_from.hash`, the source plaintext BLAKE3-256, 32 bytes |
| Current source after rename | No defined rename relationship | A Catalog resolution rule still to be decided |

The derived Entry's own `hash` verifies its output bytes. The hash inside
`derived_from` describes the source bytes; the two are not interchangeable.
A source path and equal hash suffice for the unchanged-name case. Repacking
unchanged source bytes must not mark the derivative stale, while different
source bytes must. A hash alone is not a globally unique file identity.

The namespace is `.coffret-derived/<kind>/<source Entry Path>`, initially
`thumb`. User input that normalizes into the reserved top-level component must
be refused explicitly. Managed Entries are excluded from user listings,
local-file candidate selection, and mapped-folder materialization. Derive
creates separate Packs and commits them with the original Packs in one batch;
ordinary one-file sync does not upload thumbnail Entries.

## Decisions still required

A renamed source cannot be looked up through immutable `original_path` alone.
Define the current-source relationship, rename of the mirrored derived path,
and behavior when the source is deleted or its old path is reused. The rule
must distinguish unrelated files with identical bytes and survive repack,
checkpoint restore, and pruning of intervening Journal records.

Also decide whether stale derived data may be displayed and when it is
regenerated. These presentation and regeneration policies do not change the
captured origin. Feature scheduling remains separate from the format design.

## Implementation acceptance

- Change Container metadata, Journal additions, Snapshot payloads, Rust and
  TypeScript codecs, interoperability fixtures, and SQLite origin columns
  together. FM-15 and FM-16 carry the same origin meaning as FM-9.
- Choose schema/version handling and test old-data behavior before changing
  bytes. The absence of a shipped derive command is not proof that no old
  origin values exist. Never reinterpret a Container ID as a content hash or
  silently discard an unreadable origin.
- Preserve validity on content-identical repack and detect source content
  updates. Test identical-content sources at different paths independently.
- Test rename, rename back, old-path reuse, deletion and recreation, and
  Snapshot-only restoration against the chosen current-source rule.
- Test namespace collisions as visible refusals and prevent managed Entries
  from becoming local-file upload candidates.
- Verify that source and derived Packs are separate, commit atomically, and
  remain absent from committed state if their batch does not commit.

These are future acceptance conditions, not tests already implemented or
passing. Metadata-only rename and derive implementation remain separate work.
