//! 交给 Qt 的 C ABI。
//!
//! 后续 `DocumentView` 使用 UTF-8 JSON。文本由 Rust 分配，调用方用
//! [`et_string_free`] 交还。本文件里的 `et_version` 例外：它指向进程级静态
//! 字符串，不要释放。

use std::ffi::{c_char, CString};
use std::sync::OnceLock;

#[no_mangle]
pub extern "C" fn et_abi_version() -> u32 {
    1
}

#[no_mangle]
pub extern "C" fn et_version() -> *const c_char {
    version_cstring().as_ptr()
}

#[no_mangle]
pub extern "C" fn et_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // 指针必须来自本库先前用 CString::into_raw 交给 C 的字符串。
    drop(unsafe { CString::from_raw(ptr) });
}

fn version_cstring() -> &'static CString {
    static VERSION: OnceLock<CString> = OnceLock::new();
    VERSION.get_or_init(|| CString::new(et_core::version()).expect("版本号不含内部 NUL"))
}

#[cfg(test)]
mod tests {
    use super::{et_abi_version, et_string_free, et_version};
    use std::ffi::CStr;
    use std::ffi::CString;

    #[test]
    fn abi_version_starts_at_one() {
        assert_eq!(et_abi_version(), 1);
    }

    #[test]
    fn version_matches_core_and_is_not_freed_here() {
        let text = unsafe { CStr::from_ptr(et_version()) };
        assert_eq!(text.to_str().unwrap(), et_core::version());
    }

    #[test]
    fn string_free_accepts_null_and_owned_cstring() {
        et_string_free(std::ptr::null_mut());
        let owned = CString::new("ok").unwrap();
        et_string_free(owned.into_raw());
    }
}
