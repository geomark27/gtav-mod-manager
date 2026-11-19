#!/usr/bin/env bash
# Print the body of a CHANGELOG section (without its heading) so it can be used as release notes.
# Usage: scripts/changelog-notes.sh <version|Unreleased> [file]
#   file: path of the CHANGELOG (default: CHANGELOG.md at the repo root); "-" reads standard input
#   (e.g. git show origin/main:CHANGELOG.md | scripts/changelog-notes.sh 0.1.0 -).
set -euo pipefail

section=${1:?usage: scripts/changelog-notes.sh <version|Unreleased> [file|-]}
changelog=${2:-"$(dirname "$0")/../CHANGELOG.md"}
[[ $changelog == "-" || -f $changelog ]] || { echo "no such file: $changelog" >&2; exit 1; }

# From "## [<section>]" to the next "## [" or the link references at the end,
# without leading or trailing blank lines.
notes=$(awk -v s="## [$section]" '
  index($0, "## [") == 1 { if (found) exit; found = (index($0, s) == 1); next }
  /^\[[^]]+\]: / { if (found) exit }
  found { lines[++n] = $0; if ($0 ~ /[^[:space:]]/) { if (!first) first = n; last = n } }
  END { for (i = first; first && i <= last; i++) print lines[i] }
' "$changelog")

[[ -n $notes ]] || { echo "CHANGELOG.md has no notes under [$section]" >&2; exit 1; }
printf '%s\n' "$notes"
