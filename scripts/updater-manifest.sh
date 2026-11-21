#!/usr/bin/env bash
# Build latest.json, the manifest the updater of ModSwitch reads, from the .sig file the
# Windows build leaves next to the installer (fixed asset name), and upload it to the release.
# CI runs it after building. It needs gh with GH_REPO and GH_TOKEN, plus jq (or python3).
#
# Usage: scripts/updater-manifest.sh v0.1.0 <folder-with-the-.sig>
#
# UPDATER_MANIFEST_OUT=<path>: writes latest.json there and does not upload it (to test without
# network or gh; GH_REPO may be unset). The release notes come from the version's CHANGELOG
# section; before the first release they fall back to [Unreleased] and then to the release page.
#
# Windows only: the updater looks for "windows-x86_64-nsis" first and "windows-x86_64" after.
set -euo pipefail

tag=${1:?usage: scripts/updater-manifest.sh vX.Y.Z <folder-with-the-.sig>}
sigs=${2:?usage: scripts/updater-manifest.sh vX.Y.Z <folder-with-the-.sig>}
version=${tag#v}
installer=GTAV-Mod-Manager-windows-x64-setup.exe
repo=${GH_REPO:-geomark27/gtav-mod-manager}
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT

[[ -f $sigs/$installer.sig ]] || { echo "no such file: $sigs/$installer.sig" >&2; exit 1; }

url="https://github.com/$repo/releases/download/$tag/$installer"
signature=$(tr -d '\r\n' < "$sigs/$installer.sig")

notes=$(bash "$(dirname "$0")/changelog-notes.sh" "$version" 2>/dev/null \
  || bash "$(dirname "$0")/changelog-notes.sh" Unreleased 2>/dev/null \
  || true)
[[ -n $notes ]] || notes="Release notes: https://github.com/$repo/releases/tag/$tag"

if command -v jq >/dev/null; then
  jq -n --arg version "$version" --arg notes "$notes" --arg date "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg url "$url" --arg sig "$signature" \
    '{version: $version, notes: $notes, pub_date: $date,
      platforms: {
        "windows-x86_64-nsis": {url: $url, signature: $sig},
        "windows-x86_64": {url: $url, signature: $sig}
      }}' > "$dir/latest.json"
elif command -v python3 >/dev/null; then
  python3 - "$version" "$url" "$signature" "$notes" "$dir/latest.json" <<'PY'
import json
import sys
from datetime import datetime, timezone

version, url, signature, notes, out = sys.argv[1:6]
platforms = ("windows-x86_64-nsis", "windows-x86_64")
manifest = {
    "version": version,
    "notes": notes,
    "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {name: {"url": url, "signature": signature} for name in platforms},
}
with open(out, "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2)
    handle.write("\n")
PY
else
  echo "jq (or python3) is missing" >&2
  exit 1
fi

if [[ -n ${UPDATER_MANIFEST_OUT:-} ]]; then
  cp "$dir/latest.json" "$UPDATER_MANIFEST_OUT"
  echo "latest.json for $tag written to $UPDATER_MANIFEST_OUT (not uploaded)"
  exit 0
fi

echo "latest.json for $tag:"
echo "  windows-x86_64-nsis"
echo "  windows-x86_64"
gh release upload "$tag" "$dir/latest.json" --clobber
