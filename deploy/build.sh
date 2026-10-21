#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTAINER="${EASYDUCK_BUILD_CONTAINER:-easyduck-build}"
OUTPUT="${EASYDUCK_OUTPUT_DIR:-$ROOT/build}"
CACHE="${EASYDUCK_CARGO_CACHE:-/tmp/easyduck-target}"
SYSROOT="${EASYDUCK_SYSROOT:-/opt/gst-sysroot}"
CARGO="${EASYDUCK_CARGO:-/root/.cargo/bin/cargo}"
CROSS_CC="${EASYDUCK_CROSS_CC:-aarch64-linux-gnu-gcc}"
NAMES=(robotd padd robotctl sounds)

mkdir -p "$OUTPUT/bin" "$OUTPUT/logs"
STAGE="$(docker exec "$CONTAINER" mktemp -d /tmp/easyduck-build.XXXXXXXX)"
case "$STAGE" in
    /tmp/easyduck-build.*) ;;
    *) printf 'Unexpected build directory: %s\n' "$STAGE" >&2; exit 1 ;;
esac
cleanup() {
    docker exec "$CONTAINER" rm -rf -- "$STAGE" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Each build receives a fresh source tree; compiled dependencies use the shared cache.
tar -C "$ROOT/runtime" --exclude=target --exclude=.git --exclude=tests \
    --exclude=__pycache__ --exclude='*.pyc' --exclude='.env*' \
    --exclude='*.key' --exclude='*.pem' -cf - . \
    | docker exec -i "$CONTAINER" tar -xf - -C "$STAGE"

docker exec -w "$STAGE" -e CARGO_TARGET_DIR="$CACHE" \
    -e CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="$CROSS_CC" \
    -e CC_aarch64_unknown_linux_gnu="$CROSS_CC" \
    -e PKG_CONFIG_ALLOW_CROSS=1 -e PKG_CONFIG_SYSROOT_DIR="$SYSROOT" \
    -e PKG_CONFIG_PATH="$SYSROOT/usr/lib/aarch64-linux-gnu/pkgconfig" \
    "$CONTAINER" "$CARGO" build --locked --offline --release \
    --target aarch64-unknown-linux-gnu --workspace --bins \
    2>&1 | tee "$OUTPUT/logs/arm-build.log"

for name in "${NAMES[@]}"; do
    docker cp "$CONTAINER:$CACHE/aarch64-unknown-linux-gnu/release/$name" "$OUTPUT/bin/$name"
done
(
    cd "$OUTPUT"
    sha256sum bin/robotd bin/padd bin/robotctl bin/sounds > binaries-SHA256SUMS
    file bin/robotd bin/padd bin/robotctl bin/sounds > binary-types.txt
)
