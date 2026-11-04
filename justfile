# InboxCraft task runner (two-layer dev model, docs/development.md).
# A thin layer over `cargo xtask`: `just ci` is `cargo xtask ci`, run in the
# `rust` compose service. `run` and `run-sample` execute the container-built
# binary on the host, where the display, tray, notifications and Secret
# Service are.
# Podman users: `COMPOSE_CMD="podman compose" just ...` or put it in `.env`.
# Native mode: with AGENT_HORDE=1 the check and build recipes run the toolchain
# directly (for a shell that already is the container, such as the agent
# sandbox or `just shell`); the lifecycle and host recipes refuse.

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := true
set positional-arguments := true

compose := env("COMPOSE_CMD", "docker compose")
service := "rust"
uid     := `id -u`
gid     := `id -g`
env     := "INBOXCRAFT_UID=" + uid + " INBOXCRAFT_GID=" + gid
dc      := env + " " + compose
native  := env("AGENT_HORDE", "")
exec    := if native == "1" { "" } else { dc + " exec " + service }

# List recipes
default:
    @just --list --unsorted

# ---- Lifecycle -------------------------------------------------------------

# First-time setup: build the image, start the container, fetch dependencies
setup: _container_only
    {{ dc }} build
    {{ dc }} up -d
    {{ exec }} cargo fetch

# Start the dev container (detached)
dev: _container_only
    {{ dc }} up -d

# Stop the container (keep volumes and target/)
down: _container_only
    {{ dc }} down

# Stop the container, remove volumes, delete target/
clean: _container_only
    {{ dc }} down -v
    rm -rf target

# Rebuild the image without cache (after Dockerfile changes)
rebuild: _container_only
    {{ dc }} build --no-cache

# Interactive shell in the container
shell: _container_only _up
    {{ exec }} bash

# ---- Build and checks (container) -----------------------------------------

# Build the workspace (profile: dev or release); binaries land in target/
build profile="dev": _up
    {{ exec }} cargo build --workspace --profile {{ profile }}

# Run the test suite with cargo-nextest
test *args: _up
    {{ exec }} cargo nextest run --workspace "$@"

# Clippy on all targets, warnings are errors
lint: _up
    {{ exec }} cargo clippy --workspace --all-targets -- -D warnings

# Format the workspace
fmt: _up
    {{ exec }} cargo fmt --all

# Check formatting without changing files
fmt-check: _up
    {{ exec }} cargo fmt --all -- --check

# Licences, advisories, bans and sources (deny.toml)
deny: _up
    {{ exec }} cargo deny check

# Coverage per crate under cargo llvm-cov nextest, lcov in lcov.info; `just cov --check` fails below the floors
cov *args: _up
    {{ exec }} cargo xtask cov "$@"

# Layering rules between the crates (xtask/src/layers.rs)
layers: _up
    {{ exec }} cargo xtask layers

# Build L0–L5 for wasm32-unknown-unknown
wasm: _up
    {{ exec }} cargo xtask wasm

# Every gate CI runs: cargo xtask ci (fmt, clippy, tests, doctests, assets, spdx, test-files, layers, wasm, deny)
ci: _up
    {{ exec }} cargo xtask ci

# ---- Host (display, tray, notifications, Secret Service) -------------------

# Run the container-built app on the host: just run -- --control 7981 (profile: debug or release)
run *args: _host_only
    @dir="${INBOXCRAFT_PROFILE:-debug}"; bin="target/$dir/inboxcraft"; \
    [ -x "$bin" ] || { echo "$bin is missing: run 'just build' first" >&2; exit 1; }; \
    [ "${1:-}" = "--" ] && shift; exec "$bin" "$@"

# Run the container-built app on the host with the invented sample mailbox
run-sample profile="debug": _host_only
    @bin="target/{{ profile }}/inboxcraft"; \
    [ -x "$bin" ] || { echo "$bin is missing: run 'just build' first" >&2; exit 1; }; \
    exec "$bin" --sample

# ---- Helpers ---------------------------------------------------------------

# Start the container if it is not running (no-op in native mode)
[private]
_up:
    @if [ "{{ native }}" != "1" ] && [ -z "$({{ dc }} ps --status running -q {{ service }})" ]; then {{ dc }} up -d; fi

# Refuse the container lifecycle recipes in native mode
[private]
_container_only:
    @if [ "{{ native }}" = "1" ]; then echo "not applicable in native mode (AGENT_HORDE=1): no container to manage" >&2; exit 1; fi

# Refuse the host recipes in native mode
[private]
_host_only:
    @if [ "{{ native }}" = "1" ]; then echo "not applicable in native mode (AGENT_HORDE=1): host recipes run on the owner's host" >&2; exit 1; fi
