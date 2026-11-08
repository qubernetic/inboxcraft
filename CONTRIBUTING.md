# Contributing to InboxCraft

> **Last reviewed:** <date> · **Last updated:** 2026-11-08 · **Change:** minor

Thank you for wanting to help. InboxCraft handles people's private mail, so how work comes in matters as much as the
work itself.

## Before you build anything
1. Read [AGENTS.md](AGENTS.md): the never-crash rules, the mail-is-hostile rules, the asset policy and the quality
   gates are short and not negotiable.
2. Look at [ROADMAP.md](ROADMAP.md) and [docs/gaps.md](docs/gaps.md) for what is wanted.
3. Claim an issue by commenting on it; a maintainer assigns it. A claim lapses after 14 days without movement.
4. A bug together with its fix needs no claim: send both at once.
5. A new feature that is not in the issues: open one first and wait for a yes.

## Development setup
You need `git`, [just](https://just.systems) and Docker or Podman with Compose; no Rust toolchain on your computer.
The toolchain and every check run in a dev container, and the app runs on your desktop from the container's build:

```sh
just setup        # build the image, start the container, fetch dependencies
just ci           # every gate CI runs (cargo xtask ci, in the container)
just build        # then: just run-sample
```

Podman: `COMPOSE_CMD="podman compose"`. The details, and what only works on the host, are in
[docs/development.md](docs/development.md).

## The pull request
- Branch from the latest `develop`: `feature/<issue>-<slug>` or `fix/<issue>-<slug>`. One issue per pull request,
  with `Closes #<n>`. A feature in several parts: one PR per part, `Builds on #<n>`.
- Tick "Allow edits by maintainers".
- `just ci` (= `cargo xtask ci`) passes locally.
- A crash or data-loss fix comes with a regression test; tests live in files of their own.
- Every new source file starts with the SPDX lines (`cargo xtask spdx --fix`).
- Never use a real mailbox in tests: use the fake servers and `example.org` addresses.
- Record your change in a change fragment, `changes/<issue>.md`: its `CHANGELOG.md` lines, and where it applies the
  `ROADMAP.md` progress line, new `docs/gaps.md` lines and a revision row for each document you touch. Do not edit
  those files or the documents' status lines directly; the fragments are folded in when an epic or a release closes.
  `just changes` checks them (and `just ci` does too); the format is in
  [docs/development.md](docs/development.md#change-fragments).
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/): one logical change each, imperative,
  lowercase, no period, ≤ 72 characters (`fix(imap): keep UIDVALIDITY across reconnects`).
- The PR title is a Conventional Commit too, with the issue number: `feat(triage): add screener folder (#42)`.
- Pull requests are merged with a merge commit (`--no-ff`); squash and rebase merges are not used, so keep your
  commits clean.

## A first contribution
Issues labelled `good first issue` are small and self-contained.

## Decisions that are the project owner's
InboxCraft is owned and maintained by [Qubernetic](https://qubernetic.com). These decisions are Qubernetic's:
Licensing, the project's name and icon, publishing releases, signing keys, new dependencies with network access, and
anything that changes what leaves the person's computer.

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-03 | minor | Development setup with the justfile and the dev container (#1) |
| 2026-11-08 | minor | Change fragments instead of a direct CHANGELOG edit (#111) |
