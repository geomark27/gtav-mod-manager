#!/usr/bin/env bash
# Fails when a Tauri Rust crate and its npm package are on different major.minor versions.
# `tauri build` refuses to bundle in that case, so CI catches it here instead of at release time.
#   tauri               ↔ @tauri-apps/api
#   tauri-plugin-<name> ↔ @tauri-apps/plugin-<name>
# Reads the resolved versions from Cargo.lock and apps/desktop/package-lock.json.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 - <<'EOF'
import json, re, sys

lock = open("Cargo.lock", encoding="utf-8").read()
crates = dict(re.findall(r'\[\[package\]\]\nname = "(tauri(?:-plugin-[a-z0-9-]+)?)"\nversion = "([^"]+)"', lock))
npm = json.load(open("apps/desktop/package-lock.json", encoding="utf-8"))["packages"]

def npm_version(name):
    entry = npm.get(f"node_modules/{name}")
    return entry and entry.get("version")

def minor(v):
    return ".".join(v.split(".")[:2])

pairs = [("tauri", "@tauri-apps/api")] + [
    (c, "@tauri-apps/plugin-" + c[len("tauri-plugin-"):]) for c in sorted(crates) if c.startswith("tauri-plugin-")
]
bad = []
for crate, pkg in pairs:
    cv, nv = crates.get(crate), npm_version(pkg)
    if not cv or not nv:
        continue  # a crate without npm package (or the reverse) is not a mismatch
    status = "ok" if minor(cv) == minor(nv) else "MISMATCH"
    print(f"{status:9} {crate} {cv}  ↔  {pkg} {nv}")
    if status != "ok":
        bad.append(crate)
if bad:
    print("✗ align these on the same major.minor (pin npm with ~X.Y.Z, or cargo update -p <crate>)", file=sys.stderr)
    sys.exit(1)
print("✓ Tauri crates and npm packages are aligned")
EOF
