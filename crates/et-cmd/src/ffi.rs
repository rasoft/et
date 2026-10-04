//! 交给 Qt 的 C ABI。
//!
//! `DocumentView` 使用 UTF-8 JSON。文本由 Rust 分配，调用方用
//! [`et_string_free`] 交还。本文件里的 `et_version` 例外：它指向进程级静态
//! 字符串，不要释放。

use std::ffi::{c_char, CStr, CString};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use crate::session::Session;

static SESSION: Mutex<Session> = Mutex::new(Session::new());

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

/// 新建包并作为当前文档。容量和扇区用整数传入，避免界面侧的 JSON 数字丢掉精度。
#[no_mangle]
pub extern "C" fn et_create_package(
    dir_utf8: *const c_char,
    name_utf8: *const c_char,
    user_area_bytes: u64,
    sector_size: u32,
    discard_unsaved: i32,
    out_view: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    if out_view.is_null() || out_error.is_null() {
        return 1;
    }
    unsafe {
        *out_view = std::ptr::null_mut();
        *out_error = std::ptr::null_mut();
    }

    let result = (|| {
        let dir = c_str(dir_utf8, "目录")?;
        let name = c_str(name_utf8, "名称")?;
        let view = with_session(|session| {
            session.create_package(
                Path::new(dir),
                name,
                user_area_bytes,
                sector_size,
                discard_unsaved != 0,
            )
        })?;
        view.to_json()
    })();

    match result {
        Ok(json) => write_out(out_view, json),
        Err(err) => {
            let _ = write_out(out_error, err.message().to_string());
            1
        }
    }
}

/// 打开已有包并作为当前文档。失败时不替换当前会话。
#[no_mangle]
pub extern "C" fn et_open_package(
    dir_utf8: *const c_char,
    discard_unsaved: i32,
    out_view: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    if out_view.is_null() || out_error.is_null() {
        return 1;
    }
    unsafe {
        *out_view = std::ptr::null_mut();
        *out_error = std::ptr::null_mut();
    }

    let result = (|| {
        let dir = c_str(dir_utf8, "目录")?;
        let view =
            with_session(|session| session.open_package(Path::new(dir), discard_unsaved != 0))?;
        view.to_json()
    })();

    match result {
        Ok(json) => write_out(out_view, json),
        Err(err) => {
            let _ = write_out(out_error, err.message().to_string());
            1
        }
    }
}

/// 列出可导入的分区表和镜像。不改当前会话。
#[no_mangle]
pub extern "C" fn et_preview_import(
    source_utf8: *const c_char,
    out_preview: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    if out_preview.is_null() || out_error.is_null() {
        return 1;
    }
    unsafe {
        *out_preview = std::ptr::null_mut();
        *out_error = std::ptr::null_mut();
    }

    let result = (|| {
        let source = c_str(source_utf8, "文件")?;
        crate::session::Session::preview_import(Path::new(source))
    })();

    match result {
        Ok(json) => write_out(out_preview, json),
        Err(err) => {
            let _ = write_out(out_error, err.message().to_string());
            1
        }
    }
}

/// 按勾选把 flash.conf 或 download.bin 写入当前打开的包。失败时不改当前会话。
#[no_mangle]
pub extern "C" fn et_import_package(
    source_utf8: *const c_char,
    selection_utf8: *const c_char,
    out_view: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    if out_view.is_null() || out_error.is_null() {
        return 1;
    }
    unsafe {
        *out_view = std::ptr::null_mut();
        *out_error = std::ptr::null_mut();
    }

    let result = (|| {
        let source = c_str(source_utf8, "文件")?;
        let selection = c_str(selection_utf8, "选择")?;
        let view = with_session(|session| session.import_package(Path::new(source), selection))?;
        view.to_json()
    })();

    match result {
        Ok(json) => write_out(out_view, json),
        Err(err) => {
            let _ = write_out(out_error, err.message().to_string());
            1
        }
    }
}

fn with_session<T>(f: impl FnOnce(&mut Session) -> T) -> T {
    let mut session = SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut session)
}

fn c_str<'a>(ptr: *const c_char, label: &str) -> Result<&'a str, et_core::Error> {
    if ptr.is_null() {
        return Err(et_core::Error::new(format!("{label}为空")));
    }
    let text = unsafe { CStr::from_ptr(ptr) };
    text.to_str()
        .map_err(|_| et_core::Error::new(format!("{label}不是合法的 UTF-8")))
}

