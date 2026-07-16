#!/usr/bin/env bash
# Cross-compile pagemd release binaries for Linux / Windows / macOS.
# Linux targets use cargo-zigbuild with glibc 2.17 for broad distro support.
# Zip naming: pagemd-{os}-{arch}-{version}.zip
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DIST_DIR="${DIST_DIR:-dist}"
GLIBC="${PAGEMD_LINUX_GLIBC:-2.17}"
VERSION="$(cargo metadata --no-deps --format-version 1 | sed -n 's/.*"name":"pagemd","version":"\([^"]*\)".*/\1/p' | head -n1)"
if [[ -z "${VERSION}" ]]; then
  VERSION="$(grep -E '^version\s*=' Cargo.toml | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')"
fi

HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"

mkdir -p "${DIST_DIR}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: missing required command: $1" >&2
    exit 1
  fi
}

need_cmd cargo
need_cmd rustup
need_cmd zig
need_cmd cargo-zigbuild

if ! command -v zip >/dev/null 2>&1; then
  echo "error: missing required command: zip" >&2
  exit 1
fi

ensure_target() {
  local triple="$1"
  if ! rustc --print target-list | grep -qx "${triple}"; then
    echo "error: rustc has no built-in target ${triple}" >&2
    return 1
  fi
  if ! rustup target list --installed | grep -qx "${triple}"; then
    echo "==> rustup target add ${triple}"
    rustup target add "${triple}"
  fi
}

# Package binary into dist/pagemd-{os}-{arch}-{version}.zip
package_zip() {
  local rust_triple="$1"
  local os_name="$2"
  local arch_name="$3"
  local src_bin="$4"

  local zip_name="pagemd-${os_name}-${arch_name}-${VERSION}.zip"
  local stage="${DIST_DIR}/.stage-${os_name}-${arch_name}"
  local bin_name="pagemd"
  if [[ "${os_name}" == "windows" ]]; then
    bin_name="pagemd.exe"
  fi

  rm -rf "${stage}"
  mkdir -p "${stage}"
  cp "${src_bin}" "${stage}/${bin_name}"

  rm -f "${DIST_DIR}/${zip_name}"
  (
    cd "${stage}"
    zip -qr "${ROOT}/${DIST_DIR}/${zip_name}" "${bin_name}"
  )
  rm -rf "${stage}"
  echo "    -> ${DIST_DIR}/${zip_name}"
}

# Build with zig for cross / old-glibc targets.
zig_build() {
  local rust_triple="$1"
  local zig_target="$2"
  local os_name="$3"
  local arch_name="$4"

  ensure_target "${rust_triple}" || return 1
  echo "==> cargo zigbuild --release --target ${zig_target}"
  cargo zigbuild --release --target "${zig_target}" || return 1

  local bin="target/${rust_triple}/release/pagemd"
  if [[ "${rust_triple}" == *"-windows-"* ]]; then
    bin="${bin}.exe"
  fi
  if [[ ! -f "${bin}" ]]; then
    echo "error: missing binary ${bin}" >&2
    return 1
  fi

  package_zip "${rust_triple}" "${os_name}" "${arch_name}" "${bin}"
}

# Native Apple build (no Zig glibc suffix needed).
apple_build() {
  local rust_triple="$1"
  local arch_name="$2"

  ensure_target "${rust_triple}" || return 1
  echo "==> cargo build --release --target ${rust_triple}"
  cargo build --release --target "${rust_triple}" || return 1

  local bin="target/${rust_triple}/release/pagemd"
  package_zip "${rust_triple}" "macos" "${arch_name}" "${bin}"
}

echo "PageMD ${VERSION} → ${DIST_DIR}/ (host ${HOST_OS}/${HOST_ARCH}, linux glibc ${GLIBC})"
echo "Zip naming: pagemd-{os}-{arch}-{version}.zip"

# --- Linux (glibc ${GLIBC}) via zigbuild ---
zig_build \
  "x86_64-unknown-linux-gnu" \
  "x86_64-unknown-linux-gnu.${GLIBC}" \
  "linux" \
  "x64"

zig_build \
  "aarch64-unknown-linux-gnu" \
  "aarch64-unknown-linux-gnu.${GLIBC}" \
  "linux" \
  "arm64"

# --- Windows (gnu) via zigbuild ---
# Only x86_64-pc-windows-gnu is a built-in rustc target; aarch64-pc-windows-gnu is not.
zig_build \
  "x86_64-pc-windows-gnu" \
  "x86_64-pc-windows-gnu" \
  "windows" \
  "x64"

# --- macOS ---
# Prefer native cargo on macOS hosts (SDK available). From Linux, zigbuild needs a macOS SDK.
if [[ "${HOST_OS}" == "Darwin" ]]; then
  apple_build "x86_64-apple-darwin" "x64"
  apple_build "aarch64-apple-darwin" "arm64"

  # Optional universal binary when lipo is available.
  if command -v lipo >/dev/null 2>&1; then
    echo "==> lipo macos-universal"
    univ_bin="${DIST_DIR}/.pagemd-universal"
    lipo -create \
      -output "${univ_bin}" \
      "target/x86_64-apple-darwin/release/pagemd" \
      "target/aarch64-apple-darwin/release/pagemd"
    package_zip "universal2-apple-darwin" "macos" "universal" "${univ_bin}"
    rm -f "${univ_bin}"
  fi
else
  echo "note: host is not macOS; attempting zigbuild for Apple targets (requires macOS SDK / SDKROOT)"
  if [[ -n "${SDKROOT:-}" ]]; then
    zig_build "x86_64-apple-darwin" "x86_64-apple-darwin" "macos" "x64" \
      || echo "warning: skipped macos-x64" >&2
    zig_build "aarch64-apple-darwin" "aarch64-apple-darwin" "macos" "arm64" \
      || echo "warning: skipped macos-arm64" >&2
  else
    echo "warning: SDKROOT unset; skipping macOS targets" >&2
  fi
fi

# Drop temporary stage dirs if any remain.
rm -rf "${DIST_DIR}"/.stage-* "${DIST_DIR}"/*-unknown-* "${DIST_DIR}"/*-pc-* "${DIST_DIR}"/*-apple-* 2>/dev/null || true

echo
echo "Done. Artifacts:"
ls -la "${DIST_DIR}"/pagemd-*-*.zip 2>/dev/null | sed 's/^/  /' || ls -la "${DIST_DIR}" | sed 's/^/  /'
