#!/usr/bin/env bash
# Builds PageMD.xcframework (device and simulator, arm64) from this crate.
#
#   bindings/ios/build-xcframework.sh [output directory]
#
# The output (default: bindings/ios/dist) gets PageMD.xcframework, whose
# headers carry pagemd.h and a module map, so Swift can `import PageMD`.
# Needs Xcode (xcodebuild) and the Rust targets aarch64-apple-ios and
# aarch64-apple-ios-sim.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$(mkdir -p "${1:-$HERE/dist}" && cd "${1:-$HERE/dist}" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

rustup target add aarch64-apple-ios aarch64-apple-ios-sim >/dev/null
TARGET_DIR="$(cd "$HERE" && cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
args=()
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
  (cd "$HERE" && cargo build --release --target "$target")
  mkdir -p "$WORK/$target/Headers"
  cp "$HERE/include/pagemd.h" "$HERE/module.modulemap" "$WORK/$target/Headers/"
  cp "$TARGET_DIR/$target/release/libpagemd_ios.a" "$WORK/$target/libpagemd.a"
  args+=(-library "$WORK/$target/libpagemd.a" -headers "$WORK/$target/Headers")
done

rm -rf "$OUT/PageMD.xcframework"
xcodebuild -create-xcframework "${args[@]}" -output "$OUT/PageMD.xcframework"
echo "$OUT/PageMD.xcframework"
