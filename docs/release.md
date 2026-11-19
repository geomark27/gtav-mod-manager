# Release process

How a change gets from a branch to a published installer. Everything runs from the repository
root and is driven by `make` targets; `make help` lists them with their options.

```
feature/*  ──PR──▶  develop  ──PR──▶  main  ──tag vX.Y.Z──▶  GitHub Actions  ──▶  release
                                    (make promote)          (make release)        (installer +
                                                                                  SHA256SUMS.txt +
                                                                                  latest.json)
```

Two roles: anyone can open branches and PRs to `develop`; the **project owner confirms the merges**
and is the only one who runs `make promote` and `make release`. Never commit on `main` or
`develop`: they only receive merges.

## 1. Day-to-day: a branch and its PR

```sh
make start name=my-change type=feature   # feature/* | fix/* | docs/* | release/*, from develop
# ... write code, tests and the devlog entry (docs/devlog/), and add the line to [Unreleased]
#     in CHANGELOG.md when the change is visible to users ...
make check                               # fmt-check, clippy, cargo test, UI, devlog entry
git add -A && git commit -m "feat: ..."
make ship                                # push + pr + merge, stopping at the first failure
```

`make ship` is a shortcut for `make push`, `make pr` and `make merge`. Run them separately to see
each step. `make push` refuses `main` and `develop`; `make merge` asks before merging and always
uses a **merge commit** (never squash, never rebase and merge).

When the PR is merged, remove the local remains from the main folder:

```sh
cd <main folder>
make finish                 # removes the branch's worktree and deletes the local branch
```

`make finish` only removes a branch that is already merged into `origin/develop` and has no
uncommitted changes. `make prune` cleans up other merged branches.

`make sync` fetches `origin` and fast-forwards `develop` only; it never merges or rewrites.

## 2. Promote develop to main

```sh
make promote
```

It lists the commits that are missing in `main`, opens the PR `develop` → `main` (or shows the one
already open) and asks first (`yes=1` skips the question). It **never merges**: merge the PR with a
merge commit, by hand or from the web. `make release` refuses to run while `main` is behind
`develop`.

## 3. Raise the version

The version lives in three files and must match everywhere:

- `Cargo.toml` (`[workspace.package] version`) — the source of truth;
- `apps/desktop/src-tauri/tauri.conf.json` (`version`);
- `apps/desktop/package.json` (`version`).

`make bump` writes all of them, the two root fields of `apps/desktop/package-lock.json` and the
`gmm-*` entries of `Cargo.lock`, turns `[Unreleased]` in `CHANGELOG.md` into the version's section
and closes the devlog for the version (`docs/devlog/releases/vX.Y.Z.md`). Only on a `release/*`
branch (it also works on `feature/*`, `fix/*` and `docs/*`):

```sh
make start name=release-0.2.0 type=release
make bump bump=minor              # or: make bump ver=0.2.0, or ver=v0.2.0
#    DRY_RUN=1 make bump bump=minor   shows what it would do and how many devlog entries ship
make check
git add -A && git commit -m "release: v0.2.0"
make ship                         # to develop
```

Then promote and release (next two sections). `make bump` refuses on `main` and `develop`, with a
dirty working tree, with a version that is lower than the current one, and with a tag that already
exists. It warns when no devlog entry would ship with the version.

The very first release is `make bump ver=0.1.0`: the version is already `0.1.0` everywhere, so only
`[Unreleased]` is turned into `[0.1.0]` and the devlog is closed.

## 4. Publish

```sh
make promote      # merge develop into main
make release      # tags origin/main as vX.Y.Z (DRY_RUN=1 make release simulates)
```

`make release` checks that `origin/main` has everything from `origin/develop`, that the three files
agree with the tag, that the updater public key is in place, that the GitHub secret exists and that
the tag is free. Then it creates the annotated tag on `origin/main`, pushes it and opens the
release as a draft with the `CHANGELOG.md` section as notes. It asks before doing any of that
(`yes=1` skips the question).

The tag starts `.github/workflows/release.yml`:

1. `prepare` — the tag matches the three versions, the updater key is not the placeholder, and the
   draft release exists.
2. `build` (windows-latest) — builds the UI, compiles the app with
   `src-tauri/tauri.release.conf.json` (which signs the updater artifact), renames the installer to
   `GTAV-Mod-Manager-windows-x64-setup.exe`, writes `SHA256SUMS.txt`, uploads both to the release
   and keeps the `.sig` as a workflow artifact.
3. `publish` — builds `latest.json` from the signature and publishes the release as *latest*.

The installer is ~15–25 minutes. Follow it with:

```sh
gh run watch $(gh run list --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId')
```

Every release publishes:

| Asset | Purpose |
| --- | --- |
| `GTAV-Mod-Manager-windows-x64-setup.exe` | Installer; the fixed name keeps `releases/latest/download/…` pointing at the newest one. |
| `SHA256SUMS.txt` | SHA-256 of the installer, to verify the download. |
| `latest.json` | Manifest the built-in updater reads; it needs the `.sig` generated in `build`. |

## 5. The updater key (once)

Automatic updates are signed. The key pair is created once, on a `feature/*` or `release/*` branch:

```sh
make updater-key
```

It creates `~/.tauri/gtav-mod-manager.key` (private) and writes the public key into
`apps/desktop/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`, which starts as
`"PENDING: run make updater-key"`). The private key is stored in the repository as the
`TAURI_SIGNING_PRIVATE_KEY` secret, which is what the release workflow uses.

**Back the private key up** (password manager, offline copy) and never commit it. Installed
versions only accept updates signed with the matching private key: if it is lost, every user has to
install a new version by hand.

## 6. Test an installer without releasing

```sh
gh workflow run release.yml -f ref=develop
gh run watch $(gh run list --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId')
gh run download $(gh run list --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId') \
  -n gtav-mod-manager-installer -D ./
```

The `workflow_dispatch` run builds with the plain configuration (no updater signature, no secret)
and uploads the `.exe` as the `gtav-mod-manager-installer` workflow artifact. Nothing is published
and no tag is needed. The updater cannot be exercised this way: it needs a signed release.

## 7. SmartScreen

The installer is not code-signed (there is no code-signing certificate), so the first run on a new
machine shows **“Windows protected your PC”**:

> More info → Run anyway

This is the expected behavior, not a bug. The updater signature protects the updates, not the first
download; `SHA256SUMS.txt` lets anyone check the file.

## 8. If a release run fails

The draft release stays unpublished, which is safe: nothing reaches users.

1. Read the failing job in Actions (usually a version mismatch, the missing secret, or no `.sig`).
2. Fix the cause on a branch, merge it, and either
   - re-run the failed jobs from the Actions tab (**Re-run failed jobs**), or
   - delete the tag (`git push origin :refs/tags/vX.Y.Z`, `git tag -d vX.Y.Z`) and run `make release`
     again once `origin/main` is correct.
3. The installer can be tried first with a `workflow_dispatch` run (section 6).

Never force-push or move a published tag: installed versions use it to find the next update.
