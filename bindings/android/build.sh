#!/usr/bin/env bash
# Builds libpagemd.so for Android with cargo-ndk.
#
#   bindings/android/build.sh [jniLibs directory]
#
# Writes <dir>/<abi>/libpagemd.so for arm64-v8a, armeabi-v7a and x86_64
# (default dir: bindings/android/dist/jniLibs). Copy kotlin/ (the PageMD
# class) into the app's sources. Needs the Android NDK (ANDROID_NDK_HOME)
# and cargo-ndk (`cargo install cargo-ndk`).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${1:-$HERE/dist/jniLibs}"
mkdir -p "$OUT"
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android >/dev/null
cd "$HERE"
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o "$OUT" build --release
ls -R "$OUT"
