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

if [ -d "${HOME}/.cargo/bin" ]; then
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi
if [ -d "${HOME}/.rustup/toolchains/stable-aarch64-apple-darwin/bin" ]; then
  export PATH="${HOME}/.rustup/toolchains/stable-aarch64-apple-darwin/bin:${PATH}"
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required to build taskmanager-core and run UniFFI bindgen" >&2
  exit 1
fi

cargo build -p taskmanager-core

RUNNER_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "${RUNNER_DIR}"
}
trap cleanup EXIT
mkdir -p "${RUNNER_DIR}/src"
cat > "${RUNNER_DIR}/Cargo.toml" <<'RUNNER_TOML'
[package]
name = "tsk-uniffi-bindgen-runner"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
uniffi_bindgen = "=0.31.1"
RUNNER_TOML
cat > "${RUNNER_DIR}/src/main.rs" <<'RUNNER_RS'
use std::env;
use uniffi_bindgen::bindings::{generate, GenerateOptions, TargetLanguage};

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    let source = args.next().expect("source UDL path");
    let out_dir = args.next().expect("output directory");
    generate(GenerateOptions {
        languages: vec![TargetLanguage::Swift],
        source: source.into(),
        out_dir: out_dir.into(),
        format: true,
        ..GenerateOptions::default()
    })
}
RUNNER_RS

cargo run --manifest-path "${RUNNER_DIR}/Cargo.toml" -- "${UDL_FILE}" "${GENERATED_DIR}"

# SwiftPM system library targets require the module map to be named module.modulemap.
cp "${GENERATED_DIR}/${LIB_NAME}FFI.modulemap" "${GENERATED_DIR}/module.modulemap"

echo "Generated UniFFI Swift bindings in ${GENERATED_DIR}"
