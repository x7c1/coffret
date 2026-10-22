#!/usr/bin/env bash
#
# gen-icons.sh — derive every icon of the desktop app and the explorer's
# favicon from the two sources under assets/icon/:
#
#   assets/icon/coffret.svg        the app icon
#   assets/icon/coffret-tray.svg   the tray icon: the same chest as a one-colour
#                                  silhouette, since a tray draws it small
#
# The sources are the only inputs; everything this script writes is committed.
# It needs tauri-cli, the shell and python3 (to put the .icns entries in a
# stable order). tauri-cli is not here for an app of its own: its
# `cargo tauri icon` is the SVG rasteriser, the one tool that turns an SVG into
# the PNG sizes, the .icns and the favicon rasters below in one step, without a
# graphics library installed beside it. Install it once with
# `cargo install tauri-cli --version '^2' --locked`. Where a variant is needed,
# the source is nested in a derived SVG in a temp directory and that is
# rasterised instead:
#
#   frontend/packages/apps/desktop/packaging/   the bundles' app icon:
#     icon.png                    coffret.svg as is (the .deb's)
#     icon.icns                   coffret.svg with continuous corners, inset on
#                                 a transparent canvas, see INSET_PERCENT and
#                                 write_continuous_svg (the .app's)
#   frontend/packages/apps/desktop/assets/      what the running app shows:
#     icon-dev.png                the development app's window icon (Linux):
#                                 coffret.svg with the "D" badge
#     dock-dev.png                its Dock icon (macOS): icon-dev.png's picture
#                                 with continuous corners, inset as the .icns is
#     tray{,@2x}.png              the tray, installed app, black (the macOS
#                                 template image)
#     tray-light{,@2x}.png        installed app, white (Linux)
#     tray-dev{,@2x}.png          development app, black, with a one-colour badge
#     tray-dev-light{,@2x}.png    development app, white, with a one-colour badge
#   frontend/packages/apps/web/public/
#     favicon.svg                 the chest alone, in the app icon's two colours
#                                 (lighter ones on a dark tab, see FAVICON_*)
#     favicon-32{,-dark}.png      its 32 px raster in the light and the dark colours
#     favicon-dev.svg             the same favicon with a "D" badge (see
#                                 FAVICON_BADGE_*), which a development host
#                                 serves in place of favicon.svg
#     favicon-32{,-dark}-dev.png  its 32 px raster in the light and the dark
#                                 colours, served in place of favicon-32{,-dark}.png
#
# Running it again rewrites the same files with the same bytes.
#
# Usage: scripts/gen-icons.sh     (the `make icons` entry point)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
readonly REPO_ROOT
readonly SOURCE="$REPO_ROOT/assets/icon/coffret.svg"
readonly TRAY_SOURCE="$REPO_ROOT/assets/icon/coffret-tray.svg"
readonly DESKTOP_DIR="$REPO_ROOT/frontend/packages/apps/desktop"
readonly PACKAGING_DIR="$DESKTOP_DIR/packaging"
readonly ASSETS_DIR="$DESKTOP_DIR/assets"
readonly WEB_PUBLIC_DIR="$REPO_ROOT/frontend/packages/apps/web/public"

# --- Geometry -----------------------------------------------------------------

# Both sources' canvas, and the corner radius of the app icon's background (the
# `rx` of its <rect>). The script checks the source still says so, because the
# badge is placed from it.
readonly SIZE=512
readonly SOURCE_RX=114

# macOS draws a Dock icon as a rounded square of 824 px on a 1024 px canvas,
# with transparent margins; a full-bleed icon looks larger than its neighbours.
# The `.icns` and the development app's Dock icon are therefore scaled to
# INSET_PERCENT and centred.
readonly INSET_PERCENT=80.5

# macOS rounds its icons with continuous corners, whose curvature eases in from
# the straight edge, where the source's <rect> has circular arcs that turn at
# once, and at a larger radius. The macOS icons therefore draw the background
# as a path with continuous corners, in the proportions UIKit's continuous
# rounded rectangle is commonly reproduced with: each corner starts
# CONTINUOUS_REACH radii from it, and is three cubic curves. CONTINUOUS_RX is
# the radius in source units that matches the tiles macOS 26 draws (its own
# apps' and the ones it masks): 0.2586 of the tile's side, fitted to within a
# pixel of a 1024 px rendering.
readonly CONTINUOUS_REACH=1.52866483
readonly CONTINUOUS_RX=132

