# Development setup

> **Last reviewed:** 2026-11-04 · **Last updated:** 2026-11-04 · **Change:** minor

InboxCraft is developed in two layers. The Rust toolchain and every check run in a container; the host only runs the
binary the container built. The `justfile` is the one command surface over both, and a thin layer over
`cargo xtask`: `just ci` is `cargo xtask ci`, and CI calls `cargo xtask ci` directly.

## The two layers
| Layer | Where | What runs there |
|---|---|---|
| Container | the `rust` service in `docker-compose.yml`, image from `Dockerfile` | `cargo`, `rustc`, clippy, rustfmt, nextest, cargo-deny, cargo-llvm-cov, `cargo xtask` |
| Host | your desktop session | the built `inboxcraft` binary from `target/`, through `just run` and `just run-sample` |

The working copy is bind-mounted at `/workspace`, so `target/` is shared: what the container builds, the host runs.
The image is based on Debian trixie, whose glibc is older than that of current desktop distributions, so the binary
runs on the host as it is. The container user has your UID and GID, so files keep their owner.

The host needs only `git`, `just` and Docker or Podman with Compose. It needs no Rust toolchain.

## First steps
```sh
just setup        # build the image, start the container, fetch dependencies
just ci           # every gate CI runs (= cargo xtask ci, in the container)
just build        # dev profile; just build release for an optimised build
just run-sample   # run the app on the host with the invented sample mailbox
just --list       # every recipe
```

Other recipes: `test` (nextest; extra arguments go to it), `lint`, `fmt`, `fmt-check`, `deny`, `cov`, `layers`,
`wasm`, `shell`, `dev`, `down`, `rebuild` and `clean` (removes the cargo volumes and `target/`).
`just run -- <args>` passes arguments to the app; `INBOXCRAFT_PROFILE=release just run` runs the release build.

### Podman
Set `COMPOSE_CMD="podman compose"`, in the environment or in a `.env` file next to the `justfile` (`.env` is
gitignored).

### Editors
`.devcontainer/devcontainer.json` opens the same `rust` service in editors that support dev containers.

## What runs only on the host
These need the desktop session and do not work in the container:
- the GUI window (Wayland or X11, GPU drivers),
- the tray icon (StatusNotifierItem on the session bus),
- desktop notifications,
- the Secret Service (the OS credential store that holds passwords and tokens).

Run anything that touches them with `just run` or `just run-sample`. Headless checks and tests never need them.

## Native mode for agents
With `AGENT_HORDE=1` the build and check recipes run the toolchain directly instead of through `docker compose exec`.
This is for a shell that already is the container, such as an agent sandbox or `just shell`. The lifecycle recipes
(`setup`, `dev`, `down`, `clean`, `rebuild`, `shell`) and the host recipes (`run`, `run-sample`) refuse in native
mode.

The agent-horde sandbox image needs the same Debian packages as the `Dockerfile` (the egui and winit build
prerequisites and `libdbus-1-dev`), plus the toolchain components, the `wasm32-unknown-unknown` target and the cargo
tools pinned there. As of 2026-11-04 it has them (cargo 1.98.0, nextest 0.9.150, cargo-deny 0.20.2, cargo-llvm-cov
0.9.1), and `AGENT_HORDE=1 just ci` runs green in it.

## The workspace
| Directory | What is in it |
|---|---|
| `crates/<name>` | the library crates of the architecture (`docs/architecture.md`), package `inboxcraft-<name>` |
| `apps/<name>` | the binaries: `inboxcraft`, `inboxcraft-cli`, `inboxcraft-web` |
| `xtask/` | the gates behind `cargo xtask` |

Every crate root starts with the SPDX lines and the deny header
(`#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`),
and every crate has its tests in files of its own (`src/tests.rs`, `src/<module>_tests.rs`, `tests/`). The crates
are empty so far; the apps answer `--version` and refuse everything else.

