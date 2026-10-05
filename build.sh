#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
FRONTEND_DIR="$ROOT_DIR/frontend"
BACKEND_DIR="$ROOT_DIR/backend"
DESKTOP_MANIFEST="$BACKEND_DIR/desktop/Cargo.toml"
DESKTOP_TARGET="$BACKEND_DIR/desktop/target"
BUILD_ID="$(date +%Y%m%d-%H%M%S)"
OUTPUT_DIR="$ROOT_DIR/dist/build-$BUILD_ID"

for command_name in cargo npm python3 tar sha256sum; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf 'missing required command: %s\n' "$command_name" >&2
        exit 1
    fi
done

if [[ "$(uname -s)" != "Linux" ]]; then
    printf 'this build script currently targets Linux\n' >&2
    exit 1
fi

mkdir -p "$OUTPUT_DIR"

printf '%s\n' '[1/5] building frontend'
npm --prefix "$FRONTEND_DIR" run build

printf '%s\n' '[2/5] invalidating the desktop release crate'
# Tauri build scripts may not notice changed frontend assets on their own.
cargo clean -p akanetease-desktop --release \
    --manifest-path "$DESKTOP_MANIFEST" \
    --target-dir "$DESKTOP_TARGET"

printf '%s\n' '[3/5] building and packaging desktop'
python3 "$BACKEND_DIR/scripts/package_linux.py" \
    --offline \
    --output "$OUTPUT_DIR"

ARCHIVE="$(find "$OUTPUT_DIR" -maxdepth 1 -type f -name '*.tar.gz' -print -quit)"
CHECKSUM="${ARCHIVE}.sha256"
if [[ -z "$ARCHIVE" || ! -f "$CHECKSUM" ]]; then
    printf 'packager did not produce an archive and checksum\n' >&2
    exit 1
fi

printf '%s\n' '[4/5] verifying and extracting package'
(cd "$(dirname "$ARCHIVE")" && sha256sum -c "$(basename "$CHECKSUM")")
tar -xzf "$ARCHIVE" -C "$OUTPUT_DIR"

BINARY="$(find "$OUTPUT_DIR" -type f -path '*/usr/bin/akanetease-desktop' -print -quit)"
if [[ -z "$BINARY" || ! -x "$BINARY" ]]; then
    printf 'extracted package does not contain an executable desktop binary\n' >&2
    exit 1
fi

printf '%s\n' '[5/5] running isolated startup check'
python3 "$BACKEND_DIR/scripts/package_smoke.py" "$BINARY"

printf '\nBuild complete.\n'
printf 'Archive: %s\n' "$ARCHIVE"
printf 'Run: %s\n' "$BINARY"
