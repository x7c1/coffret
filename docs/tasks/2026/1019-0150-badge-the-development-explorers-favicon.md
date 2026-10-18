---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && A=$(find backend/crates/apps/coffret-desktop/icons backend/crates/apps/coffret-desktop/icons-dev backend/crates/apps/coffret-desktop/icons-tray frontend/packages/apps/web/public -type f | sort | xargs sha256sum) && make icons && B=$(find backend/crates/apps/coffret-desktop/icons backend/crates/apps/coffret-desktop/icons-dev backend/crates/apps/coffret-desktop/icons-tray frontend/packages/apps/web/public -type f | sort | xargs sha256sum) && test \"$A\" = \"$B\" && test -f frontend/packages/apps/web/public/favicon-dev.svg && test -f frontend/packages/apps/web/public/favicon-32-dev.png && test -f frontend/packages/apps/web/public/favicon-32-dark-dev.png && grep -q 'prefers-color-scheme: dark' frontend/packages/apps/web/public/favicon-dev.svg && grep -q '#E5532D' frontend/packages/apps/web/public/favicon-dev.svg && grep -rq 'favicon-dev' backend/crates/apps/coffret-explorer-host/src/ && ! grep -rq 'favicon-dev' frontend/packages/apps/web/index.html"
assignee: null
branch: task/1019-0150-badge-the-development-explorers-favicon
created_at: 2026-10-18T16:50:07Z
updated_at: 2026-10-18T17:08:48Z
---

# feat(explorer): badge the development explorer's favicon

## Overview

The installed desktop app, the development app (`make desktop-dev`) and the
development host (`make dev`) all serve the same explorer build, so a browser
tab shows the same favicon and the same title ("coffret") whichever served
it. With the installed and a development explorer open side by side there is
no telling the tabs apart. The tray already solves this for the desktop apps
with a "D" badge on the development app's icon; this task carries the same
badge to the explorer's favicon.

The page cannot tell which host served it, so the host decides: a host told
it is a development one answers the favicon paths with the badged files. The
tab title stays "coffret" in every build (rewriting `index.html` per host is
left out on purpose).

### The badged favicon

`scripts/gen-icons.sh` already writes the favicon (`write_favicon_svg`): the
tray source's silhouette as a mask over the app icon's two colours, switching
to lighter ones under `prefers-color-scheme: dark` in the SVG, plus a light
and a dark 32 px PNG. Add the development variant next to it, in
`frontend/packages/apps/web/public/`:

- `favicon-dev.svg` — the same favicon with the dark switch, and a "D" badge
  in the bottom-right corner as on the app icon's development variant: a disc
  filled `BADGE_FILL` (`#E5532D`) with a white "D" drawn as a path
  (`glyph_d`), separated from the chest by a transparent ring (mask the chest
  around the disc, as `write_tray_badged_svg` does) rather than a ring in a
  background colour, since the favicon has no background and the tab's colour
  shows through. The badge keeps its colours under the dark scheme.
- `favicon-32-dev.png` and `favicon-32-dark-dev.png` — the light and the dark
  colours with the badge, fixed, at `FAVICON_PX`.

Size the badge so the "D" stays legible at 16 px (a tab's size); the tray's
badge numbers (`TRAY_BADGE_*`, `TRAY_GLYPH_*`) are the starting point. Keep
the new numbers as named variables at the top of the script, describe the new
files in its header comment, and keep the script's rerun-is-byte-identical
property. `index.html` is not changed: it keeps linking `/favicon.svg`,
`/favicon-32.png` and `/favicon-32-dark.png`.

### The host

`coffret-explorer-host` gets to know whether it is a development host, as a
field of `Config` (for example a `development: bool`, or a small enum —
whichever reads better beside `PageSource`). When it is, `page::serve`
answers `/favicon.svg`, `/favicon-32.png` and `/favicon-32-dark.png` with the
contents of `favicon-dev.svg`, `favicon-32-dev.png` and
`favicon-32-dark-dev.png` from the same build (embedded or on disk), with the
content type of the name asked for and the same cache header as any
non-`assets/` file (`no-cache`). Name the mapping in one place. Everything
else, the `-dev` files asked for by their own names included, is served as
today. A production host serves exactly what it serves today.

Who says which:

- **Development host (`coffret-explorer-dev`, `make dev`)**: always a
  development host.
- **Desktop app (`coffret-desktop`)**: a development host when the app is the
  development app. `src/tray.rs` already tells the two apart by the identifier
  (`DEV_IDENTIFIER`, `Build::of`); move that out of the tray into a module of
  its own so the tray and the code that builds the host's `Config`
  (`src/unlock/open_library.rs`, through `Config::for_library` — give it the
  flag as a parameter) read the same thing. Keep the identifier named in one
  place.

### Tests

- `coffret-explorer-host`: a development host answers each of the three
  favicon paths with the `-dev` file's bytes and the asked-for name's content
  type; a production host answers them with the plain files. Cover a build on
  disk (`PageSource::Directory`, with small fixture files), following the
  existing page tests.
- `coffret-desktop`: the existing `Build::of` tests move with it.

### Docs

`docs/guides/environments.md` describes the favicon (the paragraph on the
icons under the desktop app). Add that a development host — `make dev`'s, or
the development app's — serves the badged favicon so its tabs are told apart
from the installed app's, and that the title stays the same.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make icons` writes `favicon-dev.svg` (with the dark switch and the
      `#E5532D` badge), `favicon-32-dev.png` and `favicon-32-dark-dev.png`,
      and rerunning it leaves every icon output unchanged; `index.html` does
      not link the `-dev` files; the host names them (gates in
      `check_command`).
- [x] The host tests show a development host answering the three favicon
      paths with the `-dev` files and a production host with the plain ones.
- [x] `make check` passes.

### Before merge (verified outside the check command)

- [ ] With the installed app and `make desktop-dev` both running, the
      development explorer's tab shows the badged favicon and the installed
      one's the plain favicon, in Chrome's light and dark modes (a person
      looks at the tabs).
- [ ] The badge's "D" is legible at a tab's size (a person judges).