# The app icon's dev badge: a filled disc centred on the centre of the
# bottom-right corner arc, ringed in the background colour to separate it from
# the chest underneath, with a bold "D" drawn as a path (no text element, so no
# font). Sizes are in source units; at 32 px one unit is 1/16 px, so the disc
# is about 13 px across and the "D"'s stroke about 2 px.
readonly BADGE_R=108
readonly BADGE_RING=16
readonly BADGE_FILL="#E5532D"
readonly BADGE_RING_COLOR="#F4F6F8"
readonly GLYPH_COLOR="#FFFFFF"
readonly GLYPH_H=136     # the "D"'s height
readonly GLYPH_STROKE=34 # its stroke width (the bowl's inner radius is GLYPH_H/2 - this)
readonly GLYPH_STEM=50   # the straight part of the "D" before the bowl starts

# The tray icon: drawn in one colour, at TRAY_PX and twice that for a 2x
# display. The source's colour, which the white variant swaps for TRAY_WHITE.
readonly TRAY_PX=32
readonly TRAY_PX_2X=64
readonly TRAY_BLACK="#000000"
readonly TRAY_WHITE="#FFFFFF"

# The tray's dev badge: a disc in the icon's colour touching the canvas's
# bottom-right corner, with the chest masked away TRAY_BADGE_GAP beyond it so a
# transparent ring separates the two, and the "D" (the glyph sizes below) cut
# out of the disc. At 32 px the disc is about 15 px across, the gap 2 px.
readonly TRAY_BADGE_R=120
readonly TRAY_BADGE_GAP=32
readonly TRAY_GLYPH_H=144
readonly TRAY_GLYPH_STROKE=38
readonly TRAY_GLYPH_STEM=52

# The favicon: the chest alone, without the app icon's background square,
# since a browser tab draws it at 16 px beside other sites' icons. Its shape is
# the tray source's (the keyhole a hole, so the tab shows through), split down
# the middle into the app icon's two colours; on a dark tab the SVG switches
# to lighter ones, where the dark left half would sink. The PNG fallback cannot
# switch, so it comes in both and index.html picks one by the colour scheme.
readonly FAVICON_LEFT="#1C2440"
readonly FAVICON_RIGHT="#5468A8"
readonly FAVICON_LEFT_DARK="#C5CEEE"
readonly FAVICON_RIGHT_DARK="#7C8FD0"
readonly FAVICON_PX=32

# The development favicon's badge: the app icon's (a BADGE_FILL disc with a
# GLYPH_COLOR "D") in the bottom-right corner like the tray's, with the chest
# masked away FAVICON_BADGE_GAP beyond it, so the tab's colour shows through
# the ring. It stops FAVICON_BADGE_INSET short of the canvas's edges: a browser
# scaling a favicon that touches an edge can bleed that edge's colour into the
# opposite one, which shows as a red line above and left of the icon. Larger than the tray's so the "D" still reads
# at 16 px, where one unit is 1/32 px: the disc is about 9 px across, the "D"'s
# stroke about 1.4 px and the gap about 1 px. Its colours do not switch on a
# dark tab.
readonly FAVICON_BADGE_R=144
readonly FAVICON_BADGE_GAP=32
readonly FAVICON_BADGE_INSET=32
readonly FAVICON_GLYPH_H=176
readonly FAVICON_GLYPH_STROKE=46
readonly FAVICON_GLYPH_STEM=60

# --- Helpers ------------------------------------------------------------------

log() { printf '[icons] %s\n' "$*"; }
die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

