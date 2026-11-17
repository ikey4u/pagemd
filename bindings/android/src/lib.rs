//! pagemd for Android: JNI functions behind the Kotlin object
//! `io.github.ikey4u.pagemd.PageMD` (`kotlin/io/github/ikey4u/pagemd/PageMD.kt`),
//! built into `libpagemd.so` by `build.sh`. Failures become
//! `IllegalStateException`s in Kotlin.

use jni::{
    objects::{JClass, JString},
    sys::jstring,
    JNIEnv,
};

const ERROR_CLASS: &str = "java/lang/IllegalStateException";

fn text(env: &mut JNIEnv, value: &JString) -> Result<String, String> {
    env.get_string(value)
        .map(Into::into)
        .map_err(|e| format!("not a string: {e}"))
}

/// `PageMD.renderHtml(markdown: String): String`.
#[no_mangle]
pub extern "system" fn Java_io_github_ikey4u_pagemd_PageMD_renderHtml<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    markdown: JString<'local>,
) -> jstring {
    let result = text(&mut env, &markdown)
        .and_then(|md| pagemd_bindings::render_html(&md));
    match result
        .and_then(|html| env.new_string(html).map_err(|e| e.to_string()))
    {
        Ok(s) => s.into_raw(),
        Err(e) => {
            let _ = env.throw_new(ERROR_CLASS, e);
            std::ptr::null_mut()
        }
    }
}

/// `PageMD.setCacheDir(path: String)`.
#[no_mangle]
pub extern "system" fn Java_io_github_ikey4u_pagemd_PageMD_setCacheDir<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    path: JString<'local>,
) {
    let result = text(&mut env, &path)
        .and_then(|p| pagemd_bindings::set_cache_dir(std::path::Path::new(&p)));
    if let Err(e) = result {
        let _ = env.throw_new(ERROR_CLASS, e);
    }
}
