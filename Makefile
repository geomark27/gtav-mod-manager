# GTA V Mod Manager — shortcuts for the git flow, releases and quality checks.
# `make` (or `make help`) lists every target; options are passed as variables:
#   make start name=release-pipeline type=feature
#   make bump bump=minor          (or make bump ver=0.2.0)
#   make release                  (tags origin/main; GitHub Actions builds the installer)
#
# Scripts never switch branches on their own, never commit and ask before any outward action
# (push, PR, tag, release). `yes=1` skips the question and `DRY_RUN=1` only simulates.

SHELL := bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help

# WSL: the rustup toolchain comes before a system rustc.
export PATH := $(HOME)/.cargo/bin:$(PATH)

REMOTE := origin
BASE   := develop
MAIN   := main
BRANCH := $(shell git branch --show-current 2>/dev/null)
MAIN_DIR := $(shell git worktree list --porcelain 2>/dev/null | awk '/^worktree /{sub(/^worktree /,""); print; exit}')

# Branch kinds that may be pushed and opened as a PR. main and develop are integration-only.
WORK_BRANCHES := feature/*|fix/*|docs/*|release/*

# Web link of the remote (accepts https://… and git@host:…) for compare links.
REPO_URL_SH = git remote get-url $(REMOTE) | sed -E 's\#^git@([^:]+):\#https://\1/\#; s\#^ssh://git@([^/]+)/\#https://\1/\#; s\#\.git$$\#\#'

# Parameters (name, type, b, run, dest, …) reach the shell as environment variables.
export name type b run dest

GUARD_BRANCH = case "$(BRANCH)" in main|$(BASE)|"") echo "✗ cannot push or open a PR from '$(BRANCH)': work on a feature/*, fix/*, docs/* or release/* branch" >&2; exit 1;; $(WORK_BRANCHES)) ;; *) echo "✗ '$(BRANCH)' is not a work branch (feature/*, fix/*, docs/* or release/*)" >&2; exit 1;; esac
GH_OK = command -v gh >/dev/null && gh auth status >/dev/null 2>&1

.PHONY: help sync start status push pr merge ship finish prune promote \
        version notes bump updater-key release \
        fmt fmt-check lint test ui devlog check \
        dev windows-sync

##@ Git

sync: ## Fetch origin and fast-forward develop only, from any folder
	@git fetch $(REMOTE); \
	dev_dir=$$(git worktree list --porcelain | awk -v b="refs/heads/$(BASE)" '/^worktree /{p=substr($$0,10)} $$0=="branch " b {print p; exit}'); \
	nofast() { echo "✗ $(BASE) cannot fast-forward (it diverged from $(REMOTE)/$(BASE), or local changes get in the way): nothing was merged or rewritten, look at it by hand" >&2; exit 1; }; \
	if [ "$(BRANCH)" = "$(BASE)" ]; then \
		git pull --ff-only $(REMOTE) $(BASE) || nofast; \
	elif [ -n "$$dev_dir" ]; then \
		git -C "$$dev_dir" merge --ff-only $(REMOTE)/$(BASE) || nofast; \
	else \
		git fetch $(REMOTE) $(BASE):$(BASE) || nofast; \
	fi; \
	echo "✓ $(BASE) up to date$(if $(filter-out $(BASE),$(BRANCH)), (you are still on $(BRANCH)),)"

start: ## New branch from an up-to-date develop: make start name=<topic> [type=feature|fix|docs|release]
	@[[ "$(name)" =~ ^[a-z0-9]+(-[a-z0-9]+)*$$ ]] || { echo "✗ name=<topic> in lowercase with hyphens (e.g. name=release-pipeline)" >&2; exit 1; }
	@case "$(type)" in feature|fix|docs|release) ;; *) echo "✗ type must be feature, fix, docs or release" >&2; exit 1;; esac
	@[ -z "$$(git status --porcelain)" ] || { echo "✗ there are uncommitted changes: commit or stash them first" >&2; exit 1; }
	@! git show-ref --verify --quiet "refs/heads/$(type)/$(name)" || { echo "✗ branch $(type)/$(name) already exists" >&2; exit 1; }
	@$(MAKE) --no-print-directory sync
	@git switch -c $(type)/$(name) $(BASE)
	@git branch --unset-upstream 2>/dev/null || true
	@echo "✓ branch $(type)/$(name) created from $(BASE)"
	@echo "  Next: make check · commit · make ship"

status: ## Where the current branch stands: commits, upload, changes and PR
	@br="$(BRANCH)"; \
	if [ -z "$$br" ]; then echo "Detached HEAD: there is no branch status to show."; exit 0; fi; \
	echo "Branch:        $$br"; \
	case "$$br" in feature/*|fix/*|docs/*|release/*) ;; *) echo "Not a work branch (feature|fix|docs|release): nothing to show."; exit 0;; esac; \
	echo "Commits over $(REMOTE)/$(BASE): $$(git rev-list --count $(REMOTE)/$(BASE)..HEAD 2>/dev/null || echo '?')"; \
	if git rev-parse --verify --quiet "refs/remotes/$(REMOTE)/$$br" >/dev/null; then \
		pending=$$(git rev-list --count "$(REMOTE)/$$br..HEAD"); \
		if [ "$$pending" -eq 0 ]; then echo "Upload:        yes, up to date"; else echo "Upload:        yes, $$pending commit(s) not uploaded"; fi; \
	else echo "Upload:        no"; fi; \
	if [ -n "$$(git status --porcelain)" ]; then echo "Changes:       yes, uncommitted"; else echo "Changes:       none"; fi; \
	if $(GH_OK); then \
		num=$$(gh pr list --head "$$br" --state open --json number --jq '.[0].number // empty' 2>/dev/null || true); \
		if [ -n "$$num" ]; then url=$$(gh pr list --head "$$br" --state open --json url --jq '.[0].url // empty' 2>/dev/null || true); echo "Open PR:       #$$num $$url"; \
		else echo "Open PR:       none"; fi; \
	else echo "Open PR:       (gh is not authenticated)"; fi

push: ## Upload the current branch and set its upstream; force=1 uses --force-with-lease
	@$(GUARD_BRANCH)
	git push -u $(REMOTE) $(BRANCH) $(if $(force),--force-with-lease,)

pr: ## Show the open PR of the branch or create one to develop (web=1 opens the browser); without gh, the link
	@$(GUARD_BRANCH)
	@if $(GH_OK); then \
		open=$$(gh pr list --head "$(BRANCH)" --state open --json url --jq '.[0].url // empty'); \
		if [ -n "$$open" ]; then echo "✓ PR already open: $$open"; \
		else gh pr create --base $(BASE) --head $(BRANCH) --fill $(if $(web),--web,); fi; \
	else \
		url=$$($(REPO_URL_SH)); echo "gh is not authenticated; open the PR here:"; echo "$$url/compare/$(BASE)...$(BRANCH)?expand=1"; \
	fi

merge: ## Merge the branch's PR into develop with a merge commit; asks first (yes=1 skips)
	@MERGE_YES=$(yes) bash scripts/merge.sh

ship: ## push + pr + merge, stopping at the first failure; then finish from the main folder
	@$(GUARD_BRANCH)
	@$(MAKE) --no-print-directory push
	@$(MAKE) --no-print-directory pr
	@MERGE_AVISO=0 $(MAKE) --no-print-directory merge
	@echo "now, from the main folder: make finish"

finish: ## After the PR is merged: switch to develop (fast-forward), remove the branch's worktree if it has one, delete the local branch (b=<branch>)
	@br="$(if $(b),$(b),$(BRANCH))"; \
	case "$$br" in main|"$(BASE)"|"") echo "✗ nothing to finish from '$$br'" >&2; exit 1;; $(WORK_BRANCHES)) ;; *) echo "✗ '$$br' is not a work branch (feature/*, fix/*, docs/* or release/*)" >&2; exit 1;; esac; \
	[ "$$(git rev-parse --absolute-git-dir)" = "$$(git rev-parse --path-format=absolute --git-common-dir)" ] \
		|| { echo "✗ run make finish from the main folder: cd $(MAIN_DIR)" >&2; exit 1; }; \
	[ -z "$$(git status --porcelain)" ] || { echo "✗ uncommitted changes in this folder: commit or discard them first" >&2; exit 1; }; \
	git fetch --quiet --prune $(REMOTE); \
	git merge-base --is-ancestor "$$br" "$(REMOTE)/$(BASE)" \
		|| { echo "✗ '$$br' is not merged into $(REMOTE)/$(BASE): merge its PR with a merge commit first (squash and rebase do not count)" >&2; exit 1; }; \
	wt=$$(git worktree list --porcelain | awk -v b="refs/heads/$$br" '/^worktree /{p=substr($$0,10)} $$0=="branch " b {print p; exit}'); \
	if [ -n "$$wt" ] && [ "$$wt" != "$$(git rev-parse --show-toplevel)" ]; then \
		git worktree remove "$$wt" || { echo "✗ could not remove $$wt: it has uncommitted or untracked files; clean it by hand" >&2; exit 1; }; \
		echo "✓ worktree $$wt removed"; \
	fi; \
	if [ "$(BRANCH)" = "$$br" ]; then git switch "$(BASE)" && git merge --quiet --ff-only "$(REMOTE)/$(BASE)"; fi; \
	git branch -d "$$br" || { echo "✗ git did not delete local branch '$$br'" >&2; exit 1; }; \
	echo "✓ '$$br' closed"; \
	echo "The remote branch is deleted by its author (not done here):"; \
	echo "  git push $(REMOTE) --delete $$br"

prune: ## Delete local feature/fix/docs/release branches already merged into develop
	@git fetch --quiet --prune $(REMOTE); \
	tip=$$(git rev-parse "$(BASE)" 2>/dev/null || echo ''); \
	for b in $$(git branch --merged "$(BASE)" --format='%(refname:short)'); do \
		case "$$b" in main|"$(BASE)"|"$(BRANCH)") continue;; $(WORK_BRANCHES)) ;; *) continue;; esac; \
		[ -z "$$tip" ] || [ "$$(git rev-parse "$$b")" != "$$tip" ] || continue; \
		git branch -d "$$b" || echo "· $$b is checked out in another worktree: run make finish there"; \
	done

promote: ## Review what develop still has over main and offer the PR (yes=1 skips); never merges
	@YES=$(yes) bash scripts/promote.sh

##@ Versions

version: ## Show the version in the three files and the last tag
	@echo "Cargo.toml (workspace): $$(awk '/^\[workspace\.package\]/{s=1;next} /^\[/{s=0} s && /^version = "/{gsub(/^version = "|"$$/,"");print;exit}' Cargo.toml)"
	@echo "tauri.conf.json:        $$(sed -n 's/^  "version": "\(.*\)",$$/\1/p' apps/desktop/src-tauri/tauri.conf.json)"
	@echo "package.json:           $$(sed -n 's/^  "version": "\(.*\)",$$/\1/p' apps/desktop/package.json)"
	@echo "last tag:               $$(git tag -l 'v[0-9]*' --sort=-v:refname | head -1 | grep . || echo none)"

notes: ## Show what is unpublished ([Unreleased] in CHANGELOG.md)
	@bash scripts/changelog-notes.sh Unreleased

bump: ## On a work branch: set the version and turn [Unreleased] into it: make bump bump=patch|minor|major | ver=X.Y.Z (DRY_RUN=1 simulates)
	@bash scripts/bump.sh "$(bump)" "$(ver)" $(if $(DRY_RUN),--dry-run)

updater-key: ## Once, on a work branch: create the updater signing key, upload it as a secret and write the public key
	@bash scripts/updater-key.sh

release: ## After make promote: tag origin/main and open the release; Actions builds, signs and publishes (DRY_RUN=1 simulates)
	@[ -z "$(bump)$(ver)" ] || { echo "✗ make release does not change the version: do that with make bump on a release/* branch" >&2; exit 1; }
	@YES="$(yes)" bash scripts/release.sh $(if $(DRY_RUN),--dry-run)

##@ Quality

fmt: ## Format the Rust code (cargo fmt --all)
	cargo fmt --all

fmt-check: ## Check the Rust format without writing
	cargo fmt --all -- --check

lint: ## clippy without warnings (default members; the Tauri shell is built on Windows)
	cargo clippy --all-targets -- -D warnings

test: ## Rust tests of the workspace crates; run=<name> filters by name
	cargo test $(run)

ui: ## Type-check, test and build the UI
	cd apps/desktop && npm run typecheck && npm test && npm run build

devlog: ## Verify this branch's devlog entry against develop
	@bash .claude/skills/devlog/scripts/verify.sh --base $(BASE)

check: ## What must pass before a commit and in CI: Tauri versions, fmt-check, lint, test, ui, devlog
	@bash scripts/check-tauri-versions.sh
	@$(MAKE) --no-print-directory fmt-check
	@$(MAKE) --no-print-directory lint
	@$(MAKE) --no-print-directory test
	@$(MAKE) --no-print-directory ui
	@$(MAKE) --no-print-directory devlog

##@ Dev

dev: ## Run the app with hot reload (Windows: cd apps/desktop && npm run tauri dev)
	@echo "The Tauri app builds and runs on Windows (WebView2); from WSL use make windows-sync first."
	cd apps/desktop && npm run tauri dev

windows-sync: ## Copy the tree to a Windows drive for a native build: make windows-sync dest=/mnt/c/dev/gtav-mod-manager
	@bash scripts/sync-to-windows.sh "$(dest)"

##@ Help

help: ## Show this help
	@awk 'BEGIN { FS = ":.*## " } \
	/^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } \
	/^[a-z-]+:.*## / { printf "  \033[36m%-13s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@echo
	@echo "Options: name=<topic> type=feature|fix|docs|release  bump=patch|minor|major  ver=X.Y.Z"
	@echo "         run=  force=1  web=1  yes=1  DRY_RUN=1  b=<branch>  dest=<path>"
