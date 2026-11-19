#!/usr/bin/env bash
# Create (once) the key that signs automatic updates and configure it:
#   - private: ~/.tauri/gtav-mod-manager.key → TAURI_SIGNING_PRIVATE_KEY secret in the GitHub repo (for CI);
#   - public:  plugins.updater.pubkey in apps/desktop/src-tauri/tauri.conf.json (goes in the task's commit).
# The private key is never printed. Back it up: if it is lost, installed versions can no longer
# update themselves.
# It runs on a feature/* or release/* branch, because it leaves a change in tauri.conf.json that
# travels through a PR.
#
# Usage: make updater-key      (GMM_UPDATER_KEY=<path> changes where the key lives)
set -euo pipefail
cd "$(dirname "$0")/.."

die() { echo "✗ $*" >&2; exit 1; }
step() { echo "→ $*"; }

key="${GMM_UPDATER_KEY:-$HOME/.tauri/gtav-mod-manager.key}"
conf=apps/desktop/src-tauri/tauri.conf.json

case "$(git branch --show-current)" in
  feature/*|release/*) ;;
  *) die "run make updater-key on a feature/* or release/* branch: the tauri.conf.json change goes through a PR" ;;
esac

[ -f "$conf" ] || die "missing $conf"
command -v gh >/dev/null || die "the GitHub CLI (gh) is missing"
gh auth status >/dev/null 2>&1 || die "gh has no session: run 'gh auth login'"

if [[ -s $key ]]; then
  step "Reusing the existing key $key (creating another one would break updates)"
else
  step "Generating the key in $key"
  mkdir -p "$(dirname "$key")"
  # --ci: no password, so CI can sign with the secret alone.
  npx --yes @tauri-apps/cli@2 signer generate --ci -w "$key" >/dev/null
  chmod 600 "$key"
fi
[[ -s "$key.pub" ]] || die "the public key $key.pub is missing"
pub=$(tr -d '\r\n' < "$key.pub")
# The key is base64; anything else would need escaping when written into the JSON below.
[[ $pub =~ ^[A-Za-z0-9+/=]+$ ]] || die "unexpected characters in $key.pub"

step "Storing the private key as the TAURI_SIGNING_PRIVATE_KEY secret of $(gh repo view --json nameWithOwner -q .nameWithOwner)"
gh secret set TAURI_SIGNING_PRIVATE_KEY < "$key"

step "Writing the public key into $conf"
awk -v pub="$pub" '
  /^[[:space:]]*"pubkey": "/ { sub(/"pubkey": "[^"]*"/, "\"pubkey\": \"" pub "\""); done = 1 }
  { print }
  END { exit(done ? 0 : 3) }
' "$conf" > "$conf.tmp" \
  || { rm -f "$conf.tmp"; die "could not find a \"pubkey\" line in $conf"; }
mv "$conf.tmp" "$conf"
grep -qF "\"pubkey\": \"$pub\"" "$conf" || die "could not write the public key into $conf"

echo
echo "✓ Done. Include $conf in the branch commit and keep a copy of $key somewhere safe"
echo "  (a password manager). Do not upload it to the repository: if it is lost, installed"
echo "  versions can no longer update themselves."
