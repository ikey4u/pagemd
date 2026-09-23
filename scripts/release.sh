#!/usr/bin/env bash
# Cross-compile pagemd release zips for macOS / Linux / Windows.
#
# Linker choice:
#   Linux (any arch) → cargo zigbuild, glibc 2.17
#   target OS == host OS → cargo (native SDK / libc)
#   Darwin also cross-arch (x64 ↔ arm64) with the Apple SDK
#   target OS != host OS → cargo zigbuild
#   Windows gnu; macOS only if SDKROOT is set when the host is not macOS
#
# Path scrub: isolated CARGO_HOME / CARGO_TARGET_DIR, remap-path-prefix,
# -C strip=symbols, then a post-link strip. The strings scan rejects the home
# directory and the source tree. Paths under /tmp are accepted. The binary
# is not rewritten.
#
# Env: CARGO_HOME_DIR, CARGO_TARGET_DIR, DIST_DIR, PAGEMD_LINUX_GLIBC
#      SKIP_PACKAGE=1 SKIP_VERIFY=1
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

NAME="pagemd"
DIST_DIR="${DIST_DIR:-dist}"
GLIBC="${PAGEMD_LINUX_GLIBC:-2.17}"
CARGO_HOME_DIR="${CARGO_HOME_DIR:-/tmp/pagemd-cargo}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/pagemd-target}"
RUSTUP_HOME="${RUSTUP_HOME:-${HOME}/.rustup}"

VERSION="$(cargo metadata --no-deps --format-version 1 2>/dev/null \
  | sed -n 's/.*"name":"pagemd","version":"\([^"]*\)".*/\1/p' | head -n1)"
if [[ -z "${VERSION}" ]]; then
  VERSION="$(grep -E '^version\s*=' Cargo.toml | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')"
fi

HOST_UNAME="$(uname -s)"
HOST_TRIPLE="$(rustc -vV | awk '/^host:/{print $2}')"

case "${HOST_UNAME}" in
  Darwin) HOST_KIND="macos" ;;
  Linux) HOST_KIND="linux" ;;
  MINGW*|MSYS*|CYGWIN*) HOST_KIND="windows" ;;
  *) HOST_KIND="unknown" ;;
esac

host_arch_name() {
  case "${HOST_TRIPLE}" in
    x86_64-*) echo x64 ;;
    aarch64-*) echo arm64 ;;
    *) echo unknown ;;
  esac
}

os_of_triple() {
  case "$1" in
    *apple-darwin*) echo macos ;;
    *linux*) echo linux ;;
    *windows*) echo windows ;;
    *) echo unknown ;;
  esac
}

arch_of_triple() {
  case "$1" in
    x86_64-*) echo x64 ;;
    aarch64-*) echo arm64 ;;
    *) echo unknown ;;
  esac
}

HOST_ARCH_NAME="$(host_arch_name)"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: missing required command: $1" >&2
    [[ -n "${2:-}" ]] && echo "  hint: $2" >&2
    exit 1
  }
}

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

# rustc last-match-wins for overlapping remaps: list broad prefixes first.
build_remap_flags() {
  local flags=()
  flags+=("--remap-path-prefix=${HOME}=/home")
  if [[ -n "${RUSTUP_HOME}" && -d "${RUSTUP_HOME}" ]]; then
    flags+=("--remap-path-prefix=${RUSTUP_HOME}=/rustup")
  fi
  flags+=("--remap-path-prefix=${CARGO_HOME_DIR}=/cargo")
  flags+=("--remap-path-prefix=${ROOT}=/src")
  flags+=("--remap-path-prefix=${CARGO_TARGET_DIR}=/target")
  flags+=("-C" "strip=symbols")
  local out="" f
  for f in "${flags[@]}"; do
    out+="${out:+ }${f}"
  done
  printf '%s' "$out"
}

export_build_env() {
  local extra="${1:-}"
  mkdir -p "$CARGO_HOME_DIR" "$CARGO_TARGET_DIR"
  export CARGO_HOME="$CARGO_HOME_DIR"
  export CARGO_TARGET_DIR
  local remap
  remap="$(build_remap_flags)"
  if [[ -n "$extra" ]]; then
    export RUSTFLAGS="${remap} ${extra}"
  else
    export RUSTFLAGS="${remap}"
  fi
}

