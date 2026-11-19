#!/usr/bin/env bash
# Carry develop to main: review what is missing, open the PR (merge commit, never squash or
# rebase) and leave the merge to its author. Only the project owner runs this.
#   make promote        (yes=1 skips the question)
set -euo pipefail
cd "$(dirname "$0")/.."

remote=origin
main=main
base=develop

fail() { echo "✗ $*" >&2; exit 1; }
ok() { echo "✓ $*"; }

command -v gh >/dev/null && gh auth status >/dev/null 2>&1 || fail "gh is needed and authenticated: gh auth login"
[ -z "$(git status --porcelain)" ] || fail "there are uncommitted changes: commit or discard them first"

git fetch --quiet --prune "$remote" "$main" "$base"
if git rev-parse -q --verify "refs/heads/$base" >/dev/null; then
  git merge-base --is-ancestor "$base" "$remote/$base" \
    || fail "$base has commits that are not in $remote/$base: everything reaches $base through a PR"
fi
[ "$(git rev-list --count "$remote/$main..$remote/$base")" -gt 0 ] || { ok "$main already has everything from $base"; exit 0; }

pending=$(git log --no-merges --reverse --format='- %s' "$remote/$main..$remote/$base")
echo "Reaching $main:"
echo "$pending"
echo

pr=$(gh pr list --base "$main" --head "$base" --state open --json number,url --jq '.[0] | "\(.number) \(.url)"' || true)
if [ -n "$pr" ]; then
  num=${pr%% *}
  url=${pr#* }
  ok "PR #$num ($base → $main) is already open: $url"
else
  if [ "${YES:-}" != 1 ]; then
    printf 'Open the PR %s → %s? [y/N]: ' "$base" "$main" >&2
    read -r answer || answer=""
    [[ "$answer" =~ ^[yYsS]$ ]] || { echo "No PR created."; exit 0; }
  fi
  url=$(gh pr create --base "$main" --head "$base" --title "Promote develop to main" \
    --body "$(printf 'Promotion of develop into main.\n\nCommits:\n\n%s\n\nMerge with a **merge commit**: never squash or rebase.\n' "$pending")")
  ok "PR created: $url"
fi

echo
echo "Next steps (the owner, by hand):"
echo "  1. Merge the PR with a merge commit (never squash, never rebase and merge)."
echo "  2. make release        (tags origin/main; Actions builds, signs and publishes the installer)"
echo "This script never merges."
