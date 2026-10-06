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

/// 在临时目录新建包并作为当前文档。名称、容量和扇区由命令层固定。
#[no_mangle]
pub extern "C" fn et_create_package(
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
        let view = with_session(|session| session.create_package(discard_unsaved != 0))?;
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

/// 把 etpk 文件解包到临时目录并作为当前文档。失败时不替换当前会话。
#[no_mangle]
pub extern "C" fn et_open_package(
    file_utf8: *const c_char,
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
        let file = c_str(file_utf8, "文件")?;
        let view =
            with_session(|session| session.open_package(Path::new(file), discard_unsaved != 0))?;
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

/// 把当前工作副本打包成 etpk 文件。成功后文档仍然打开。
#[no_mangle]
pub extern "C" fn et_save_package(
    file_utf8: *const c_char,
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
        let file = c_str(file_utf8, "文件")?;
        let view = with_session(|session| session.save_package(Path::new(file)))?;
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

/// 关掉当前文档并删除临时工作副本。
#[no_mangle]
pub extern "C" fn et_close_package() {
    with_session(|session| session.close());
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

/// 删除勾选的分区。失败时不改当前会话。
#[no_mangle]
pub extern "C" fn et_remove_partitions(
    ids_json: *const c_char,
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
        let ids = c_str(ids_json, "分区选择")?;
        let view = with_session(|session| session.remove_partitions(ids))?;
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
        et_abi_version, et_close_package, et_create_package, et_import_package, et_open_package,
        et_preview_import, et_save_package, et_string_free, et_version,
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
        assert_eq!(
            et_create_package(0, std::ptr::null_mut(), std::ptr::null_mut()),
            1
        );

        let tmp = TempDir::new();
        let mut view = std::ptr::null_mut();
        let mut error = std::ptr::null_mut();
        let rc = et_create_package(1, &mut view, &mut error);
        assert_eq!(rc, 0);
        assert!(error.is_null());
        let view_text = unsafe { CStr::from_ptr(view) }
            .to_str()
            .unwrap()
            .to_string();
        assert!(view_text.contains("\"dirty\":true"));
        assert!(view_text.contains("\"archive\":null"));
        assert!(view_text.contains("\"name\":\"Untitled\""));
        assert!(view_text.contains("\"userAreaBytes\":null"));
        assert!(view_text.contains("\"sectorSize\":512"));
        assert!(view_text.contains("\"alignment\":1048576"));
        et_string_free(view);

        let package = tmp.0.join("Untitled.etpk");
        let package_c = CString::new(package.to_str().unwrap()).unwrap();
        view = std::ptr::null_mut();
        error = std::ptr::null_mut();
        let rc = et_save_package(package_c.as_ptr(), &mut view, &mut error);
        assert_eq!(rc, 0, "{}", unsafe {
            if error.is_null() {
                String::new()
            } else {
                CStr::from_ptr(error).to_string_lossy().into_owned()
            }
        });
        assert!(error.is_null());
        let view_text = unsafe { CStr::from_ptr(view) }.to_str().unwrap();
        assert!(view_text.contains("\"dirty\":false"));
        assert!(view_text.contains("Untitled.etpk"));
        et_string_free(view);
        assert!(package.is_file());

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
        assert!(view_text.contains("\"name\":\"Untitled\""));
        assert!(view_text.contains("\"partitions\":[]"));
        assert!(view_text.contains("\"dirty\":false"));
        et_string_free(view);
        et_close_package();
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
        let mut view = std::ptr::null_mut();
        let mut error = std::ptr::null_mut();
        let rc = et_create_package(1, &mut view, &mut error);
        assert_eq!(rc, 0);
        let created = unsafe { CStr::from_ptr(view) }
            .to_str()
            .unwrap()
            .to_string();
        et_string_free(view);
        let created_json: serde_json::Value = serde_json::from_str(&created).unwrap();
        let root = std::path::PathBuf::from(created_json["root"].as_str().unwrap());

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
        assert!(root.join("manifest.json").is_file());
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
        assert!(view_text.contains("\"name\":\"Untitled\""));
        assert!(view_text.contains("\"name\":\"boot\""));
        assert!(view_text.contains("1572864"));
        et_string_free(view);
        assert!(root.join("manifest.json").is_file());
        assert!(!tmp.0.join("imported.etpk").exists());
        et_close_package();
    }
}
