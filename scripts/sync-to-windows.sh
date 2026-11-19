#!/usr/bin/env bash
# Copies the source tree to a folder on a Windows drive so the Tauri app can be built and run
# natively on Windows (building from \\wsl$ paths is slow and unreliable).
# Excludes build outputs and dependencies; Windows gets its own node_modules and target/.
#
# Usage: scripts/sync-to-windows.sh /mnt/c/dev/gtav-mod-manager
set -euo pipefail

dest="${1:-}"
[ -n "$dest" ] || { echo "usage: $0 <destination under /mnt/<drive>/...>" >&2; exit 2; }
case "$dest" in
	/mnt/[a-z]/*) ;;
	*) echo "destination must be on a Windows drive (/mnt/<drive>/...)" >&2; exit 2 ;;
esac

src="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$dest"
tar -C "$src" \
	--exclude=./target --exclude='*/node_modules' --exclude='*/dist' \
	--exclude=./apps/desktop/src-tauri/gen --exclude=./.delegar --exclude=./opencode.json \
	-cf - . | tar -C "$dest" -xf -
echo "✓ synced to $dest"
echo "  On Windows: cd <that folder>\\apps\\desktop && npm install && npm run tauri dev"