## The gates (`cargo xtask`)
| Command | What it checks |
|---|---|
| `cargo xtask ci` | every gate below, in this order: `cargo fmt --check`, clippy with `-D warnings`, `spdx`, `test-files`, `assets`, `layers`, nextest (profile `ci`, `.config/nextest.toml`), doctests, `cov --check`, `wasm`, `cargo deny check`, `stats` |
| `cargo xtask spdx` | every `.rs` file starts with a `// SPDX-FileCopyrightText:` line and `// SPDX-License-Identifier:` with the workspace licence; `--fix` adds what is missing, a file under another licence is reported and never changed |
| `cargo xtask test-files` | no inline test module (`mod tests { ... }`, `mod <name>_tests { ... }`, or any module with a body under `#[cfg(test)]`) outside test files |
| `cargo xtask cov` | line coverage per crate and for the workspace under `cargo llvm-cov nextest` (profile `ci`), the report in `lcov.info`; `--check` fails below the floors in `.config/coverage.toml` (see "The coverage gate") |
| `cargo xtask assets` | every asset file (picture, icon, font or sound by extension, or anything under `assets/`) has a row in `ATTRIBUTION.md`; `<name>` in a row path stands for one or more characters within one directory |
| `cargo xtask layers` | the layering rules below, from the table in `xtask/src/layers.rs` |
| `cargo xtask wasm` | the standalone and layer 0 to 5 crates, all but `net`, build for `wasm32-unknown-unknown` (one `cargo build`) |
| `cargo xtask stats` | Rust files, code lines and test lines (not blank) per crate, and a total; it only reports |
| `cargo xtask help` | the commands and the exit codes |

`ci` runs every gate even after one fails and names the failed ones at the end. Exit codes: 0 passed, 1 a gate
failed, 2 a usage or setup error. `--root <dir>` points a gate at another workspace (the tests use it). The licence
is written in one place, `workspace.package.license` in the root `Cargo.toml`; `spdx` reads it from there.

### The layering rules
Every workspace crate has a class in `xtask/src/layers.rs`: `Layer(n)` (0 to 6), `Standalone` (`control-key`,
`charset`), `Testkit` (`testkit`) or `Exempt` (the apps and `xtask`). A new crate is registered there in the commit
that adds it. `layers` reads the direct dependencies of every member (normal, dev and build, target-specific ones
included) and fails on:

- a member that is not registered;
- a dependency on a higher layer, or on a crate in its own layer that `INTRA_LAYER_ORDER` does not list (only
  `net` -> `proto` and `net` -> `dav`);
- a standalone crate with any workspace dependency;
- `testkit` as a normal or build dependency (dev-dependencies only);
- a UI crate (`egui`, `eframe`, `winit`, `egui_kittest`, `rfd`) anywhere but layer 6 and the apps;
- a network crate (`NETWORK_CRATES`: `rustls`, `tokio-rustls`, `native-tls`, `openssl`, `socket2`, `mio`, `hyper`,
  `reqwest`, `ureq`, `oauth2` and the like) anywhere but `net`, `testkit` and the apps;
- a dependency on an app or `xtask` from a library crate.

Member patterns in the root `Cargo.toml` are directories or `<dir>/*`; `layers`, `wasm` and `stats` refuse other
globs.

Files the gates skip: hidden directories, `target/`, `target-*/` and `plan/`; `assets` also skips the gitignored
local directories `corpus/`, `craft-fonts/`, `craft-i18n/`, `dist/` and `log/`.

### The coverage gate
`cargo xtask cov` runs `cargo llvm-cov nextest --workspace --profile ci --lcov --output-path lcov.info`, so the
tests run under coverage and a failing test fails the gate. It then prints one line per crate (lines, covered,
cover, floor) and the workspace total. `just cov` runs it; `just cov --check` and `just ci` also fail below a floor.
`lcov.info` stays in the root for the Codecov upload; a stale one is removed before the run.

The floors live in one file, `.config/coverage.toml`:

| Floor | Crates |
|---|---|
| 95 % | the workspace total |
| 98 % | `charset`, `dav`, `engine`, `ical`, `mime`, `model`, `proto`, `store`, `testkit`, `time`, `vcard` |
| 80 % | `net`, `ui-egui` |

- A line is an lcov `DA` record (llvm-cov already leaves out the test files); it is covered when its count is not 0.
  A file belongs to the member whose directory holds it most closely.
- `exclude` lists files counted nowhere: `xtask/src/main.rs`, the `main` dispatch.
- `workspace-exclude` lists files counted for their own crate but not for the workspace total: the apps' `main`
  (`apps/*/src/main.rs`); the `ui-egui` drawing code joins it when it lands.