[ $# -eq 0 ] || die "usage: scripts/gen-icons.sh (takes no arguments)"
[ -f "$SOURCE" ] || die "source icon not found: $SOURCE"
[ -f "$TRAY_SOURCE" ] || die "source tray icon not found: $TRAY_SOURCE"
grep -q "rx=\"$SOURCE_RX\"" "$SOURCE" ||
  die "$SOURCE no longer has rx=\"$SOURCE_RX\"; update SOURCE_RX (the badge is placed from it)"
for color in "$FAVICON_LEFT" "$FAVICON_RIGHT"; do
  grep -q "fill=\"$color\"" "$SOURCE" ||
    die "$SOURCE no longer uses $color; update FAVICON_LEFT/FAVICON_RIGHT (the favicon takes the app icon's colours)"
done
grep -q "fill=\"$TRAY_BLACK\"" "$TRAY_SOURCE" ||
  die "$TRAY_SOURCE no longer fills with $TRAY_BLACK; update TRAY_BLACK (the white variant swaps it)"
cargo tauri --version >/dev/null 2>&1 ||
  die "cargo tauri not found (one-time: cargo install tauri-cli --version '^2' --locked)"
command -v python3 >/dev/null 2>&1 || die "python3 not found (it puts the .icns entries in a stable order)"

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/coffret-gen-icons.XXXXXX")"
readonly WORK_DIR
trap 'rm -rf "$WORK_DIR"' EXIT

# A path as the log shows it: relative to the repository.
shown() { printf '%s' "${1#"$REPO_ROOT"/}"; }

# An SVG without any XML declaration, so it can be nested in a derived SVG as
# an inner <svg> element.
source_body() {
  sed '/^<?xml/d' "$1"
}

# Rasterise an SVG into a directory with `cargo tauri icon`, which writes a
# fixed set of sizes (or the ones `-p` asks for) of which the callers keep only
# what they use. Its per-file progress goes to a log that is shown only when it
# fails.
rasterise() {
  local svg="$1" out="$2"
  shift 2
  mkdir -p "$out"
  if ! cargo tauri icon "$svg" -o "$out" "$@" >"$out.log" 2>&1; then
    cat "$out.log" >&2
    die "cargo tauri icon failed on $svg"
  fi
}

# Rewrite an .icns with its entries sorted by type. tauri-cli writes them in
# hash-map order, which differs between runs; sorting keeps a rerun from
# changing the committed file's bytes. The format is a header ("icns", total
# length) followed by entries of (4-byte type, 4-byte big-endian length
# including these 8 bytes, data).
sort_icns() {
  python3 - "$1" <<'PY'
import struct, sys
path = sys.argv[1]
data = open(path, "rb").read()
if data[:4] != b"icns":
    sys.exit(f"{path}: not an icns file")
entries, i = [], 8
while i < len(data):
    length = struct.unpack(">I", data[i + 4 : i + 8])[0]
    entries.append(data[i : i + length])
    i += length
body = b"".join(sorted(entries, key=lambda entry: entry[:4]))
open(path, "wb").write(b"icns" + struct.pack(">I", 8 + len(body)) + body)
PY
}

# The path data of a bold "D" centred on ($1, $2), $3 high with a stroke of $4
# and $5 of straight edge before the bowl: an outer outline and an inner
# counter, for an evenodd fill. The bowl is a half circle of radius $3/2.
glyph_d() {
  local cx="$1" cy="$2" h="$3" stroke="$4" stem="$5"
  local r=$((h / 2)) ri=$((h / 2 - stroke))
  local w=$((stem + r))
  local x0=$((cx - w / 2)) y0=$((cy - r))
  local xb=$((x0 + stem)) y1=$((cy + r))
  local xi=$((x0 + stroke)) yi0=$((cy - ri)) yi1=$((cy + ri))
  printf 'M%s %s H%s A%s %s 0 0 1 %s %s H%s Z M%s %s H%s A%s %s 0 0 1 %s %s H%s Z' \
    "$x0" "$y0" "$xb" "$r" "$r" "$xb" "$y1" "$x0" \
    "$xi" "$yi0" "$xb" "$ri" "$ri" "$xb" "$yi1" "$xi"
}

# --- Derived SVGs ---------------------------------------------------------------

# The path data of a SIZE square with continuous corners of radius
# CONTINUOUS_RX (see CONTINUOUS_REACH), clockwise from the top edge.
continuous_corners() {
  awk -v s="$SIZE" -v r="$CONTINUOUS_RX" -v k="$CONTINUOUS_REACH" 'BEGIN {
    # One corner, as offsets in radii from it along the edge it leaves (a)
    # and the edge it joins (b): three curves, each with two control points
    # and an end point, after a short line.
    n = split("1.08849323 0 0.86840689 0.02238361 0.66993427 0.06549600 " \
              "0.63149399 0.07491100 " \
              "0.37282392 0.16905899 0.16906013 0.37282401 0.07491176 0.63149399 " \
              "0.02238302 0.86840701 0 1.08849299 0 " k, c, " ")
    # The corners clockwise from the top right: where it sits, and the unit
    # steps from it towards the edge it leaves (a) and the edge it joins (b).
    split(s " 0 -1 0 0 1 " s " " s " 0 -1 -1 0 0 " s " 1 0 0 -1 0 0 0 1 1 0", q, " ")
    printf "M%.2f 0", k * r
    for (i = 0; i < 4; i++) {
      cx = q[i*6+1]; cy = q[i*6+2]; ax = q[i*6+3]; ay = q[i*6+4]; bx = q[i*6+5]; by = q[i*6+6]
      printf " L%.2f %.2f", cx + ax * k * r, cy + ay * k * r
      for (j = 1; j <= n; j += 2) {
        if (j == 1 || j == 9 || j == 21) printf " C"
        if (j == 7) printf " L"
        printf " %.2f %.2f", cx + (ax * c[j] + bx * c[j+1]) * r, cy + (ay * c[j] + by * c[j+1]) * r
      }
    }
    printf " Z"
  }'
}

