#!/usr/bin/env bash
# Set the version of GTA V Mod Manager (it never commits). On main it is run by `make release
# bump=…`, which commits and publishes; `make bump` alone works on work branches:
#   - writes the version to the root Cargo.toml ([workspace.package]), tauri.conf.json,
#     package.json, the two root fields of package-lock.json and the gmm-* entries of Cargo.lock;
#   - turns [Unreleased] in CHANGELOG.md into the new version and updates the links at the end;
#   - closes the devlog for the version (docs/devlog/releases/vX.Y.Z.md).
# To publish a new version: on main, make release bump=patch|minor|major (see docs/release.md).
#
# Usage: scripts/bump.sh <patch|minor|major|""> [X.Y.Z] [--dry-run|--print-version]
#   --print-version                    only print the target version (used by release.sh)
#   BUMP_IN_PLACE=1                    allow main (set by release.sh)
#   make bump bump=patch|minor|major   raise that part of the current version
#   make bump ver=X.Y.Z                exact version (the "v" is optional); it may be the current
#                                      one only when its tag does not exist yet (first release)
#   DRY_RUN=1                          only show what it would do
set -euo pipefail
cd "$(dirname "$0")/.."

die() { echo "✗ $*" >&2; exit 1; }
step() { echo "→ $*"; }

bump=${1:-}
version=${2#v}
dry_run=false
print_version=false
[[ ${3:-} == "--dry-run" ]] && dry_run=true
[[ ${3:-} == "--print-version" ]] && print_version=true
in_place=false

# [workspace.package] version of the Cargo.toml read on stdin.
workspace_version() {
  awk '/^\[workspace\.package\]/{s=1;next} /^\[/{s=0} s && /^version = "/{gsub(/^version = "|"$/,"");print;exit}'
}

# The target version for `current` from bump= or ver= (printed; dies on bad input).
target_version() {
  local current=$1 major minor patch
  [[ -z $bump || -z $version ]] || die "use bump= or ver=, not both"
  if [[ -z $bump ]]; then
    echo "$version"
    return
  fi
  IFS=. read -r major minor patch <<<"$current"
  case $bump in
    patch) echo "$major.$minor.$((patch + 1))" ;;
    minor) echo "$major.$((minor + 1)).0" ;;
    major) echo "$((major + 1)).0.0" ;;
    *) die "invalid bump: '$bump' (patch, minor or major)" ;;
  esac
}

