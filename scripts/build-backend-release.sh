#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${TARGET_DIR:-${ROOT_DIR}/dist/backend}"
BIN_NAME="event-organization-api"

if ! command -v cargo >/dev/null 2>&1; then
  cat >&2 <<'EOF'
cargo is not on PATH.
Install Rust or export the toolchain path before running this script.
EOF
  exit 1
fi

mkdir -p "${TARGET_DIR}"

cargo build \
  --release \
  --bin "${BIN_NAME}" \
  --manifest-path "${ROOT_DIR}/backend/Cargo.toml"

install -m 0755 \
  "${ROOT_DIR}/backend/target/release/${BIN_NAME}" \
  "${TARGET_DIR}/${BIN_NAME}"

printf 'Backend release binary written to %s\n' "${TARGET_DIR}/${BIN_NAME}"