use_native() {
  local triple="$1"
  local tos tarch
  tos="$(os_of_triple "${triple}")"
  tarch="$(arch_of_triple "${triple}")"
  if [[ "${tos}" != "${HOST_KIND}" ]]; then
    return 1
  fi
  # Same OS: Darwin can native-cross x64/arm64; other OSes only same arch.
  if [[ "${HOST_KIND}" == "macos" ]]; then
    return 0
  fi
  [[ "${tarch}" == "${HOST_ARCH_NAME}" ]]
}

bin_path() {
  local triple="$1"
  local bin="${CARGO_TARGET_DIR}/${triple}/release/${NAME}"
  if [[ "${triple}" == *"-windows-"* ]]; then
    bin="${bin}.exe"
  fi
  printf '%s' "$bin"
}

package_zip() {
  local os_name="$1"
  local arch_name="$2"
  local src_bin="$3"
  local zip_name="${NAME}-${os_name}-${arch_name}-${VERSION}.zip"
  local stage="${DIST_DIR}/.stage-${os_name}-${arch_name}"
  local bin_name="${NAME}"
  if [[ "${os_name}" == "windows" ]]; then
    bin_name="${NAME}.exe"
  fi

  rm -rf "${stage}"
  mkdir -p "${stage}" "${DIST_DIR}"
  cp "${src_bin}" "${stage}/${bin_name}"

  rm -f "${DIST_DIR}/${zip_name}"
  (
    cd "${stage}"
    zip -qr "${ROOT}/${DIST_DIR}/${zip_name}" "${bin_name}"
  )
  rm -rf "${stage}"
  echo "    -> ${DIST_DIR}/${zip_name}"
}

find_rust_objcopy() {
  local sysroot
  sysroot="$(rustc --print sysroot 2>/dev/null)" || return 0
  find "${sysroot}/lib/rustlib" -name rust-objcopy -type f 2>/dev/null | head -1
}

strip_bin() {
  local triple="$1"
  local bin="$2"
  local tos
  tos="$(os_of_triple "${triple}")"
  echo "==> strip ${tos} $(basename "$bin")"
  case "${tos}" in
    macos)
      if command -v strip >/dev/null 2>&1 && strip -x "$bin" 2>/dev/null; then
        return 0
      fi
      if command -v llvm-strip >/dev/null 2>&1; then
        llvm-strip "$bin" && return 0
      fi
      echo "warn: macOS strip skipped for $bin (already stripped?)" >&2
      ;;
    linux)
      local objcopy
      objcopy="$(find_rust_objcopy)"
      if [[ -n "${objcopy}" ]]; then
        "$objcopy" --strip-all "$bin"
        return 0
      fi
      if command -v llvm-strip >/dev/null 2>&1; then
        llvm-strip --strip-all "$bin"
        return 0
      fi
      echo "warn: no rust-objcopy/llvm-strip; relying on -C strip=symbols for $bin" >&2
      ;;
    windows)
      if command -v x86_64-w64-mingw32-strip >/dev/null 2>&1; then
        x86_64-w64-mingw32-strip "$bin" || echo "warn: mingw strip failed for $bin" >&2
        return 0
      fi
      local objcopy
      objcopy="$(find_rust_objcopy)"
      if [[ -n "${objcopy}" ]]; then
        "$objcopy" --strip-all "$bin" || echo "warn: rust-objcopy strip failed for $bin" >&2
        return 0
      fi
      echo "warn: no Windows strip tool; relying on -C strip=symbols for $bin" >&2
      ;;
  esac
}

