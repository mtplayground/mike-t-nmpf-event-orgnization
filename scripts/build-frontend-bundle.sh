#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND_DIR="${ROOT_DIR}/frontend"
TARGET_DIR="${TARGET_DIR:-${ROOT_DIR}/dist/frontend}"

if ! command -v npm >/dev/null 2>&1; then
  cat >&2 <<'EOF'
npm is not on PATH.
Install Node.js and npm before running this script.
EOF
  exit 1
fi

if [[ -z "${VITE_API_BASE_URL:-}" ]]; then
  cat >&2 <<'EOF'
VITE_API_BASE_URL is not set.
Set it to the public API origin before building, for example:
  export VITE_API_BASE_URL=https://events.example.com/api
EOF
  exit 1
fi

cd "${FRONTEND_DIR}"
npm ci
npm run build

rm -rf "${TARGET_DIR}"
mkdir -p "${TARGET_DIR}"
cp -R "${FRONTEND_DIR}/dist/." "${TARGET_DIR}/"

printf 'Frontend bundle written to %s\n' "${TARGET_DIR}"
