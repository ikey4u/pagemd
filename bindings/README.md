# Mobile bindings

pagemd-core for apps: Markdown in, a self-contained HTML document out, ready for
a web view. Both bindings share `shared/` (rendering for embedding, and a
writable cache directory inside an app sandbox).

| Directory | What it builds | How |
| --- | --- | --- |
| `ios/` | `PageMD.xcframework`: a static library, `pagemd.h` and a module map, so Swift can `import PageMD` | `ios/build-xcframework.sh [out]` (Xcode) |
| `android/` | `libpagemd.so` per ABI, and the Kotlin object `io.github.ikey4u.pagemd.PageMD` in `android/kotlin/` | `android/build.sh [jniLibs]` (Android NDK, cargo-ndk) |
| `shared/` | The Rust crate both use: `render_html`, `set_cache_dir` | — |

## iOS

```swift
import PageMD

let caches = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].path
_ = pagemd_set_cache_dir(caches)
if let out = pagemd_render_html("# Hello") {
    let html = String(cString: out)
    pagemd_string_free(out)
}
```

On failure `pagemd_render_html` returns NULL; `pagemd_last_error()` says why.

## Android

```kotlin
PageMD.setCacheDir(context.cacheDir.path)
val html = PageMD.renderHtml("# Hello")
```

Failures throw `IllegalStateException`.

## Workspace

The bindings are members of the repository's root Cargo workspace, so they
build against `crates/pagemd-core` with the same `Cargo.lock` (and dependency
versions) as the CLI. Build with `--locked`. They are not default members:
`cargo build` at the root skips them; use `-p pagemd-ios`, `-p pagemd-android`
or `--workspace`.

The cache directory is passed to pagemd through `RenderOptions`. The
bindings never set environment variables, because `setenv` can race with
other threads of the app reading the environment.

## Tests

`cargo test --locked -p pagemd-bindings -p pagemd-ios -p pagemd-android` from
the repository root (the C interface runs on any host; the JNI crate builds on
the host too, without an NDK).
