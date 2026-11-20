#!/usr/bin/env bash
# Publish a GTA V Mod Manager version from main.
#   With bump= or ver= (run on main): runs fmt-check, clippy, the tests and the UI checks, raises
#   the version (scripts/bump.sh: the three files, the
#   lockfiles, CHANGELOG.md and the devlog card), commits "release: vX.Y.Z" on main and pushes it.
#   Then, always:
#   1. checks that origin/main has everything from origin/develop, that the three version files
#      agree, that the updater key is configured and that the tag is free;
#   2. puts the tag vX.Y.Z on origin/main, uploads it and opens the release as a draft with `gh`;
#   3. fast-forwards origin/develop to origin/main, so the next work starts from the release.
# Once the tag arrives, GitHub Actions (.github/workflows/release.yml) builds the Windows
# installer, signs it for the updater, attaches it with SHA256SUMS.txt and latest.json, and
# publishes the release.
# It never switches branches; it only commits the version bump, and only on main.
#
# Usage: scripts/release.sh <patch|minor|major|""> [X.Y.Z] [--dry-run]   (YES=1 skips the question)
#   make release                     publish the version already in main
#   make release bump=minor          raise the version on main, commit, push and publish
#   make release ver=0.3.0           same with an exact version
#   DRY_RUN=1 only shows what it would do; yes=1 skips the question
set -euo pipefail
cd "$(dirname "$0")/.."

die() { echo "✗ $*" >&2; exit 1; }
step() { echo "→ $*"; }

bump=${1:-}
ver=${2:-}
dry_run=false
[[ ${3:-} == "--dry-run" ]] && dry_run=true
raise=false
[[ -n $bump || -n $ver ]] && raise=true

command -v gh >/dev/null || die "the GitHub CLI (gh) is missing"
gh auth status >/dev/null 2>&1 || die "gh has no session: run 'gh auth login'"

step "Checking the remote"
git fetch --quiet --prune --tags origin main develop

pending=$(git rev-list --count origin/main..origin/develop)
[[ $pending -eq 0 ]] \
  || die "develop has $pending commit(s) that are not in main: run make promote and merge the PR"

taken="it already exists: raise the version with make release bump=patch|minor|major"

# Raise the version on main first: commit and push it, then publish as usual.
if $raise; then
  [[ $(git branch --show-current) == main ]] \
    || die "a new version is made on main: git switch main, then make release bump=…"
  [[ -z $(git status --porcelain) ]] \
    || die "there are uncommitted changes: commit or stash them first"
  if [[ $(git rev-parse HEAD) != $(git rev-parse origin/main) ]]; then
    git merge-base --is-ancestor HEAD origin/main \
      || die "local main has commits that are not in origin/main: push them (git push) first"
    step "Updating local main"
    git merge --quiet --ff-only origin/main
  fi
  target=$(BUMP_IN_PLACE=1 bash scripts/bump.sh "$bump" "$ver" --print-version) || exit 1
  git rev-parse -q --verify "refs/tags/v$target" >/dev/null && die "v$target $taken"
  [[ -z $(git ls-remote --tags origin "refs/tags/v$target") ]] || die "v$target $taken"
  BUMP_IN_PLACE=1 bash scripts/bump.sh "$bump" "$ver" --dry-run
  if $dry_run; then
    echo
    echo "Dry run (DRY_RUN): would commit \"release: v$target\" on main, push it, tag and publish."
    exit 0
  fi
  # No PR runs CI before this release: the same checks run here, before anything is written.
  step "Checking the code (fmt, clippy, tests, UI)"
  make --no-print-directory fmt-check lint test ui >/dev/null \
    || die "the checks failed (run make fmt-check lint test ui to see why): nothing was changed"
  echo "  ✓ checks passed"
  if [[ ${YES:-} != 1 ]]; then
    read -r -p "Raise to $target, commit on main, push and publish v$target? [y/N] " answer \
      || answer=""
    [[ $answer =~ ^[yYsS]$ ]] || die "cancelled"
  fi
  BUMP_IN_PLACE=1 bash scripts/bump.sh "$bump" "$ver"
  step "Committing and pushing release: v$target"
  git add -A
  git commit --quiet -m "release: v$target"
  git push --quiet origin main
  git fetch --quiet origin main
  # The question was already asked.
  YES=1
fi

