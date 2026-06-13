#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IOS_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${IOS_DIR}/../.." && pwd)"
CORE_DIR="${REPO_ROOT}/core"
GENERATED_DIR="${IOS_DIR}/Sources/TskCore/Generated"
UDL_FILE="${CORE_DIR}/uniffi/core.udl"
LIB_NAME="taskmanager_core"

mkdir -p "${GENERATED_DIR}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required to build taskmanager-core" >&2
  exit 1
fi

if ! command -v uniffi-bindgen >/dev/null 2>&1; then
  echo "error: uniffi-bindgen 0.31.1 is required" >&2
  echo "install with: cargo install uniffi_bindgen --version 0.31.1" >&2
  exit 1
fi

cargo build -p taskmanager-core
uniffi-bindgen generate \
  "${UDL_FILE}" \
  --language swift \
  --library "${REPO_ROOT}/target/debug/lib${LIB_NAME}.dylib" \
  --out-dir "${GENERATED_DIR}"

echo "Generated UniFFI Swift bindings in ${GENERATED_DIR}"