# $1 with its rounded background drawn with continuous corners: the <rect> of
# radius SOURCE_RX becomes a path of the same fill, rounded at CONTINUOUS_RX.
write_continuous_svg() {
  local in="$1" out="$2"
  sed -E "s|<rect width=\"$SIZE\" height=\"$SIZE\" rx=\"$SOURCE_RX\" fill=\"([^\"]+)\"/>|<path d=\"$(continuous_corners)\" fill=\"\\1\"/>|" "$in" >"$out"
  grep -q '<rect' "$out" && die "$in: its background <rect> was not found to redraw"
  return 0
}

# $1 scaled to INSET_PERCENT and centred on a transparent canvas of the same size.
write_inset_svg() {
  local in="$1" out="$2"
  local scale offset
  scale="$(awk "BEGIN { print $INSET_PERCENT / 100 }")"
  offset="$(awk "BEGIN { print $SIZE * (100 - $INSET_PERCENT) / 200 }")"
  cat >"$out" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 $SIZE $SIZE" width="$SIZE" height="$SIZE">
  <g transform="translate($offset $offset) scale($scale)">
$(source_body "$in")
  </g>
</svg>
EOF
}

# $1 with the dev badge over its bottom-right corner.
write_badged_svg() {
  local in="$1" out="$2"
  local cx=$((SIZE - SOURCE_RX)) cy=$((SIZE - SOURCE_RX))
  cat >"$out" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 $SIZE $SIZE" width="$SIZE" height="$SIZE">
$(source_body "$in")
  <circle cx="$cx" cy="$cy" r="$BADGE_R" fill="$BADGE_FILL" stroke="$BADGE_RING_COLOR" stroke-width="$BADGE_RING"/>
  <path fill="$GLYPH_COLOR" fill-rule="evenodd" d="$(glyph_d "$cx" "$cy" "$GLYPH_H" "$GLYPH_STROKE" "$GLYPH_STEM")"/>
</svg>
EOF
}

# The tray source $1 filled with the colour $2.
write_tray_svg() {
  local in="$1" color="$2" out="$3"
  sed "s/fill=\"$TRAY_BLACK\"/fill=\"$color\"/g" "$in" >"$out"
}

# The tray source $1 filled with the colour $2, with the one-colour dev badge:
# the chest masked away around the disc, and the "D" cut out of the disc (the
# disc and the "D" as one evenodd path: inside the "D" is a hole, inside its
# counter is disc again).
write_tray_badged_svg() {
  local in="$1" color="$2" out="$3"
  local c=$((SIZE - TRAY_BADGE_R)) r="$TRAY_BADGE_R"
  local gap_r=$((TRAY_BADGE_R + TRAY_BADGE_GAP))
  cat >"$out" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 $SIZE $SIZE" width="$SIZE" height="$SIZE">
  <mask id="badge-gap" maskUnits="userSpaceOnUse" x="0" y="0" width="$SIZE" height="$SIZE">
    <rect width="$SIZE" height="$SIZE" fill="#FFFFFF"/>
    <circle cx="$c" cy="$c" r="$gap_r" fill="#000000"/>
  </mask>
  <g mask="url(#badge-gap)">
$(source_body "$in" | sed "s/fill=\"$TRAY_BLACK\"/fill=\"$color\"/g")
  </g>
  <path fill="$color" fill-rule="evenodd" d="M$((c - r)) $c A$r $r 0 1 0 $((c + r)) $c A$r $r 0 1 0 $((c - r)) $c Z $(glyph_d "$c" "$c" "$TRAY_GLYPH_H" "$TRAY_GLYPH_STROKE" "$TRAY_GLYPH_STEM")"/>
</svg>
EOF
}

