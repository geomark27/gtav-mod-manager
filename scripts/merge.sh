#!/usr/bin/env bash
# Merge the PR of the current branch into develop, always with a merge commit. The author runs
# it (it needs gh authenticated).
#   make merge [yes=1]
set -euo pipefail

remote=origin
base=develop

fail() { echo "✗ $*" >&2; exit 1; }
ok() { echo "✓ $*"; }

command -v gh >/dev/null && gh auth status >/dev/null 2>&1 || fail "gh is needed and authenticated: gh auth login"

branch=$(git branch --show-current)
case "$branch" in
  feature/*|fix/*|docs/*|release/*) ;;
  main|"$base"|"") fail "there is no PR to merge from '${branch:-detached HEAD}': work on a feature/*, fix/*, docs/* or release/* branch" ;;
  *) fail "'$branch' is not a work branch (feature/*, fix/*, docs/* or release/*)" ;;
esac

git fetch --quiet "$remote" "$branch" 2>/dev/null || fail "'$branch' is not in $remote: run make push"
[ "$(git rev-list --count "$remote/$branch..$branch")" -eq 0 ] || fail "'$branch' has commits that were not uploaded: run make push"

pr=$(gh pr list --head "$branch" --state open --json number --jq '.[0].number // empty')
[ -n "$pr" ] || fail "there is no open PR for '$branch': run make pr"
target=$(gh pr view "$pr" --json baseRefName --jq .baseRefName)
[ "$target" = "$base" ] || fail "PR #$pr goes to '$target', not to $base: develop → main is make promote"
title=$(gh pr view "$pr" --json title --jq .title)

if [ "${MERGE_YES:-}" != 1 ]; then
  printf 'Merge PR #%s «%s» into %s with a merge commit? [y/N]: ' "$pr" "$title" "$base" >&2
  read -r answer || answer=""
  [[ "$answer" =~ ^[yYsS]$ ]] || fail "not merged: PR #$pr stays open"
fi

# The branch is not deleted here: make finish removes it from the main folder.
gh pr merge "$pr" --merge
ok "PR #$pr merged into $base with a merge commit"

# make ship sets MERGE_AVISO=0 because it prints this same hint at the end.
if [ "${MERGE_AVISO:-1}" != 0 ]; then
  echo "now, from the main folder: make finish"
fi
