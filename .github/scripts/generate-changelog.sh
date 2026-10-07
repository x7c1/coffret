#!/usr/bin/env bash
# Shared changelog generation using conventional commit categorization.
#
# Reads commits between $last_tag (exclusive) and HEAD (inclusive) and
# groups them by their conventional-commit type prefix
# (`type:` or `type(scope):`). Unknown types fall through to "Other".
#
# Usage:
#   source generate-changelog.sh
#   changelog=$(generate_changelog "$last_tag" "$repo_url")
#
# With an empty `last_tag` (no release yet), the last 20 non-merge commits
# reachable from HEAD are summarized.

generate_changelog() {
  local last_tag="$1"
  local repo_url="$2"
  local log

  if [ -n "$last_tag" ]; then
    log=$(git log "${last_tag}..HEAD" --oneline --no-merges)
  else
    log=$(git log --oneline --no-merges -20)
  fi

  format_changelog "$log" "$repo_url"
}

format_changelog() {
  local log="$1"
  local repo_url="$2"
  local -A sections
  local -a section_order=(feat fix perf refactor style docs chore other)
  local -A section_titles=(
    [feat]="Features"
    [fix]="Bug Fixes"
    [perf]="Performance"
    [refactor]="Refactoring"
    [style]="Style"
    [docs]="Documentation"
    [chore]="Chores"
    [other]="Other Changes"
  )

  # Initialize empty sections so iteration order stays stable.
  for key in "${section_order[@]}"; do
    sections[$key]=""
  done

  # Categorize each commit by its conventional-commit type prefix.
  while IFS= read -r line; do
    [ -z "$line" ] && continue

    # Drop the commit hash prefix that `git log --oneline` emits.
    local message="${line#* }"
    # Convert in-message PR references (#123) into Markdown links.
    # shellcheck disable=SC2001 # a capture group, which ${var//} cannot express
    message=$(sed "s|#\([0-9]\+\)|[#\1](${repo_url}/pull/\1)|g" <<<"$message")

    # Extract the type from `type:` or `type(scope):`.
    local type=""
    if [[ "$message" =~ ^([a-z]+)\(.*\): ]]; then
      type="${BASH_REMATCH[1]}"
    elif [[ "$message" =~ ^([a-z]+): ]]; then
      type="${BASH_REMATCH[1]}"
    fi

    case "$type" in
      feat | fix | perf | refactor | style | docs | chore)
        sections[$type]+="- ${message}"$'\n'
        ;;
      *)
        sections[other]+="- ${message}"$'\n'
        ;;
    esac
  done <<<"$log"

  local output=""
  for key in "${section_order[@]}"; do
    if [ -n "${sections[$key]}" ]; then
      output+="## ${section_titles[$key]}"$'\n\n'
      output+="${sections[$key]}"$'\n'
    fi
  done

  echo "$output"
}