branch=$(git branch --show-current)
case "$branch" in
  feature/*|fix/*|docs/*|release/*) ;;
  main)
    if [[ ${BUMP_IN_PLACE:-} == 1 ]]; then
      in_place=true
    else
      die "on main the version is raised and published in one step: make release bump=patch|minor|major (or ver=X.Y.Z)"
    fi
    ;;
  develop) die "releases are made from main: git switch main, then make release bump=patch|minor|major" ;;
  "") die "detached HEAD: switch to main or to a work branch first" ;;
  *) die "'$branch' is not a work branch (feature/*, fix/*, docs/* or release/*)" ;;
esac
if $print_version; then
  current=$(workspace_version < Cargo.toml)
  version=$(target_version "$current") || exit 1
  [[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
    || die "give the version: bump=patch|minor|major (or ver=X.Y.Z)"
  echo "$version"
  exit 0
fi
if [[ -n $(git status --porcelain) ]]; then
  # A dry run only reads, so it is allowed with a dirty tree; the real run needs a clean one.
  if $dry_run; then
    echo "! uncommitted changes (fine for a dry run; commit before the real bump)"
  else
    die "there are uncommitted changes: commit or stash them before bumping"
  fi
fi

# The version lives in three files; the workspace Cargo.toml is the source of truth.
current=$(workspace_version < Cargo.toml)
[[ $current =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "could not read [workspace.package] version from Cargo.toml"
version=$(target_version "$current") || exit 1
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
  || die "give the version: make bump bump=patch|minor|major  (or ver=X.Y.Z)"
tag="v$version"
IFS=. read -r new_major new_minor new_patch <<<"$version"

# Release type for the devlog card: major, minor or patch.
same=false
if [[ $version == "$current" ]]; then
  same=true
  [[ -z $(git tag -l "$tag") ]] \
    || die "the current version is already published ($tag exists): use bump= or a higher version"
  if ((new_patch != 0)); then release_type=patch
  elif ((new_minor != 0)); then release_type=minor
  else release_type=major
  fi
else
  newest=$(printf '%s\n%s\n' "$current" "$version" | sort -V | tail -1)
  [[ $newest == "$version" ]] || die "$version is lower than the current version ($current)"
  IFS=. read -r cur_major cur_minor _cur_patch <<<"$current"
  if ((new_major != cur_major)); then release_type=major
  elif ((new_minor != cur_minor)); then release_type=minor
  else release_type=patch
  fi
fi

[[ -f CHANGELOG.md ]] || die "CHANGELOG.md is missing at the repo root"
if grep -qE "^## \[$version\]" CHANGELOG.md; then
  die "CHANGELOG.md already has a [$version] section"
fi
notes=$(bash scripts/changelog-notes.sh Unreleased 2>/dev/null) \
  || die "write what changed under [Unreleased] in CHANGELOG.md first"

# Last published tag (highest version, excluding this one).
previous=$(git tag -l 'v[0-9]*' --sort=-v:refname | grep -vxF "$tag" | head -1 || true)
repo_url=$(git remote get-url origin 2>/dev/null \
  | sed -E 's#^git@([^:]+):#https://\1/#; s#^ssh://git@([^/]+)/#https://\1/#; s#\.git$##') \
  || true
[[ -n $repo_url ]] || die "could not read the URL of the origin remote"

echo
if $same; then
  echo "Version $current (same number; only [Unreleased] is closed)${previous:+. Last tag: $previous}"
else
  echo "Version $current → $version${previous:+ (last tag: $previous)}"
fi
echo "Type: $release_type"
echo "Notes:"
echo "$notes" | sed 's/^/  │ /'
echo

# The devlog is closed with the same notes that go into the release (§9 of its guide).
devlog=.claude/skills/devlog/scripts/close-release.sh
close_args=("$version" --type "$release_type" --notes "<notes file>")
[[ -z $previous ]] || close_args+=(--previous "$previous")

if [[ -f $devlog ]]; then
  pending=$("$devlog" "$version" --type "$release_type" --dry-run 2>&1 \
    | sed -n 's/.*would ship in v[^ ]* (\([0-9][0-9]*\)).*/\1/p' | head -1)
  pending=${pending:-0}
  echo "Devlog entries that would ship with this version: $pending"
  if ((pending == 0)); then
    echo "! no unreleased devlog entry closes with this version: write one before releasing"
  fi
else
  echo "! $devlog is missing: the devlog will not be closed"
fi

if $dry_run; then
  echo
  echo "Dry run (DRY_RUN): nothing was changed."
  if [[ -f $devlog ]]; then
    echo "Would run: $devlog ${close_args[*]}"
  fi
  exit 0
fi

