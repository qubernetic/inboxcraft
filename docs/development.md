# Development setup

> **Last reviewed:** 2026-11-04 · **Last updated:** 2026-11-05 · **Change:** minor

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
`wasm`, `shot` (the `ui_shot` pictures of the sample window, see `docs/design.md`), `icon` (exports the app icon),
`flatpak-sources` and `flatpak-sources-check`, `shell`, `dev`, `down`, `rebuild`, `clean` (removes the cargo
volumes and `target/`), on the host `flatpak` (see "The dev Flatpak") and, for the owner only, `it-live` (see "Live
tests").
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
behind `inboxcraft` (`--sample` for the invented sample mailbox, `--help`) open the window; `inboxcraft-cli` and
`inboxcraft-web` answer `--version` and refuse everything else.

## The gates (`cargo xtask`)
| Command | What it checks |
|---|---|
| `cargo xtask ci` | every gate below, in this order: `cargo fmt --check`, clippy with `-D warnings`, `spdx`, `test-files`, `assets`, `layers`, `flatpak-sources-check`, doctests, `cov --check` (the one test run, nextest profile `ci`, `.config/nextest.toml`), `wasm`, `cargo deny check`, `stats` |
| `cargo xtask spdx` | every `.rs` file starts with a `// SPDX-FileCopyrightText:` line and `// SPDX-License-Identifier:` with the workspace licence; `--fix` adds what is missing, a file under another licence is reported and never changed |
| `cargo xtask test-files` | no inline test module (`mod tests { ... }`, `mod <name>_tests { ... }`, or any module with a body under `#[cfg(test)]`) outside test files |
| `cargo xtask cov` | line coverage per crate and for the workspace under `cargo llvm-cov nextest` (profile `ci`), the report in `lcov.info`; `--check` fails below the floors in `.config/coverage.toml` (see "The coverage gate") |
| `cargo xtask assets` | every asset file (picture, icon, font or sound by extension, or anything under `assets/`) has a row in `ATTRIBUTION.md`; `<name>` in a row path stands for one or more characters within one directory |
| `cargo xtask layers` | the layering rules below, from the table in `xtask/src/layers.rs` |
| `cargo xtask flatpak-sources-check` | `contrib/flatpak/cargo-sources.json` is what `Cargo.lock` gives (see "The dev Flatpak"); `cargo xtask flatpak-sources` writes it |
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
  `store` -> `model`, `net` -> `proto` and `net` -> `dav`);
- a standalone crate with any workspace dependency;
- `testkit` as a normal or build dependency (dev-dependencies only);
- `testkit` with any dependency on `proto` (`TESTKIT_NEVER`): a fake server never uses the parser it tests;
- a UI crate (`egui`, `eframe`, `winit`, `egui_kittest`, `rfd`) anywhere but layer 6 and the apps;
- a network crate (`NETWORK_CRATES`: `rustls`, `tokio-rustls`, `native-tls`, `openssl`, `socket2`, `mio`, `hyper`,
  `reqwest`, `ureq`, `oauth2` and the like) anywhere but `net`, `testkit` and the apps;
- a testkit-only crate (`TESTKIT_ONLY_CRATES`: `rcgen`) as a normal or build dependency anywhere but `testkit`; a
  dev-dependency on one is fine, and a crate that comes to need one lifts it in a reviewed change;
- a net-only crate (`NET_ONLY_CRATES`: `flate2`, the COMPRESS=DEFLATE stream) as a normal or build dependency
  anywhere but `net` and `testkit`;
- a dependency on an app or `xtask` from a library crate.

Member patterns in the root `Cargo.toml` are directories or `<dir>/*`; `layers`, `wasm` and `stats` refuse other
globs.

Files the gates skip: hidden directories, `target/`, `target-*/` and `plan/`; `assets` also skips the gitignored
local directories `corpus/`, `craft-fonts/`, `craft-i18n/`, `dist/` and `log/`.