fn write_out(slot: *mut *mut c_char, text: String) -> i32 {
    match CString::new(text) {
        Ok(value) => {
            unsafe { *slot = value.into_raw() };
            0
        }
        Err(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        et_abi_version, et_create_package, et_import_package, et_open_package, et_preview_import,
        et_string_free, et_version,
    };
    use std::ffi::{CStr, CString};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 这些测试共用进程里的会话，必须串行，否则导入会写进另一个测试的包。
    fn session_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

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

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("et-ffi-{nanos}-{n}"));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn create_package_ffi_returns_json_or_an_error_string() {
        let _session = session_test_lock();
        let dir = CString::new("unused").unwrap();
        let name = CString::new("unused").unwrap();
        assert_eq!(
            et_create_package(
                dir.as_ptr(),
                name.as_ptr(),
                0,
                512,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ),
            1
        );

        let tmp = TempDir::new();
        let missing = tmp.0.join("nope.etpk");
        let missing_c = CString::new(missing.to_str().unwrap()).unwrap();
        let empty_name = CString::new("").unwrap();
        let mut view = std::ptr::null_mut();
        let mut error = std::ptr::null_mut();
        let rc = et_create_package(
            missing_c.as_ptr(),
            empty_name.as_ptr(),
            16 * 1024 * 1024 * 1024,
            512,
            0,
            &mut view,
            &mut error,
        );
        assert_eq!(rc, 1);
        assert!(view.is_null());
        assert!(!missing.exists());
        let error_text = unsafe { CStr::from_ptr(error) }.to_str().unwrap();
        assert_eq!(error_text, "名称为空");
        et_string_free(error);

        let package = tmp.0.join("board-d1.etpk");
        let package_c = CString::new(package.to_str().unwrap()).unwrap();
        let package_name = CString::new("board-d1").unwrap();
        view = std::ptr::null_mut();
        error = std::ptr::null_mut();
        let rc = et_create_package(
            package_c.as_ptr(),
            package_name.as_ptr(),
            16 * 1024 * 1024 * 1024,
            512,
            0,
            &mut view,
            &mut error,
        );
        assert_eq!(rc, 0);
        assert!(error.is_null());
        let view_text = unsafe { CStr::from_ptr(view) }.to_str().unwrap();
        assert!(view_text.contains("\"dirty\":false"));
        assert!(view_text.contains("\"name\":\"board-d1\""));
        assert!(view_text.contains("17179869184"));
        et_string_free(view);
        assert!(package.join("manifest.json").is_file());
        assert!(package.join("images").is_dir());

        view = std::ptr::null_mut();
        error = std::ptr::null_mut();
        assert_eq!(
            et_open_package(
                package_c.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            1
        );
        let rc = et_open_package(package_c.as_ptr(), 0, &mut view, &mut error);
        assert_eq!(rc, 0);
        assert!(error.is_null());
        let view_text = unsafe { CStr::from_ptr(view) }.to_str().unwrap();
        assert!(view_text.contains("\"name\":\"board-d1\""));
        assert!(view_text.contains("\"partitions\":[]"));
        et_string_free(view);
    }

    #[test]
    fn import_package_ffi_returns_json_or_an_error_string() {
        let _session = session_test_lock();
        let source = CString::new("unused").unwrap();
        assert_eq!(
            et_import_package(
                source.as_ptr(),
                source.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ),
            1
        );

        let tmp = TempDir::new();
        let package = tmp.0.join("kept.etpk");
        let package_c = CString::new(package.to_str().unwrap()).unwrap();
        let package_name = CString::new("kept").unwrap();
        let mut view = std::ptr::null_mut();
        let mut error = std::ptr::null_mut();
        let rc = et_create_package(
            package_c.as_ptr(),
            package_name.as_ptr(),
            16 * 1024 * 1024 * 1024,
            512,
            1,
            &mut view,
            &mut error,
        );
        assert_eq!(rc, 0);
        et_string_free(view);

        let missing = tmp.0.join("missing.conf");
        let missing_c = CString::new(missing.to_str().unwrap()).unwrap();
        view = std::ptr::null_mut();
        error = std::ptr::null_mut();
        let selection = CString::new(r#"{"importTable":true,"images":[]}"#).unwrap();
        let rc = et_import_package(
            missing_c.as_ptr(),
            selection.as_ptr(),
            &mut view,
            &mut error,
        );
        assert_eq!(rc, 1);
        assert!(view.is_null());
        assert!(package.join("manifest.json").is_file());
        let error_text = unsafe { CStr::from_ptr(error) }.to_str().unwrap();
        assert!(error_text.contains("文件不存在"));
        et_string_free(error);

        let conf = tmp.0.join("flash.conf");
        fs::write(tmp.0.join("boot.img"), b"xyz").unwrap();
        fs::write(
            &conf,
            "block_size 512\ntable_type gpt\nflash_size 0x10000\nboot boot.img true RAW ro 0 0 0xC00 0x200\n",
        )
        .unwrap();
        let conf_c = CString::new(conf.to_str().unwrap()).unwrap();
        view = std::ptr::null_mut();
        error = std::ptr::null_mut();
        let rc = et_preview_import(conf_c.as_ptr(), &mut view, &mut error);
        assert_eq!(rc, 0);
        assert!(error.is_null());
        let preview_text = unsafe { CStr::from_ptr(view) }.to_str().unwrap();
        assert!(preview_text.contains("\"fileName\":\"boot.img\""));
        assert!(preview_text.contains("\"name\":\"boot\""));
        et_string_free(view);

        view = std::ptr::null_mut();
        error = std::ptr::null_mut();
        let chosen = CString::new(r#"{"importTable":true,"images":[0]}"#).unwrap();
        let rc = et_import_package(conf_c.as_ptr(), chosen.as_ptr(), &mut view, &mut error);
        assert_eq!(rc, 0, "{}", unsafe {
            if error.is_null() {
                String::new()
            } else {
                CStr::from_ptr(error).to_string_lossy().into_owned()
            }
        });
        assert!(error.is_null());
        let view_text = unsafe { CStr::from_ptr(view) }.to_str().unwrap();
        assert!(view_text.contains("\"name\":\"kept\""));
        assert!(view_text.contains("\"name\":\"boot\""));
        assert!(view_text.contains("1572864"));
        et_string_free(view);
        assert!(package.join("manifest.json").is_file());
        assert!(!tmp.0.join("imported.etpk").exists());
    }
}
