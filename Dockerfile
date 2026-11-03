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

# Non-root user matching the host UID/GID so bind-mounted files keep their owner.
RUN set -eu; \
    if ! getent group "${GID}" >/dev/null; then groupadd -g "${GID}" dev; fi; \
    useradd -m -u "${UID}" -g "${GID}" -s /bin/bash dev; \
    mkdir -p /usr/local/cargo/registry /usr/local/cargo/git /workspace; \
    chown -R "${UID}:${GID}" /usr/local/cargo /usr/local/rustup /workspace; \
    git config --system --add safe.directory /workspace

USER dev
WORKDIR /workspace