### The coverage gate
`cargo xtask cov` runs `cargo llvm-cov nextest --workspace --profile ci --lcov --output-path lcov.info`, so the
tests run under coverage and a failing test fails the gate. In `cargo xtask ci` this is the only test run: there is
no separate nextest gate, so the tests run only in the coverage-instrumented build. When the run fails the gate says
`cov: the tests failed (...); no coverage floor was checked`, so a red test never reads as a floor. nextest writes
the JUnit report of profile `ci` to `target/nextest/ci/junit.xml` in the root, whatever target directory llvm-cov
builds in. Otherwise it prints one line per crate (lines, covered,
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

## The dev Flatpak
`contrib/flatpak/` builds InboxCraft as a Flatpak with the app id `io.github.qubernetic.inboxcraft`, from the
checkout and without a network during the build. After the first setup, `git pull && just flatpak` updates the
installed app, and it starts from the desktop's menu like any other: the entry opens the app, its second action
("Open the Sample Mailbox") starts it with `--sample`.

| File | What it is |
|---|---|
| `io.github.qubernetic.inboxcraft.yml` | the manifest: the freedesktop 25.08 runtime with the `rust-stable` SDK extension, the build commands and the permissions |
| `cargo-sources.json` | every crate of `Cargo.lock` as a download with its checksum, unpacked into a vendor folder for the offline build |
| `io.github.qubernetic.inboxcraft.desktop` | the menu entry, with the sample mailbox as its second action |
| `io.github.qubernetic.inboxcraft.metainfo.xml` | the AppStream description software centres show |

The icon is `assets/app-icon/inboxcraft.svg`, exported from the shapes in `crates/ui-egui/src/icons.rs`; the same
shapes are the window icon. After a change to the shapes, `just icon` exports the file again; a test fails while the
file and the code differ.

**Permissions.** The manifest asks for as little as the app needs, each with its reason in a comment above it: the
network (IMAP, SMTP, CalDAV), a Wayland window with X11 only as a fallback (and the shared memory X11 needs), the
GPU, the Secret Service for passwords and tokens, desktop notifications, and the Downloads folder, where Save puts
attachments (under a new name, never over a file). The Background portal needs no permission. No other folder of
yours is open to the app: everything it keeps lives in `~/.var/app/io.github.qubernetic.inboxcraft/`. A test fails
when a permission has no comment above it, or when the manifest opens a folder other than Downloads.

**Dependencies.** A change to `Cargo.lock` needs `just flatpak-sources` (in the container, or natively), and the
regenerated `contrib/flatpak/cargo-sources.json` goes into the same commit; the `flatpak-sources-check` gate of
`cargo xtask ci` fails until it is there. Only crates.io crates are allowed: a git or other registry dependency is
refused.

### Build and install it locally
On the host you need `flatpak` and `flatpak-builder` (the `flatpak-builder` package of your distribution, or the
`org.flatpak.Builder` app from Flathub), and the Flathub remote for your user once:

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
```

Then, in the checkout:

```sh
git pull
just flatpak
```

`just flatpak` lints the manifest with `flatpak-builder-lint` when it is installed (or the linter of the
`org.flatpak.Builder` app); the linter's findings are shown and the build goes on, as Flathub's rules are stricter
than a dev build needs. It then runs `flatpak-builder --user --install`, which fetches the runtime, the SDK and the
Rust extension from Flathub the first time, builds in `target/flatpak/` and installs the app for your user. The
first build compiles every crate and takes a while; later ones reuse the downloads. Run it with
`flatpak run io.github.qubernetic.inboxcraft` (add `--sample` for the sample mailbox), and remove it with
`flatpak uninstall --user io.github.qubernetic.inboxcraft`.

### Install the bundle from CI
The CI bundle job (#32) builds the same manifest into a single-file `.flatpak` bundle on every ready PR run and
keeps it as an artifact of the run. Download it from the run's page, or with `gh run download <run-id>`, then
install it for your user:

```sh
flatpak install --user <bundle>.flatpak
```

The bundle needs the same runtime; `flatpak install` offers to fetch it from Flathub. Until that job lands, build
locally.

## Live tests
The owner alone runs opt-in live tests, by hand, on two throwaway test accounts. Agents write and maintain these
tests but never run them and never hold credentials for any account; every other test uses the in-process fake
servers and `example.org` addresses.

- **Where:** `crates/testkit/tests/live.rs` (the harness) and `crates/net/tests/live.rs`: both accounts sign in and
  list their folders; the `live-imap` round trip (sign-in, folder list, run folder, APPEND of an invented message,
  first sync, IDLE, resync, folder removed); the account check, whose every IMAP step must pass on the real chain.
  All of them over implicit TLS on port 993. Every live test is named `live_*` and starts with
  `inboxcraft_testkit::live::start`, which returns the configuration or prints `SKIP <test>: <reason>` and lets the
  test return early. They skip cleanly without credentials, and always when `AGENT_HORDE` or `CI` is set, so
  `just test` and `just ci` run them as skipped tests with the reason in the output.
- **Accounts:** `live-imap` and `live-imap2`, two throwaway IMAP and SMTP accounts on the reference provider. The
  harness's own types name them only by these names: their debug and error output never shows an address, a login
  name or a secret. Values a test prints itself, or passes to an `assert_eq!` that fails, are not redacted; a live
  test never does either.
- **Allow-list:** `LiveConfig::check_recipient` admits exactly the two addresses given at run time, each a plain
  dot-atom address; a live test that sends mail checks every recipient with it first.
- **Run folder:** `InboxCraft-Test/<run id>` (with the server's hierarchy delimiter), created by
  `RunFolder::create` and removed by `RunFolder::remove`, or on drop when a test fails before it gets there.
  `RunFolder` builds the name itself and refuses a delimiter that could change it, so it deletes no other folder.
  `LiveConfig::invented_message` gives the run's invented messages (from and to `live-test@example.org`); APPEND
  puts them in the run folder and sends them to nobody.
- **Profile:** `nextest --profile live` (`.config/nextest.toml`) runs only the `live_*` tests, one at a time.

### Running them (owner)
`just it-live` runs on the host. It refuses in native mode and when `AGENT_HORDE` or `CI` is set. It works in two
steps:

1. It makes `target/nextest` and builds the live test binaries (`testkit` and `net`) into an archive,
   `target/nextest/live.tar.zst`, before it reads any value, so no build script, proc macro or other test binary
   ever runs with the credentials.
2. It reads eight values from the OS credential store, service `inboxcraft-live`, one entry per variable, and runs
   the `live` profile from the archive, which never compiles. The values reach the container through its own
   environment for that run only, never on a command line or in a file.

Extra arguments are test name filters for nextest; options (anything starting with `-`) are refused, so nothing
can widen the run beyond the archived live tests.

After the run it scans the output for the two secrets, the two addresses and the two login names, each as it is and
as Rust's `{:?}` prints it, and fails the run when one appears, naming only the variable. The scan runs after the
output is already on the screen and in the terminal's scrollback: it reports a leak, it does not hide it. Check the
output yourself before you paste it anywhere, such as into an issue.

| Variable | Meaning |
|---|---|
| `INBOXCRAFT_LIVE_IMAP_ADDRESS`, `INBOXCRAFT_LIVE_IMAP2_ADDRESS` | the accounts' addresses: the recipient allow-list |
| `INBOXCRAFT_LIVE_IMAP_HOST`, `INBOXCRAFT_LIVE_IMAP2_HOST` | the servers' host names |
| `INBOXCRAFT_LIVE_IMAP_USER`, `INBOXCRAFT_LIVE_IMAP2_USER` | the login names |
| `INBOXCRAFT_LIVE_IMAP_SECRET`, `INBOXCRAFT_LIVE_IMAP2_SECRET` | the passwords or tokens |
| `INBOXCRAFT_LIVE_RUN_ID` | set by `just it-live`: UTC time and process id |

Storing a value (it prompts for it, so it never lands in the shell history):

```sh
secret-tool store --label "InboxCraft live INBOXCRAFT_LIVE_IMAP_SECRET" \
    service inboxcraft-live variable INBOXCRAFT_LIVE_IMAP_SECRET          # Linux, Secret Service
security add-generic-password -s inboxcraft-live -a INBOXCRAFT_LIVE_IMAP_SECRET -w   # macOS, Keychain
```

Use only throwaway accounts made for these tests, never a personal one.

## CI
`.github/workflows/ci.yml` runs `cargo xtask ci`, the same gates in the same order as `just ci`, on pull requests
into `develop`, `main` and `epic/**`, on pushes to those branches, and on demand (`gh workflow run ci.yml --ref
<branch>`).

| Job | What it does | Token |
|---|---|---|
| `test` | `cargo xtask ci` on `ubuntu-24.04`; uploads `lcov.info` and `target/nextest/ci/junit.xml`, both from the `cov` gate, as the `reports` artifact; restores the dependency cache, and on a push saves it | read |
| `codecov` | uploads the coverage and the test results to Codecov with the job's GitHub OIDC token; runs no repository code | read, `id-token: write` |
| `flatpak` | builds `contrib/flatpak/` into `inboxcraft.flatpak` in Flathub's builder image (privileged, for bubblewrap; no cache) and keeps it as the `flatpak` artifact for 7 days | read |
| `ci` | the gate: succeeds only when `test`, `codecov` and `flatpak` did | read |

- **A draft starts no job.** Every job is skipped on a draft PR, so a push to a draft bills nothing; the draft cannot
  merge anyway, as `ci` is the required check. The full run starts when the PR is marked ready (`gh pr ready`) and
  repeats on every later push; a newer run of the same PR cancels the older one. Run `just ci` locally before
  marking a PR ready.
- `ci` is the check to require. A skipped check reports as passing, so a required check has to be a job that runs on
  every full run.
- `test` installs the Debian packages of the `Dockerfile` (without the ones the runner already has), the toolchain
  from `rust-toolchain.toml`, and nextest, cargo-llvm-cov and cargo-deny at the versions the `Dockerfile` pins: it
  reads them from there, so a bump stays one change.
- **One cache, written only by merged code.** The `test` job caches the compiled third-party dependencies
  (`deps`, `build` and `.fingerprint` of every profile under `target/`) and the cargo registry
  (`~/.cargo/registry/index`, `~/.cargo/registry/cache`, `~/.cargo/git/db`; the unpacked sources are not cached,
  cargo unpacks them again from the `.crate` files). Every full run restores it with `actions/cache/restore`; only a
  `push` run, that is a push to `develop`, `main` or `epic/**`, saves it with `actions/cache/save`, after a green
  run, so a `pull_request` or `workflow_dispatch` run never writes it. Before the save, every package and target
  name of this workspace is deleted from `target/`, with the binaries, reports and coverage data: no artifact of
  our own code is cached. The job sets `CARGO_INCREMENTAL=0`.
- The key is `deps-v1-<os>-<ISO week>-<hash of rust-toolchain.toml>-<hash of the [profile.*] sections>-<hash of
  Cargo.lock>`, so it rotates every week and a toolchain or profile change starts from nothing. A run whose
  `Cargo.lock` has no entry yet restores the newest entry of the same week, toolchain and profiles and rebuilds what
  changed; that entry can carry old dependency versions until the week ends. A PR restores the entries of its base
  branch and of `main`.
- Purge the cache with `gh cache delete --all` (one entry: `gh cache list`, then `gh cache delete <key>`). The
  `codecov` and `flatpak` jobs and `fuzz.yml` use no cache.
- Every action is pinned by its full commit SHA with the tag in a comment; the workflow token is read-only and only
  `codecov` widens it.

`.github/workflows/fuzz.yml` runs every target in `fuzz/fuzz_targets/` with cargo-fuzz for 300 seconds each, nightly
at 02:17 UTC, and for 30 seconds on a PR that touches `fuzz/` or the workflow. cargo-fuzz needs nightly Rust: the
toolchain is pinned by date in the workflow. A crash fails the run and its input is kept as the `fuzz-artifacts`
artifact; it becomes a regression test. With no targets the job passes. GitHub runs a schedule only from the default
branch, `main`, so the nightly run starts with the first release.

`fuzz/` is a Cargo workspace of its own, so libFuzzer and the nightly toolchain never reach the main workspace. A
new parser adds its target as `fuzz/fuzz_targets/<name>.rs` and a `[[bin]]` entry in `fuzz/Cargo.toml`; locally it
runs with `cargo +nightly fuzz run <name>` from the repository root. The targets are `rfc5322_date` (#15),
`charset_decode` and `rfc2047_words` (#16), `model_values` (#17), `mime_message`, `mime_headers` and `mime_dsn`
(#18), `imap_client` (#20), `testkit_imap` (#23), the fake IMAP server's own command reader, `net_envelope`
(#25), the envelope of a header block, `engine_params` (#27), and `net_certificate`, `net_autoconfig` and `net_dns`
(#28): a server's certificate, an autoconfig file, and DNS answers and resolver configurations, and
`html_display` (#29), the safe HTML display.

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
| 2026-11-04 | minor | Live tests: `just it-live`, the nextest `live` profile and `testkit::live` (#10) |
| 2026-11-04 | minor | Live tests: build before reading the credentials, the wider leak scan, what is not redacted (#10) |
| 2026-11-04 | minor | Fuzzing: the `fuzz/` workspace and how a target is added (#15) |
| 2026-11-04 | minor | Fuzzing: the charset and encoded word targets (#16) |
| 2026-11-04 | minor | Fuzzing: the model values target (#17) |
| 2026-11-04 | minor | Fuzzing: the message, header and delivery report targets (#18) |
| 2026-11-04 | minor | Fuzzing: the IMAP client target (#20) |
| 2026-11-04 | minor | The layering rule that keeps `proto` out of `testkit` (#23) |
| 2026-11-04 | minor | Fuzzing: the fake IMAP server target (#23) |
| 2026-11-04 | minor | The layering rule that keeps `rcgen` and `flate2` in `testkit` (#23) |
| 2026-11-04 | minor | `store` may depend on `model` inside layer 1 (#24) |
| 2026-11-05 | minor | Fuzzing: the envelope target of `net` (#25) |
| 2026-11-05 | minor | `flate2` moves from the testkit-only to the net-only crates (#26) |
| 2026-11-05 | minor | Live tests: the `net` round trip on `live-imap`, invented messages (#26) |
| 2026-11-05 | minor | Fuzzing: the certificate, autoconfig and DNS targets of `net`; live test of the account check (#28) |
| 2026-11-05 | minor | Live tests: `just it-live` makes the archive's folder first (#57) |
| 2026-11-05 | minor | Live tests: the second account is `live-imap2` on the reference provider (#57) |
| 2026-11-05 | minor | Live tests: both accounts sign in and list their folders (#57) |
| 2026-11-05 | minor | Fuzzing: the safe HTML display target (#29) |
| 2026-11-05 | minor | The `shot` recipe; the desktop app opens the window (#30) |
| 2026-11-05 | minor | The dev Flatpak: `just flatpak`, the CI bundle, `flatpak-sources-check`, `just icon` (#31) |
| 2026-11-05 | minor | CI: the `flatpak` bundle job (#32) |
| 2026-11-05 | minor | The Flatpak opens the Downloads folder for Save (#33) |
| 2026-11-05 | minor | CI: the `cov` gate is the one test run, no separate nextest gate (#66) |
| 2026-11-05 | minor | CI: the dependency cache of the `test` job, saved by branch pushes only (#67) |
| 2026-11-05 | minor | CI: no `draft` job, a draft starts no job (#68) |