if ! $same; then
  step "Writing version $version"
  awk -v v="$version" '
    /^\[workspace\.package\]/ { s = 1; print; next }
    /^\[/ { s = 0 }
    s && /^version = "/ { print "version = \"" v "\""; next }
    { print }
  ' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
  sed -i -E "s/^  \"version\": \".*\",$/  \"version\": \"$version\",/" \
    apps/desktop/src-tauri/tauri.conf.json apps/desktop/package.json

  # The lockfile keeps the version twice at the root: the project and packages."".
  awk -v v="$version" '
    n < 2 && /^[[:space:]]*"version": "/ { sub(/"version": "[^"]*"/, "\"version\": \"" v "\""); n++ }
    { print }
  ' apps/desktop/package-lock.json > apps/desktop/package-lock.json.tmp \
    && mv apps/desktop/package-lock.json.tmp apps/desktop/package-lock.json

  # Every gmm-* package of the workspace must carry the new version.
  locked() {
    awk -v v="$version" '
      /^name = "gmm-/ { f = 1; next }
      f && /^version = / { if ($0 != "version = \"" v "\"") bad = 1; f = 0 }
      END { exit (bad ? 1 : 0) }
    ' Cargo.lock
  }
  if ! { cargo update --workspace --offline --quiet 2>/dev/null && locked; }; then
    echo "! cargo update could not refresh Cargo.lock: rewriting the gmm-* entries only"
    awk -v v="$version" '
      /^name = "gmm-/ { f = 1; print; next }
      f && /^version = / { print "version = \"" v "\""; f = 0; next }
      { print }
    ' Cargo.lock > Cargo.lock.tmp && mv Cargo.lock.tmp Cargo.lock
    locked || die "could not update the gmm-* versions in Cargo.lock"
  fi

  # The three files must agree, or the tag check in release.yml stops the release.
  app=$(sed -n 's/^  "version": "\(.*\)",$/\1/p' apps/desktop/src-tauri/tauri.conf.json)
  pkg=$(sed -n 's/^  "version": "\(.*\)",$/\1/p' apps/desktop/package.json)
  [[ $app == "$version" && $pkg == "$version" ]] \
    || die "the version did not land in all files (tauri.conf.json: '$app', package.json: '$pkg')"
  first_two=$(grep -m2 '"version": "' apps/desktop/package-lock.json \
    | sed -E 's/.*"version": "([^"]*)".*/\1/' | paste -sd+ -)
  [[ $first_two == "$version+$version" ]] \
    || die "could not update the two root versions of package-lock.json"
fi

step "Updating CHANGELOG.md"
today=$(date +%F)
if [[ -n $previous ]]; then
  version_link="[$version]: $repo_url/compare/$previous...$tag"
else
  version_link="[$version]: $repo_url/releases/tag/$tag"
fi
unreleased_link="[Unreleased]: $repo_url/compare/$tag...HEAD"
if grep -q '^\[Unreleased\]: ' CHANGELOG.md; then
  awk -v header="## [$version] - $today" \
      -v unreleased_link="$unreleased_link" -v version_link="$version_link" '
    $0 == "## [Unreleased]" { print; print ""; print header; next }
    index($0, "[Unreleased]: ") == 1 { print unreleased_link; print version_link; next }
    { print }
  ' CHANGELOG.md > CHANGELOG.md.tmp && mv CHANGELOG.md.tmp CHANGELOG.md
else
  awk -v header="## [$version] - $today" '
    $0 == "## [Unreleased]" { print; print ""; print header; next }
    { print }
  ' CHANGELOG.md > CHANGELOG.md.tmp && mv CHANGELOG.md.tmp CHANGELOG.md
  # No link references in the file: append them at the end.
  { echo; echo "$unreleased_link"; echo "$version_link"; } >> CHANGELOG.md
fi

if [[ -f $devlog ]]; then
  step "Closing the devlog for v$version"
  notes_file=$(mktemp)
  trap 'rm -f "$notes_file"' EXIT
  bash scripts/changelog-notes.sh "$version" > "$notes_file"
  if [[ -n $previous ]]; then
    "$devlog" "$version" --type "$release_type" --previous "$previous" --notes "$notes_file" >/dev/null
  else
    "$devlog" "$version" --type "$release_type" --notes "$notes_file" >/dev/null
  fi
  rm -f "$notes_file"
  trap - EXIT
  echo "  ✓ docs/devlog/releases/v$version.md"
fi

echo
echo "✓ Version $version is in the working tree (nothing committed)."
$in_place || echo "  Review the diff, then: make check · commit · make ship"
