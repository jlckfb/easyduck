# Linux x86-64 host, ARM64 target. Build from the repository root.
ARG UBUNTU_IMAGE=ubuntu:22.04@sha256:4f838adc7181d9039ac795a7d0aba05a9bd9ecd480d294483169c5def983b64d
FROM ${UBUNTU_IMAGE}
ARG RUST_VERSION=1.98.1
ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_HOME=/root/.cargo \
    RUSTUP_HOME=/root/.rustup \
    PATH=/root/.cargo/bin:${PATH}

# Keep native and target packages separate while using Ubuntu's matching glibc.
RUN test "$(dpkg --print-architecture)" = amd64 \
    && dpkg --add-architecture arm64 \
    && sed -i 's/^deb /deb [arch=amd64] /' /etc/apt/sources.list \
    && printf '%s\n' \
       'deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy main universe' \
       'deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy-updates main universe' \
       'deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy-security main universe' \
       > /etc/apt/sources.list.d/arm64.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
       ca-certificates curl build-essential gcc-aarch64-linux-gnu \
       libc6-dev-arm64-cross pkg-config libudev-dev:amd64 libudev-dev:arm64 \
       python3 file \
    && rm -rf /var/lib/apt/lists/* \
    && ln -s / /opt/gst-sysroot

ARG RUSTUP_DIST_SERVER=https://static.rust-lang.org
ARG RUSTUP_UPDATE_ROOT=https://static.rust-lang.org/rustup
RUN curl --proto '=https' --tlsv1.2 --fail --show-error --silent \
       https://sh.rustup.rs -o /tmp/easyduck-rustup.sh \
    && RUSTUP_DIST_SERVER="${RUSTUP_DIST_SERVER}" RUSTUP_UPDATE_ROOT="${RUSTUP_UPDATE_ROOT}" \
       sh /tmp/easyduck-rustup.sh -y --no-modify-path --profile minimal \
       --default-toolchain "${RUST_VERSION}" --target aarch64-unknown-linux-gnu \
    && rm /tmp/easyduck-rustup.sh

# .dockerignore permits only Cargo manifests and the lock file.
# Placeholder targets let Cargo resolve the real workspace without copying source,
# calibration data, credentials, models or local tests into the toolchain image.
COPY runtime/ /tmp/easyduck-dependencies/
RUN test -z "$(find /tmp/easyduck-dependencies -type f ! -name Cargo.toml ! -name Cargo.lock -print)" \
    && for manifest in /tmp/easyduck-dependencies/*/Cargo.toml; do \
       mkdir -p "$(dirname "$manifest")/src"; \
       printf '%s\n' '// Dependency-fetch placeholder.' > "$(dirname "$manifest")/src/lib.rs"; \
    done \
    && cd /tmp/easyduck-dependencies \
    && cargo fetch --locked --target aarch64-unknown-linux-gnu \
    && cargo fetch --locked --target x86_64-unknown-linux-gnu \
    && cd / \
    && rm -rf /tmp/easyduck-dependencies

WORKDIR /work
CMD ["sleep", "infinity"]
