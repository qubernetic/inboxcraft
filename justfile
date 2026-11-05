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

# Every gate CI runs: cargo xtask ci (fmt, clippy, tests, doctests, assets, spdx, test-files, layers, cov, wasm, deny)
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

# Opt-in live tests on the owner's two throwaway test accounts (owner only; docs/development.md, "Live tests")
it-live *args: _host_only _up
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "${AGENT_HORDE:-}" ] || [ -n "${CI:-}" ]; then
        echo "it-live: agents and CI never run live tests" >&2; exit 1
    fi
    for arg in "$@"; do
        case "$arg" in
            -*) echo "it-live: only test name filters pass on to nextest, not options ($arg)" >&2; exit 1 ;;
        esac
    done
    # Build first, before any value is read: no build script, proc macro or other test binary ever sees one.
    archive=target/nextest/live.tar.zst
    # `nextest archive` does not make the archive's folder; on a fresh checkout it would exit with 103.
    {{ dc }} exec {{ service }} mkdir -p target/nextest
    {{ dc }} exec {{ service }} cargo nextest archive -p inboxcraft-testkit -p inboxcraft-net --test live --archive-file "$archive"
    lookup() {
        if command -v secret-tool >/dev/null 2>&1; then
            secret-tool lookup service inboxcraft-live variable "$1"
        elif command -v security >/dev/null 2>&1; then
            security find-generic-password -s inboxcraft-live -a "$1" -w
        else
            echo "it-live: no OS credential store tool found (secret-tool or security)" >&2; return 1
        fi
    }
    vars=()
    for account in IMAP GOOGLE; do
        for field in ADDRESS HOST USER SECRET; do vars+=("INBOXCRAFT_LIVE_${account}_${field}"); done
    done
    pass=()
    for var in "${vars[@]}"; do
        value="$(lookup "$var")" || value=""
        if [ -z "$value" ]; then
            echo "it-live: $var is not in the credential store (service inboxcraft-live)" >&2; exit 1
        fi
        export "$var=$value"
        pass+=(-e "$var")
    done
    INBOXCRAFT_LIVE_RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)-$$"
    export INBOXCRAFT_LIVE_RUN_ID
    pass+=(-e INBOXCRAFT_LIVE_RUN_ID)
    # The values reach the container through this process's environment, never on a command line or in a file,
    # and only the archived live test binary runs with them: a run from an archive never compiles.
    log="$(mktemp)"
    trap 'rm -f "$log"' EXIT
    status=0
    {{ dc }} exec "${pass[@]}" {{ service }} cargo nextest run --archive-file "$archive" --workspace-remap . --profile live "$@" 2>&1 \
        | tee "$log" || status=$?
    # The scan runs after the output is on the screen: it fails the run, it hides nothing. It runs in bash alone,
    # so no value is ever an argument of an external command, and it looks for each value raw and as Rust's
    # `{:?}` prints it (with `\` and `"` escaped).
    output="$(<"$log")"
    for var in INBOXCRAFT_LIVE_IMAP_SECRET INBOXCRAFT_LIVE_GOOGLE_SECRET INBOXCRAFT_LIVE_IMAP_ADDRESS \
        INBOXCRAFT_LIVE_GOOGLE_ADDRESS INBOXCRAFT_LIVE_IMAP_USER INBOXCRAFT_LIVE_GOOGLE_USER; do
        value="${!var}"
        escaped="${value//\\/\\\\}"
        escaped="${escaped//\"/\\\"}"
        if [[ "$output" == *"$value"* || "$output" == *"$escaped"* ]]; then
            echo "it-live: the value of $var appears in the test output" >&2; status=1
        fi
    done
    exit "$status"

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
