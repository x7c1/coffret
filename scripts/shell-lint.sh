#!/usr/bin/env bash
#
# Lint every shell script in the repository with shellcheck, and hold it to one
# format with shfmt.
#
# The scripts are found rather than listed: every file git knows of — tracked,
# or new and not ignored — whose name ends in .sh or .bash, or whose first line
# is a sh or bash shebang. A script added later is covered without touching
# this file or the Makefile.
#
# Both tools are pinned, and fetched rather than taken from the PATH. A newer
# release of shellcheck adds checks, and one of shfmt formats differently, so a
# tool that came from whatever the machine or the CI runner ships would let the
# verdict on an unchanged tree change under it, and differ between a developer's
# checkout and CI. The release binaries are fetched once into .tmp/tools/
# (gitignored), each checked against the SHA-256 recorded below before it is
# run, so this needs neither sudo nor a container runtime, and CI runs this same
# script and so the same binaries. To bump a tool, change its version and every
# one of its hashes below together; the hashes are the ones GitHub lists for
# each release asset.
#
# The shfmt flags are the scripts' own style, written down once here: two-space
# indents, `case` arms indented under `case`, and a binary operator ending the
# line it continues rather than opening the next.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
cd "$ROOT"

readonly SHELLCHECK_VERSION=0.11.0
readonly SHFMT_VERSION=3.14.1
readonly SHFMT_FLAGS=(-i 2 -ci)
readonly TOOLS="$ROOT/.tmp/tools"

fail() {
  echo "shell-lint: $*" >&2
  exit 1
}

# The platform as the two projects name it in their release assets.
os="$(uname -s)"
arch="$(uname -m)"
case "$os/$arch" in
  Darwin/arm64)
    shellcheck_asset="shellcheck-v$SHELLCHECK_VERSION.darwin.aarch64.tar.gz"
    shellcheck_sha256=339b930feb1ea764467013cc1f72d09cd6b869ebf1013296ba9055ab2ffbd26f
    shfmt_asset="shfmt_v${SHFMT_VERSION}_darwin_arm64"
    shfmt_sha256=b7c872db63553ccffc7253aba3ed7d4885a27d83f1ba567b1138c6315a5847e5
    ;;
  Darwin/x86_64)
    shellcheck_asset="shellcheck-v$SHELLCHECK_VERSION.darwin.x86_64.tar.gz"
    shellcheck_sha256=c2c15e08df0e8fbc374c335b230a7ee958c313fa5714817a59aa59f1aa594f51
    shfmt_asset="shfmt_v${SHFMT_VERSION}_darwin_amd64"
    shfmt_sha256=d33eee0da0f92835b3562e9767a05cee7e4eaeef47daa03bfd09da17b4b590a6
    ;;
  Linux/x86_64)
    shellcheck_asset="shellcheck-v$SHELLCHECK_VERSION.linux.x86_64.tar.gz"
    shellcheck_sha256=b7af85e41cc99489dcc21d66c6d5f3685138f06d34651e6d34b42ec6d54fe6f6
    shfmt_asset="shfmt_v${SHFMT_VERSION}_linux_amd64"
    shfmt_sha256=76e77641faa025814b77f153b29796b8e6fa2fca03e0c76a691608b86c7ea7bf
    ;;
  Linux/aarch64 | Linux/arm64)
    shellcheck_asset="shellcheck-v$SHELLCHECK_VERSION.linux.aarch64.tar.gz"
    shellcheck_sha256=68a8133197a50beb8803f8d42f9908d1af1c5540d4bb05fdfca8c1fa47decefc
    shfmt_asset="shfmt_v${SHFMT_VERSION}_linux_arm64"
    shfmt_sha256=5f2db09dae91fca848f7adbdd014632e921a383863a2ad7e0450ad3aba0c6489
    ;;
  *) fail "no pinned shellcheck and shfmt for $os/$arch; add its release assets and hashes to $0." ;;
esac

sha256_of() {
  if command -v sha256sum >/dev/null; then
    sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null; then
    shasum -a 256 "$1" | cut -d' ' -f1
  else
    fail "found neither sha256sum nor shasum to verify the download with."
  fi
}

# Download $1 to $2 and refuse it unless its SHA-256 is $3.
fetch_verified() {
  local url="$1" out="$2" want="$3" got
  echo "shell-lint: fetching $url" >&2
  curl -fsSL --retry 3 -o "$out" "$url" || fail "could not download $url."
  got="$(sha256_of "$out")"
  if [ "$got" != "$want" ]; then
    rm -f "$out"
    fail "$url has SHA-256 $got, and $want is the one pinned; refusing to run it."
  fi
}

# Each binary is cached under a directory named by its version, and put there
# only once its download has been verified, so a present binary is a verified
# one and a bumped version fetches afresh.
shellcheck="$TOOLS/shellcheck-$SHELLCHECK_VERSION/shellcheck"
shfmt="$TOOLS/shfmt-$SHFMT_VERSION/shfmt"

if [ ! -x "$shellcheck" ]; then
  scratch="$(mktemp -d)"
  fetch_verified \
    "https://github.com/koalaman/shellcheck/releases/download/v$SHELLCHECK_VERSION/$shellcheck_asset" \
    "$scratch/$shellcheck_asset" "$shellcheck_sha256"
  tar -xzf "$scratch/$shellcheck_asset" -C "$scratch"
  mkdir -p "$(dirname "$shellcheck")"
  mv "$scratch/shellcheck-v$SHELLCHECK_VERSION/shellcheck" "$shellcheck"
  rm -rf "$scratch"
fi

if [ ! -x "$shfmt" ]; then
  mkdir -p "$(dirname "$shfmt")"
  fetch_verified \
    "https://github.com/mvdan/sh/releases/download/v$SHFMT_VERSION/$shfmt_asset" \
    "$shfmt.download" "$shfmt_sha256"
  chmod +x "$shfmt.download"
  mv "$shfmt.download" "$shfmt"
fi

scripts=()
while IFS= read -r -d '' file; do
  # A tracked file deleted in the working tree is still listed.
  [ -f "$file" ] || continue
  case "$file" in
    *.sh | *.bash)
      scripts+=("$file")
      continue
      ;;
  esac
  first=""
  IFS= read -r first <"$file" || true
  if [[ "$first" =~ ^#!.*[/[:space:]](ba)?sh([[:space:]]|$) ]]; then
    scripts+=("$file")
  fi
done < <(git ls-files -z --cached --others --exclude-standard)

[ "${#scripts[@]}" -gt 0 ] || fail "found no shell script to lint, which means the search is wrong."

echo "shell-lint: shellcheck $SHELLCHECK_VERSION and shfmt $SHFMT_VERSION over ${#scripts[@]} script(s)"

status=0
"$shellcheck" "${scripts[@]}" || status=1
if ! "$shfmt" "${SHFMT_FLAGS[@]}" -d "${scripts[@]}"; then
  echo "shell-lint: the diff above is what shfmt would change; \`$shfmt ${SHFMT_FLAGS[*]} -w <file>\` applies it." >&2
  status=1
fi
exit "$status"
