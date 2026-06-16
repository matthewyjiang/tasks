#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IOS_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${IOS_DIR}/../.." && pwd)"
CORE_DIR="${REPO_ROOT}/core"
GENERATED_DIR="${IOS_DIR}/Sources/TskCore/Generated"
FRAMEWORKS_DIR="${IOS_DIR}/Frameworks"
XCFRAMEWORK_PATH="${FRAMEWORKS_DIR}/TaskmanagerCore.xcframework"
UDL_FILE="${CORE_DIR}/uniffi/core.udl"
LIB_NAME="taskmanager_core"
MODULE_NAME="taskmanager_coreFFI"
CARGO_PROFILE="${CARGO_PROFILE:-debug}"

# Cargo (and uniffi's bindgen runner via cargo_metadata) needs to be invoked
# from inside the workspace, regardless of the user's CWD.
cd "${REPO_ROOT}"

RUST_TARGETS=(
  "aarch64-apple-darwin"
  "aarch64-apple-ios"
  "aarch64-apple-ios-sim"
)

# Prefer the rustup-managed Rust toolchain over system/Homebrew Rust. The iOS
# targets are installed for rustup, and Homebrew rustc may not see them.
if [ -d "/opt/homebrew/bin" ]; then
  export PATH="${PATH}:/opt/homebrew/bin"
fi
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
if ! command -v rustup >/dev/null 2>&1; then
  echo "error: rustup is required to install Apple Rust targets" >&2
  exit 1
fi

# CommandLineTools cannot create xcframeworks; point at a real Xcode if present.
if [ -z "${DEVELOPER_DIR:-}" ] && [ -d "/Applications/Xcode.app/Contents/Developer" ]; then
  export DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"
fi
if ! command -v xcodebuild >/dev/null 2>&1; then
  echo "error: xcodebuild is required to create the XCFramework" >&2
  exit 1
fi

INSTALLED_TARGETS="$(rustup target list --installed)"
for target in "${RUST_TARGETS[@]}"; do
  if ! grep -q "^${target}\$" <<<"${INSTALLED_TARGETS}"; then
    echo "Installing Rust target ${target}..."
    rustup target add "${target}"
  fi
done

STAGING_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "${STAGING_DIR}"
}
trap cleanup EXIT

RUNNER_DIR="${STAGING_DIR}/bindgen-runner"
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

BINDGEN_OUT="${STAGING_DIR}/bindgen"
mkdir -p "${BINDGEN_OUT}"
cargo run --manifest-path "${RUNNER_DIR}/Cargo.toml" -- "${UDL_FILE}" "${BINDGEN_OUT}"

CARGO_BUILD_FLAGS=()
if [ "${CARGO_PROFILE}" = "release" ]; then
  CARGO_BUILD_FLAGS+=("--release")
fi

# Cap concurrency: parallel cross-builds of taskmanager-core (rusqlite/bundled
# pulls in a heavy C compile) have blown past available RAM and panicked the
# kernel. Override with CARGO_BUILD_JOBS=<N> if your machine has more headroom.
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
CARGO_BUILD_FLAGS+=("-j" "${CARGO_BUILD_JOBS}")

for target in "${RUST_TARGETS[@]}"; do
  cargo build -p taskmanager-core --target "${target}" ${CARGO_BUILD_FLAGS[@]+"${CARGO_BUILD_FLAGS[@]}"}
done

# The XCFramework needs the FFI header alongside a `module.modulemap` so that
# `import taskmanager_coreFFI` resolves from Swift.
HEADERS_DIR="${STAGING_DIR}/headers"
mkdir -p "${HEADERS_DIR}"
cp "${BINDGEN_OUT}/${MODULE_NAME}.h" "${HEADERS_DIR}/"
cp "${BINDGEN_OUT}/${MODULE_NAME}.modulemap" "${HEADERS_DIR}/module.modulemap"

mkdir -p "${FRAMEWORKS_DIR}"
rm -rf "${XCFRAMEWORK_PATH}"

XCFRAMEWORK_ARGS=()
for target in "${RUST_TARGETS[@]}"; do
  XCFRAMEWORK_ARGS+=(
    "-library" "${REPO_ROOT}/target/${target}/${CARGO_PROFILE}/lib${LIB_NAME}.a"
    "-headers" "${HEADERS_DIR}"
  )
done
xcodebuild -create-xcframework "${XCFRAMEWORK_ARGS[@]}" -output "${XCFRAMEWORK_PATH}"

mkdir -p "${GENERATED_DIR}"
# The Swift bindings live as a regular source file; the header/modulemap now
# live inside the XCFramework, so drop any older copies from the source tree.
rm -f "${GENERATED_DIR}/${LIB_NAME}.swift"
rm -f "${GENERATED_DIR}/${MODULE_NAME}.h"
rm -f "${GENERATED_DIR}/${MODULE_NAME}.modulemap"
rm -f "${GENERATED_DIR}/module.modulemap"
cp "${BINDGEN_OUT}/${LIB_NAME}.swift" "${GENERATED_DIR}/${LIB_NAME}.swift"

echo "Generated UniFFI Swift bindings in ${GENERATED_DIR}"
echo "Created ${XCFRAMEWORK_PATH}"
