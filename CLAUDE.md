# CLAUDE.md

Read [AGENTS.md](AGENTS.md) first: it overrides any default and defines what each agent owns,
the Rust and TypeScript conventions, and the safety rules.

## Change log (mandatory)

Every change on a branch needs its entry in the devlog (`docs/devlog/`, settings in
`.devlog.conf`). Follow the `devlog` skill before committing or opening a PR: a change without an
entry is not finished.

With several agents, only the orchestrator writes the entry; subagents return a `DEVLOG
CONTRIBUTION` block in their report (`agent:`, `what:`, `why:`, `files:`, `test:`, `follow-up:`)
and the orchestrator consolidates it.

`make devlog` (part of `make check`) checks the current branch's entry against `develop`. CI runs
only on the promotion PR to `main` (tests before a release), not on every branch.

## Branches and releases

Work on `feature/*`, `fix/*`, `docs/*` or `release/*`; **never commit on `main` or `develop`** —
they only receive merges, through a PR with a merge commit. `make start name=<topic>
type=feature|fix|docs|release` creates the branch.

Release: see [docs/release.md](docs/release.md). Only the project owner runs `make promote` and
`make release`; agents never run them, and never run git or `gh` commands that write.