# The favicon: the tray source $1 as a mask over two half-canvas rectangles
# filled $2 (left) and $3 (right). With $4 = dark-switch it also carries a
# style that turns the halves to the dark colours under a dark colour scheme
# (for favicon.svg, which the browser draws itself); with $4 = fixed the
# colours stay as given (for the PNGs, which the rasteriser draws once). With
# $5 = badge it carries the development badge (see FAVICON_BADGE_*): the chest
# masked away around the disc, and the disc with its "D" on top; with $5 =
# plain it does not.
write_favicon_svg() {
  local in="$1" left="$2" right="$3" mode="$4" badge="$5" out="$6"
  local half=$((SIZE / 2)) style=""
  local c=$((SIZE - FAVICON_BADGE_INSET - FAVICON_BADGE_R)) r="$FAVICON_BADGE_R"
  local gap_r=$((FAVICON_BADGE_R + FAVICON_BADGE_GAP))
  if [ "$mode" = dark-switch ]; then
    style="  <style>@media (prefers-color-scheme: dark) { .left { fill: $FAVICON_LEFT_DARK } .right { fill: $FAVICON_RIGHT_DARK } }</style>"
  fi
  {
    printf '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 %s %s" width="%s" height="%s">\n' "$SIZE" "$SIZE" "$SIZE" "$SIZE"
    [ -z "$style" ] || printf '%s\n' "$style"
    printf '  <mask id="chest" maskUnits="userSpaceOnUse" x="0" y="0" width="%s" height="%s">\n' "$SIZE" "$SIZE"
    source_body "$in" | sed "s/fill=\"$TRAY_BLACK\"/fill=\"#FFFFFF\"/g"
    [ "$badge" = plain ] ||
      printf '    <circle cx="%s" cy="%s" r="%s" fill="#000000"/>\n' "$c" "$c" "$gap_r"
    printf '  </mask>\n'
    printf '  <g mask="url(#chest)">\n'
    printf '    <rect class="left" width="%s" height="%s" fill="%s"/>\n' "$half" "$SIZE" "$left"
    printf '    <rect class="right" x="%s" width="%s" height="%s" fill="%s"/>\n' "$half" "$half" "$SIZE" "$right"
    printf '  </g>\n'
    if [ "$badge" = badge ]; then
      printf '  <circle cx="%s" cy="%s" r="%s" fill="%s"/>\n' "$c" "$c" "$r" "$BADGE_FILL"
      printf '  <path fill="%s" fill-rule="evenodd" d="%s"/>\n' "$GLYPH_COLOR" \
        "$(glyph_d "$c" "$c" "$FAVICON_GLYPH_H" "$FAVICON_GLYPH_STROKE" "$FAVICON_GLYPH_STEM")"
    fi
    printf '</svg>\n'
  } >"$out"
}

# --- App icon -------------------------------------------------------------------

# The bundles' app icon, into PACKAGING_DIR: icon.png from coffret.svg as is,
# icon.icns from the inset variant of it.
mkdir -p "$PACKAGING_DIR" "$ASSETS_DIR"
rasterise "$SOURCE" "$WORK_DIR/installed-full"
cp "$WORK_DIR/installed-full/icon.png" "$PACKAGING_DIR/icon.png"
log "wrote $(shown "$PACKAGING_DIR/icon.png")"
write_continuous_svg "$SOURCE" "$WORK_DIR/installed-continuous.svg"
write_inset_svg "$WORK_DIR/installed-continuous.svg" "$WORK_DIR/installed-inset.svg"
rasterise "$WORK_DIR/installed-inset.svg" "$WORK_DIR/installed-inset"
sort_icns "$WORK_DIR/installed-inset/icon.icns"
cp "$WORK_DIR/installed-inset/icon.icns" "$PACKAGING_DIR/icon.icns"
log "wrote $(shown "$PACKAGING_DIR/icon.icns") (inset to $INSET_PERCENT %)"