verify_no_personal_paths() {
  local bin="$1"
  local label="$2"
  if ! command -v strings >/dev/null 2>&1; then
    echo "warn: strings not found; skip path scan for $label" >&2
    return 0
  fi
  echo "==> verify paths: $label"
  local hits
  # /tmp and /private/tmp are the system temp dir (cargo's isolated home lives
  # there). Drop those paths before matching so a dependency's
  # env!("CARGO_MANIFEST_DIR") does not fail the scan. The binary is unchanged.
  hits="$(
    strings "$bin" \
      | sed -E \
        -e 's#/private/tmp/[A-Za-z0-9._+/-]*##g' \
        -e 's#/tmp/[A-Za-z0-9._+/-]*##g' \
      | grep -E \
        -e "${HOME}" \
        -e "${CARGO_HOME_DIR}" \
        -e "${CARGO_TARGET_DIR}" \
        -e "${ROOT}" \
        -e '/Users/[^/]+/\.cargo' \
        -e '/Users/[^/]+/\.rustup' \
        -e '/Users/[^/]+/Dev/' \
        || true
  )"
  if [[ -n "$hits" ]]; then
    echo "error: personal path(s) still embedded in $bin:" >&2
    echo "$hits" | head -40 >&2
    return 1
  fi
  echo "    OK: $bin"
}

package_built_bin() {
  local triple="$1"
  local os_name="$2"
  local arch_name="$3"
  local bin
  bin="$(bin_path "${triple}")"
  if [[ ! -f "${bin}" ]]; then
    echo "error: missing binary ${bin}" >&2
    return 1
  fi
  strip_bin "${triple}" "${bin}"
  if [[ "${SKIP_VERIFY:-0}" != "1" ]]; then
    verify_no_personal_paths "${bin}" "${os_name}-${arch_name}"
  fi
  if [[ "${SKIP_PACKAGE:-0}" != "1" ]]; then
    package_zip "${os_name}" "${arch_name}" "${bin}"
  fi
}

# Linux always zigbuilds so the binary links glibc ${GLIBC}, even on a Linux host.
build_linux() {
  local triple="$1"
  local arch_name="$2"
  local zig_target="${triple}.${GLIBC}"

  ensure_target "${triple}" || return 1
  echo "==> cargo zigbuild --release --target ${zig_target}"
  export_build_env
  cargo zigbuild --release --target "${zig_target}" || return 1
  package_built_bin "${triple}" "linux" "${arch_name}"
}

build_one() {
  local triple="$1"
  local os_name="$2"
  local arch_name="$3"

  if [[ "${os_name}" == "linux" ]]; then
    build_linux "${triple}" "${arch_name}"
    return
  fi

  ensure_target "${triple}" || return 1

  if use_native "${triple}"; then
    echo "==> native cargo --release --target ${triple}"
    export_build_env
    cargo build --release --target "${triple}" || return 1
  else
    local zig_target="${triple}"
    local extra=""
    if [[ "${os_name}" == "macos" && -z "${SDKROOT:-}" && "${HOST_KIND}" != "macos" ]]; then
      echo "warning: SDKROOT unset; skipping ${os_name}-${arch_name}" >&2
      return 1
    fi
    if [[ "${os_name}" == "windows" && -n "${MINGW_DLLTOOL:-}" ]] \
      && command -v "${MINGW_DLLTOOL}" >/dev/null 2>&1; then
      extra="-C dlltool=${MINGW_DLLTOOL}"
      export "CC_x86_64_pc_windows_gnu=${MINGW_CC:-x86_64-w64-mingw32-gcc}"
      export "AR_x86_64_pc_windows_gnu=${MINGW_AR:-x86_64-w64-mingw32-ar}"
    fi
    echo "==> cargo zigbuild --release --target ${zig_target}"
    export_build_env "${extra}"
    cargo zigbuild --release --target "${zig_target}" || return 1
  fi

  package_built_bin "${triple}" "${os_name}" "${arch_name}"
}

try_build() {
  if ! build_one "$@"; then
    echo "warning: skipped $2-$3" >&2
    return 0
  fi
}

usage() {
  echo "usage: $0 [macos:arm64|macos:x64|linux:x64|linux:arm64|windows:x64]" >&2
  echo "  no args        build every pagemd zip" >&2
  echo "  macos:arm64    build that zip only" >&2
  exit 1
}

