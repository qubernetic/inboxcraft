# Development image for InboxCraft (two-layer dev model, docs/development.md).
# Everything that needs Rust runs here; the host only runs the built binary.
# The Rust version here and in rust-toolchain.toml are bumped together.
# The base image is pinned by its multi-arch index digest; a tag bump also
# moves the digest. Debian trixie's glibc is older than current desktop
# distributions', so binaries built here also run on the host.
FROM rust:1.98.0-slim-trixie@sha256:17d1ba895198f9934c6314ec5346a0d5115372f3243390c3d731e242f35c2f27

ARG UID=1000
ARG GID=1000
# Every downloaded artifact is checked against a sha256 committed here; a
# version bump updates the version and both per-architecture checksums.
ARG CARGO_BINSTALL_VERSION=1.25.3
ARG CARGO_BINSTALL_SHA256_X86_64=1f72931713e9091478538c6c78c3f11f205b4f6f394f35e61835036b23929d2a
ARG CARGO_BINSTALL_SHA256_AARCH64=19ef7a058397f83d6b2cc83727fb8d38babd8ad657210c921b7771a9e8b3a6fb
ARG SCCACHE_VERSION=0.19.0
ARG SCCACHE_SHA256_X86_64=aa73c7270c997894038c0d8f0d297865f57569e1687883322bc431713d4bb9dd
ARG SCCACHE_SHA256_AARCH64=3e47fc4a71b1ddd1b4294a0de9d34498922fdf41e3cd0bda5df3954e3d6deb9d
ARG JUST_VERSION=1.53.0
ARG CARGO_DENY_VERSION=0.20.2
ARG CARGO_LLVM_COV_VERSION=0.9.1
# Must satisfy nextest-version in .config/nextest.toml.
ARG CARGO_NEXTEST_VERSION=0.9.150

ENV DEBIAN_FRONTEND=noninteractive

# Debian packages come unpinned from the digest-pinned base image's trixie
# repository. From build-essential on, the list is the egui/winit build
# prerequisites the Craft CI gate installs, plus libdbus-1-dev for the
# Secret Service client.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        pkg-config \
        build-essential \
        libxkbcommon-dev \
        libwayland-dev \
        libx11-dev \
        libxcursor-dev \
        libxrandr-dev \
        libxi-dev \
        libgtk-3-dev \
        libfontconfig1-dev \
        libfreetype-dev \
        libdbus-1-dev \
        mesa-vulkan-drivers \
        libvulkan1 \
    && rm -rf /var/lib/apt/lists/*

# Toolchain components and targets the workspace expects (rust-toolchain.toml).
RUN rustup component add rustfmt clippy llvm-tools-preview \
    && rustup target add wasm32-unknown-unknown

# cargo-binstall from its pinned release archive, verified before it is
# extracted. No install script, nothing from a branch ref.
RUN set -eu; \
    case "$(uname -m)" in \
        x86_64) sha="${CARGO_BINSTALL_SHA256_X86_64}" ;; \
        aarch64) sha="${CARGO_BINSTALL_SHA256_AARCH64}" ;; \
        *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;; \
    esac; \
    archive="cargo-binstall-$(uname -m)-unknown-linux-musl.tgz"; \
    curl -fsSL --proto '=https' --tlsv1.2 -o "/tmp/${archive}" \
        "https://github.com/cargo-bins/cargo-binstall/releases/download/v${CARGO_BINSTALL_VERSION}/${archive}"; \
    echo "${sha}  /tmp/${archive}" | sha256sum -c -; \
    tar -xzf "/tmp/${archive}" -C /usr/local/cargo/bin cargo-binstall; \
    rm -f "/tmp/${archive}"; \
    cargo binstall -V

# Cargo tools as prebuilt binaries from their own release assets; versions are
# pinned. The third-party quick-install mirror and source builds are disabled.
RUN cargo binstall --no-confirm --locked \
        --disable-strategies quick-install,compile \
        "just@${JUST_VERSION}" \
        "cargo-deny@${CARGO_DENY_VERSION}" \
        "cargo-llvm-cov@${CARGO_LLVM_COV_VERSION}" \
        "cargo-nextest@${CARGO_NEXTEST_VERSION}" \
    && rm -rf /usr/local/cargo/registry/* /usr/local/cargo/git/*

# sccache from its pinned release archive, verified before it is extracted.
# It lets the target dirs of several worktrees share the compiled third-party
# dependencies. The wrapper drops CARGO_TARGET_DIR and CARGO_BUILD_TARGET_DIR
# first: sccache hashes every CARGO_* variable, so with them in the key no two
# target dirs would ever share an entry (rustc gets its output dir as an
# argument and does not need them). It runs rustc without sccache when
# CARGO_INCREMENTAL is on, which sccache refuses.
RUN set -eu; \
    case "$(uname -m)" in \
        x86_64) sha="${SCCACHE_SHA256_X86_64}" ;; \
        aarch64) sha="${SCCACHE_SHA256_AARCH64}" ;; \
        *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;; \
    esac; \
    name="sccache-v${SCCACHE_VERSION}-$(uname -m)-unknown-linux-musl"; \
    curl -fsSL --proto '=https' --tlsv1.2 -o "/tmp/${name}.tar.gz" \
        "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/${name}.tar.gz"; \
    echo "${sha}  /tmp/${name}.tar.gz" | sha256sum -c -; \
    tar -xzf "/tmp/${name}.tar.gz" -C /usr/local/cargo/bin --strip-components=1 "${name}/sccache"; \
    rm -f "/tmp/${name}.tar.gz"; \
    printf '%s\n' \
        '#!/bin/bash' \
        '# bash, not sh: dash drops variables whose names are not shell names,' \
        '# such as CARGO_BIN_EXE_inboxcraft-web, before rustc could read them.' \
        '# sccache hashes every CARGO_* variable; the target dir must not be part of the key.' \
        'unset CARGO_TARGET_DIR CARGO_BUILD_TARGET_DIR' \
        '# sccache refuses to run with incremental builds forced on (the coverage run).' \
        'case "${CARGO_INCREMENTAL:-0}" in' \
        '    0) exec sccache "$@" ;;' \
        '    *) exec "$@" ;;' \
        'esac' \
        > /usr/local/cargo/bin/sccache-wrap; \
    chmod 0755 /usr/local/cargo/bin/sccache /usr/local/cargo/bin/sccache-wrap; \
    sccache --version

# Non-root user matching the host UID/GID so bind-mounted files keep their owner.
RUN set -eu; \
    if ! getent group "${GID}" >/dev/null; then groupadd -g "${GID}" dev; fi; \
    useradd -m -u "${UID}" -g "${GID}" -s /bin/bash dev; \
    mkdir -p /usr/local/cargo/registry /usr/local/cargo/git /workspace; \
    chown -R "${UID}:${GID}" /usr/local/cargo /usr/local/rustup /workspace; \
    git config --system --add safe.directory /workspace

USER dev
WORKDIR /workspace

# Every rustc call in the container goes through sccache (set here, never in
# .cargo/config.toml: native mode and the GitHub workflow build without it).
# The cache lies under target/ on the bind mount, so it outlives the container
# and needs no volume of its own; `just clean` deletes it with target/.
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache-wrap \
    SCCACHE_DIR=/workspace/target/sccache \
    SCCACHE_CACHE_SIZE=40G
