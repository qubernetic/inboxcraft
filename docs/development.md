# Development setup

> **Last reviewed:** 2026-11-03 · **Last updated:** 2026-11-03 · **Change:** major

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
tools pinned there. Today it carries only Go, Rust, just and uv, so `AGENT_HORDE=1 just ci` does not build the
desktop crates in it yet.

## Bumping a pinned version
- **Rust:** change `channel` in `rust-toolchain.toml` and the base image tag in the `Dockerfile` together; a new tag
  also needs its new multi-arch index digest.
- **Downloaded tools:** every download in the `Dockerfile` is checked against a sha256 committed next to its version;
  a bump updates the version and both per-architecture checksums.
- **cargo-nextest:** keep `CARGO_NEXTEST_VERSION` at or above `nextest-version` in `.config/nextest.toml`.

After a `Dockerfile` change, run `just rebuild`.

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-11-03 | major | First version: the two-layer model, the justfile, native mode (#1) |