build_all() {
  # glibc pin is the portability promise; fail the release if linux x64 cannot build.
  build_one "x86_64-unknown-linux-gnu" "linux" "x64" || return 1
  try_build "aarch64-unknown-linux-gnu" "linux" "arm64"

  if [[ "${HOST_KIND}" == "windows" ]]; then
    build_one "${HOST_TRIPLE}" "windows" "${HOST_ARCH_NAME}"
  else
    build_one "x86_64-pc-windows-gnu" "windows" "x64"
  fi

  if [[ "${HOST_KIND}" == "macos" ]]; then
    try_build "x86_64-apple-darwin" "macos" "x64"
    try_build "aarch64-apple-darwin" "macos" "arm64"
    if command -v lipo >/dev/null 2>&1 \
      && [[ -f "$(bin_path x86_64-apple-darwin)" ]] \
      && [[ -f "$(bin_path aarch64-apple-darwin)" ]]; then
      echo "==> lipo macos-universal"
      local univ_bin="${DIST_DIR}/.pagemd-universal"
      lipo -create \
        -output "${univ_bin}" \
        "$(bin_path x86_64-apple-darwin)" \
        "$(bin_path aarch64-apple-darwin)"
      strip_bin "aarch64-apple-darwin" "${univ_bin}"
      if [[ "${SKIP_PACKAGE:-0}" != "1" ]]; then
        package_zip "macos" "universal" "${univ_bin}"
      fi
      rm -f "${univ_bin}"
    fi
  else
    echo "note: host is not macOS; Apple targets need a macOS SDK (SDKROOT)"
    try_build "x86_64-apple-darwin" "macos" "x64"
    try_build "aarch64-apple-darwin" "macos" "arm64"
  fi
}

needs_zig() {
  local only="${1:-}"
  if [[ -z "${only}" || "${only}" == linux:* || "${only}" == windows:* ]]; then
    if [[ "${only}" == windows:* && "${HOST_KIND}" == "windows" ]]; then
      return 1
    fi
    return 0
  fi
  # macOS-only on a Mac uses the native SDK.
  if [[ "${only}" == macos:* && "${HOST_KIND}" == "macos" ]]; then
    return 1
  fi
  return 0
}

ONLY="${1:-}"
case "${ONLY}" in
  ""|macos:arm64|macos:x64|linux:x64|linux:arm64|windows:x64) ;;
  *) usage ;;
esac

echo "${NAME} ${VERSION} → ${DIST_DIR}/ (host ${HOST_KIND}/${HOST_ARCH_NAME} ${HOST_TRIPLE}, linux glibc ${GLIBC})"
if [[ -n "${ONLY}" ]]; then
  echo "target: ${ONLY}"
else
  echo "linux via zigbuild (glibc ${GLIBC}); native cargo on ${HOST_KIND}; zigbuild for other OSes"
fi
echo "zip: ${NAME}-{os}-{arch}-{version}.zip"

need_cmd cargo "rustup default stable"
need_cmd rustup "https://rustup.rs/"
need_cmd zip
if needs_zig "${ONLY}"; then
  need_cmd zig "mise run release installs zig"
  need_cmd cargo-zigbuild "mise run release installs cargo-zigbuild"
fi

echo "==> clean cargo target cache ${CARGO_TARGET_DIR}"
rm -rf "${CARGO_TARGET_DIR}"

export_build_env
if ! cargo fetch -q; then
  echo "repairing broken registry in $CARGO_HOME_DIR" >&2
  rm -rf "$CARGO_HOME_DIR/registry"
  cargo fetch -q
fi

mkdir -p "${DIST_DIR}"

case "${ONLY}" in
  macos:arm64) build_one "aarch64-apple-darwin" "macos" "arm64" ;;
  macos:x64) build_one "x86_64-apple-darwin" "macos" "x64" ;;
  linux:x64) build_one "x86_64-unknown-linux-gnu" "linux" "x64" ;;
  linux:arm64) build_one "aarch64-unknown-linux-gnu" "linux" "arm64" ;;
  windows:x64)
    if [[ "${HOST_KIND}" == "windows" ]]; then
      build_one "${HOST_TRIPLE}" "windows" "${HOST_ARCH_NAME}"
    else
      build_one "x86_64-pc-windows-gnu" "windows" "x64"
    fi
    ;;
  "") build_all ;;
esac

rm -rf "${DIST_DIR}"/.stage-* 2>/dev/null || true

echo
echo "Done. Artifacts:"
ls -la "${DIST_DIR}"/*.zip 2>/dev/null | sed 's/^/  /' || ls -la "${DIST_DIR}" | sed 's/^/  /'
