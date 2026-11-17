//! pagemd for iOS: the C interface in `include/pagemd.h`, built into
//! `PageMD.xcframework` by `build-xcframework.sh` and imported in Swift as
//! `import PageMD`.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn fail(message: impl Into<String>) {
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(message.into()));
}

fn to_c(s: String) -> *mut c_char {
    // Interior NULs can't cross; drop them so the document still arrives.
    let s = if s.contains('\0') { s.replace('\0', "") } else { s };
    CString::new(s).map(CString::into_raw).unwrap_or(std::ptr::null_mut())
}

unsafe fn from_c<'a>(ptr: *const c_char, what: &str) -> Option<&'a str> {
    if ptr.is_null() {
        fail(format!("{what} is NULL"));
        return None;
    }
    match CStr::from_ptr(ptr).to_str() {
        Ok(s) => Some(s),
        Err(e) => {
            fail(format!("{what} is not UTF-8: {e}"));
            None
        }
    }
}

/// See `pagemd_render_html` in `include/pagemd.h`.
///
/// # Safety
/// `markdown` must be NULL or a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn pagemd_render_html(markdown: *const c_char) -> *mut c_char {
    LAST_ERROR.with(|e| *e.borrow_mut() = None);
    let Some(markdown) = from_c(markdown, "markdown") else { return std::ptr::null_mut() };
    match pagemd_bindings::render_html(markdown) {
        Ok(html) => to_c(html),
        Err(e) => {
            fail(e);
            std::ptr::null_mut()
        }
    }
}

/// See `pagemd_set_cache_dir` in `include/pagemd.h`.
///
/// # Safety
/// `path` must be NULL or a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn pagemd_set_cache_dir(path: *const c_char) -> c_int {
    let Some(path) = from_c(path, "path") else { return -1 };
    match pagemd_bindings::set_cache_dir(Path::new(path)) {
        Ok(()) => 0,
        Err(e) => {
            fail(e);
            -1
        }
    }
}

/// See `pagemd_last_error` in `include/pagemd.h`.
#[no_mangle]
pub extern "C" fn pagemd_last_error() -> *mut c_char {
    LAST_ERROR.with(|e| e.borrow().clone().map(to_c).unwrap_or(std::ptr::null_mut()))
}

/// See `pagemd_string_free` in `include/pagemd.h`.
///
/// # Safety
/// `string` must be NULL or a string returned by this library, freed once.
#[no_mangle]
pub unsafe extern "C" fn pagemd_string_free(string: *mut c_char) {
    if !string.is_null() {
        drop(CString::from_raw(string));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_through_the_c_interface() {
        let md = CString::new("# Hello").unwrap();
        let out = unsafe { pagemd_render_html(md.as_ptr()) };
        assert!(!out.is_null());
        let html = unsafe { CStr::from_ptr(out) }.to_str().unwrap().to_string();
        unsafe { pagemd_string_free(out) };
        assert!(html.contains("Hello"));
        assert!(pagemd_last_error().is_null());
    }

    #[test]
    fn reports_errors() {
        assert!(unsafe { pagemd_render_html(std::ptr::null()) }.is_null());
        let err = pagemd_last_error();
        assert!(!err.is_null());
        assert_eq!(unsafe { CStr::from_ptr(err) }.to_str().unwrap(), "markdown is NULL");
        unsafe { pagemd_string_free(err) };
        let bad = CString::new("/proc/not-writable").unwrap();
        assert_eq!(unsafe { pagemd_set_cache_dir(bad.as_ptr()) }, -1);
    }
}