- Patterns are paths of `/`-separated components: a name, `*` for one component, or `**` last for one or more.
- A crate with no lines has nothing to fall below. Cover is printed rounded down, so a crate below its floor never
  prints as on it.
- A floor is never lowered in the PR that falls below it; lowering one needs the project owner.

## CI
`.github/workflows/ci.yml` runs `cargo xtask ci`, the same gates in the same order as `just ci`, on pull requests
into `develop`, `main` and `epic/**`, on pushes to those branches, and on demand (`gh workflow run ci.yml --ref
<branch>`).

| Job | What it does | Token |
|---|---|---|
| `draft` | on a draft PR only: one step, so the draft shows a check | read |
| `test` | `cargo xtask ci` on `ubuntu-24.04`; uploads `lcov.info` and `target/nextest/ci/junit.xml` as the `reports` artifact | read |
| `codecov` | uploads the coverage and the test results to Codecov with the job's GitHub OIDC token; runs no repository code | read, `id-token: write` |
| `ci` | the gate: succeeds only when `test` and `codecov` did | read |

- **Drafts get only the quick check.** The full run starts when the PR is marked ready (`gh pr ready`) and repeats
  on every later push; a newer run of the same PR cancels the older one. Run `just ci` locally before marking a PR
  ready.
- `ci` is the check to require. A skipped check reports as passing, so a required check has to be a job that runs on
  every full run.
- `test` installs the Debian packages of the `Dockerfile` (without the ones the runner already has), the toolchain
  from `rust-toolchain.toml`, and nextest, cargo-llvm-cov and cargo-deny at the versions the `Dockerfile` pins: it
  reads them from there, so a bump stays one change.
- No cargo cache: nothing a PR run writes feeds a later run.
- Every action is pinned by its full commit SHA with the tag in a comment; the workflow token is read-only and only
  `codecov` widens it.

`.github/workflows/fuzz.yml` runs every target in `fuzz/fuzz_targets/` with cargo-fuzz for 300 seconds each, nightly
at 02:17 UTC, and for 30 seconds on a PR that touches `fuzz/` or the workflow. cargo-fuzz needs nightly Rust: the
toolchain is pinned by date in the workflow. A crash fails the run and its input is kept as the `fuzz-artifacts`
artifact; it becomes a regression test. With no targets the job passes. GitHub runs a schedule only from the default
branch, `main`, so the nightly run starts with the first release.

`.github/workflows/close-issues-on-merge.yml` closes the issues named on their own `Closes #N` line when a PR is
merged into `develop`. A slice merged into an epic branch closes nothing; the epic PR into `develop` closes them all.

`.github/dependabot.yml` opens weekly grouped PRs into `develop`: `cargo-dependencies` (prefix `chore(deps)`, only
releases at least 7 days old) and `actions` (prefix `ci(deps)`).

## Bumping a pinned version
- **Rust:** change `channel` in `rust-toolchain.toml` and the base image tag in the `Dockerfile` together; a new tag
  also needs its new multi-arch index digest.
- **Downloaded tools:** every download in the `Dockerfile` is checked against a sha256 committed next to its version;
  a bump updates the version and both per-architecture checksums.
- **cargo-nextest:** keep `CARGO_NEXTEST_VERSION` at or above `nextest-version` in `.config/nextest.toml`.
- **Fuzzing:** `FUZZ_TOOLCHAIN` and `CARGO_FUZZ_VERSION` in `.github/workflows/fuzz.yml`.
- **Actions:** Dependabot bumps the SHA and the tag comment together.

After a `Dockerfile` change, run `just rebuild`.

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-11-03 | major | First version: the two-layer model, the justfile, native mode (#1) |
| 2026-11-04 | minor | The workspace layout and the first `cargo xtask` gates: `ci`, `spdx`, `test-files` (#5) |
| 2026-11-04 | minor | The `assets`, `layers`, `wasm` and `stats` gates and the layering rules (#6) |
| 2026-11-04 | minor | The coverage gate `cov` and its floors (#7) |
| 2026-11-04 | minor | CI: the workspace workflow with Codecov, nightly fuzzing, Dependabot (#9) |
| 2026-11-04 | minor | The `testkit` coverage floor (#8) |