# The development app's window icon: the app icon with the "D" badge. Its Dock
# icon is the same picture inset, as the installed app's .icns is, since the
# Dock draws whatever it is handed across the whole tile.
write_badged_svg "$SOURCE" "$WORK_DIR/dev.svg"
rasterise "$WORK_DIR/dev.svg" "$WORK_DIR/dev"
cp "$WORK_DIR/dev/icon.png" "$ASSETS_DIR/icon-dev.png"
log "wrote $(shown "$ASSETS_DIR/icon-dev.png")"
write_continuous_svg "$SOURCE" "$WORK_DIR/continuous.svg"
write_badged_svg "$WORK_DIR/continuous.svg" "$WORK_DIR/dock-dev-full.svg"
write_inset_svg "$WORK_DIR/dock-dev-full.svg" "$WORK_DIR/dock-dev.svg"
rasterise "$WORK_DIR/dock-dev.svg" "$WORK_DIR/dock-dev"
cp "$WORK_DIR/dock-dev/icon.png" "$ASSETS_DIR/dock-dev.png"
log "wrote $(shown "$ASSETS_DIR/dock-dev.png") (inset to $INSET_PERCENT %)"

# --- Tray ---------------------------------------------------------------------

# Write one tray variant, $2 (tray, tray-dev-light, …), from the SVG $1:
# $2.png at TRAY_PX and $2@2x.png at TRAY_PX_2X.
write_tray() {
  local svg="$1" name="$2"
  local out="$WORK_DIR/$name"
  rasterise "$svg" "$out" -p "$TRAY_PX" -p "$TRAY_PX_2X"
  cp "$out/${TRAY_PX}x${TRAY_PX}.png" "$ASSETS_DIR/$name.png"
  cp "$out/${TRAY_PX_2X}x${TRAY_PX_2X}.png" "$ASSETS_DIR/$name@2x.png"
  log "wrote $(shown "$ASSETS_DIR/$name.png") and $name@2x.png"
}

# Black files carry no suffix, white ones "-light".
for suffix in "" -light; do
  case "$suffix" in
    "") color="$TRAY_BLACK" ;;
    -light) color="$TRAY_WHITE" ;;
  esac
  write_tray_svg "$TRAY_SOURCE" "$color" "$WORK_DIR/tray$suffix.svg"
  write_tray "$WORK_DIR/tray$suffix.svg" "tray$suffix"
  write_tray_badged_svg "$TRAY_SOURCE" "$color" "$WORK_DIR/tray-dev$suffix.svg"
  write_tray "$WORK_DIR/tray-dev$suffix.svg" "tray-dev$suffix"
done

# --- Favicon ------------------------------------------------------------------

mkdir -p "$WEB_PUBLIC_DIR"
# Plain files carry no suffix, the development ones "-dev" (after the colour
# scheme's, so favicon-32-dark-dev.png is favicon-32-dark.png's counterpart).
for badge in plain badge; do
  case "$badge" in
    plain) build="" ;;
    badge) build=-dev ;;
  esac
  write_favicon_svg "$TRAY_SOURCE" "$FAVICON_LEFT" "$FAVICON_RIGHT" dark-switch "$badge" "$WEB_PUBLIC_DIR/favicon$build.svg"
  log "wrote $(shown "$WEB_PUBLIC_DIR/favicon$build.svg")"
  # Light files carry no suffix, dark ones "-dark".
  for suffix in "" -dark; do
    case "$suffix" in
      "") left="$FAVICON_LEFT" right="$FAVICON_RIGHT" ;;
      -dark) left="$FAVICON_LEFT_DARK" right="$FAVICON_RIGHT_DARK" ;;
    esac
    name="favicon$suffix$build"
    write_favicon_svg "$TRAY_SOURCE" "$left" "$right" fixed "$badge" "$WORK_DIR/$name.svg"
    rasterise "$WORK_DIR/$name.svg" "$WORK_DIR/$name" -p "$FAVICON_PX"
    cp "$WORK_DIR/$name/${FAVICON_PX}x${FAVICON_PX}.png" "$WEB_PUBLIC_DIR/favicon-$FAVICON_PX$suffix$build.png"
    log "wrote $(shown "$WEB_PUBLIC_DIR/favicon-$FAVICON_PX$suffix$build.png")"
  done
done