# The version lives in three files and the tag must match all of them (release.yml checks too).
cargo_version=$(git show origin/main:Cargo.toml \
  | awk '/^\[workspace\.package\]/{s=1;next} /^\[/{s=0} s && /^version = "/{gsub(/^version = "|"$/,"");print;exit}')
app_version=$(git show origin/main:apps/desktop/src-tauri/tauri.conf.json \
  | sed -n 's/^  "version": "\(.*\)",$/\1/p')
ui_version=$(git show origin/main:apps/desktop/package.json \
  | sed -n 's/^  "version": "\(.*\)",$/\1/p')
[[ $cargo_version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
  || die "could not read [workspace.package] version from Cargo.toml in origin/main"
[[ $app_version == "$cargo_version" && $ui_version == "$cargo_version" ]] \
  || die "origin/main says $cargo_version in Cargo.toml, '$app_version' in tauri.conf.json and '$ui_version' in package.json: fix the files on main"
tag="v$cargo_version"

# Without the key, installed versions cannot verify nor receive updates.
if git show origin/main:apps/desktop/src-tauri/tauri.conf.json | grep -q '"pubkey": "PENDING'; then
  die "the updater public key is not in main: run make updater-key on a work branch and merge it"
fi
gh secret list --json name -q '.[].name' | grep -qx TAURI_SIGNING_PRIVATE_KEY \
  || die "the repo has no TAURI_SIGNING_PRIVATE_KEY secret: run make updater-key"

git rev-parse -q --verify "refs/tags/$tag" >/dev/null && die "$tag $taken"
remote_tag=$(git ls-remote --tags origin "refs/tags/$tag") || die "could not query the remote tags"
[[ -z $remote_tag ]] || die "$tag $taken"

previous=$(git describe --tags --abbrev=0 origin/main 2>/dev/null || true)
if [[ -n $previous ]]; then
  newest=$(printf '%s\n%s\n' "${previous#v}" "$cargo_version" | sort -V | tail -1)
  [[ $newest == "$cargo_version" ]] || die "$cargo_version is not higher than the last tag ($previous)"
fi

notes=$(git show origin/main:CHANGELOG.md | bash scripts/changelog-notes.sh "$cargo_version" - 2>/dev/null) \
  || die "CHANGELOG.md in main has no [$cargo_version] section: use make release bump=patch|minor|major"
commit=$(git rev-parse --short origin/main)
repo_url=$(git remote get-url origin \
  | sed -E 's#^git@([^:]+):#https://\1/#; s#^ssh://git@([^/]+)/#https://\1/#; s#\.git$##')

echo
echo "Release $tag on origin/main ($commit), last tag: ${previous:-none}"
echo "Notes:"
echo "$notes" | sed 's/^/  │ /'
echo

if $dry_run; then
  echo "Dry run (DRY_RUN): no tag and no release were created."
  exit 0
fi

if [[ ${YES:-} != 1 ]]; then
  read -r -p "Publish $tag to $repo_url? [y/N] " answer || answer=""
  [[ $answer =~ ^[yYsS]$ ]] || die "cancelled"
fi

step "Creating and uploading the tag $tag"
git tag -a "$tag" origin/main -m "GTA V Mod Manager $tag"
git push --quiet origin "$tag"

# The «prepare» job of the workflow also creates the draft: whichever gets there first wins.
step "Creating the draft release"
gh release view "$tag" >/dev/null 2>&1 \
  || printf '%s\n' "$notes" | gh release create "$tag" --draft --verify-tag \
    --title "GTA V Mod Manager $tag" --notes-file -

# develop follows main, so the next branch starts from the released version. develop is
# already contained in main (checked above), so this is a fast-forward.
step "Moving develop to the release"
if git push --quiet origin origin/main:refs/heads/develop 2>/dev/null; then
  if [[ $(git branch --show-current) != develop ]] && git show-ref --verify --quiet refs/heads/develop; then
    git fetch --quiet origin develop:develop 2>/dev/null || true
  fi
else
  echo "! could not fast-forward develop to main: do it by hand (a PR main → develop)"
fi

echo
echo "✓ $tag published. GitHub Actions is building the installer (~15–25 min):"
echo "    gh run watch \$(gh run list --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId')"
echo "  When it finishes, the release publishes itself: $repo_url/releases/tag/$tag"
