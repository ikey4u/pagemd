package io.github.ikey4u.pagemd

/**
 * pagemd: Markdown to a self-contained HTML document for a WebView.
 *
 * Loads `libpagemd.so` (built by `bindings/android/build.sh` into
 * `jniLibs/<abi>/`). Call [setCacheDir] once with the app's cache directory
 * (for example `context.cacheDir.path`) before the first [renderHtml].
 */
object PageMD {
    init {
        System.loadLibrary("pagemd")
    }

    /** The HTML document for [markdown]; throws [IllegalStateException] on failure. */
    @JvmStatic
    external fun renderHtml(markdown: String): String

    /** Uses [path] for pagemd's cache (fonts, resources); throws if it can't be written. */
    @JvmStatic
    external fun setCacheDir(path: String)
}
