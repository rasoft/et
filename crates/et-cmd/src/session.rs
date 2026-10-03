//! 当前打开的包。
//!
//! 撤销栈会在编辑命令接入时放在这里。新建成功后没有可撤销的操作。

use std::path::{Path, PathBuf};

use serde::Serialize;

use et_core::disk;
use et_core::manifest::Metadata;
use et_core::Error;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentView {
    pub root: String,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub metadata: ViewMetadata,
    pub partitions: Vec<ViewPartition>,
    pub issues: Vec<ViewIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewMetadata {
    pub name: String,
    pub description: String,
    pub sector_size: u32,
    pub user_area_bytes: u64,
    pub alignment: u64,
}

/// 分区行在增加分区时填入。新建的空包没有分区。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewPartition {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub partition_type: String,
    pub start_bytes: u64,
    pub start_fixed: bool,
    pub size_bytes: u64,
    pub image: Option<String>,
}

/// 校验项在布局校验接入后填入。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewIssue {
    pub severity: String,
    pub partition_id: Option<String>,
    pub message: String,
}

impl DocumentView {
    pub fn to_json(&self) -> Result<String, Error> {
        serde_json::to_string(self).map_err(|err| Error::new(format!("无法生成文档视图：{err}")))
    }
}

#[derive(Debug)]
pub struct Session {
    root: Option<PathBuf>,
    manifest: Option<et_core::manifest::Manifest>,
    dirty: bool,
}

impl Session {
    pub const fn new() -> Self {
        Self {
            root: None,
            manifest: None,
            dirty: false,
        }
    }

    pub fn create_package(
        &mut self,
        dir: &Path,
        name: &str,
        user_area_bytes: u64,
        sector_size: u32,
        discard_unsaved: bool,
    ) -> Result<DocumentView, Error> {
        if self.dirty && !discard_unsaved {
            return Err(Error::new("有未保存的修改"));
        }
        let metadata = Metadata::try_new(name, user_area_bytes, sector_size)?;
        let created = disk::create_package(dir, metadata)?;
        self.root = Some(created.root);
        self.manifest = Some(created.manifest);
        self.dirty = false;
        self.view()
    }

    fn view(&self) -> Result<DocumentView, Error> {
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| Error::new("没有打开的包"))?;
        let manifest = self
            .manifest
            .as_ref()
            .ok_or_else(|| Error::new("没有打开的包"))?;
        document_view(root, manifest, self.dirty)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

fn document_view(
    root: &Path,
    manifest: &et_core::manifest::Manifest,
    dirty: bool,
) -> Result<DocumentView, Error> {
    let root = root
        .to_str()
        .ok_or_else(|| Error::new("路径不是合法的 UTF-8"))?
        .to_string();
    Ok(DocumentView {
        root,
        dirty,
        can_undo: false,
        can_redo: false,
        metadata: ViewMetadata {
            name: manifest.metadata.name.clone(),
            description: manifest.metadata.description.clone(),
            sector_size: manifest.metadata.sector_size,
            user_area_bytes: manifest.metadata.user_area_bytes,
            alignment: manifest.metadata.alignment,
        },
        partitions: Vec::new(),
        issues: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::Session;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("et-session-{nanos}-{n}"));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const GIB16: u64 = 16 * 1024 * 1024 * 1024;

    #[test]
    fn create_package_opens_a_clean_empty_document() {
        let tmp = TempDir::new();
        let dir = tmp.path().join("board-d1.etpk");
        let mut session = Session::new();
        let view = session
            .create_package(&dir, "board-d1", GIB16, 512, false)
            .unwrap();

        assert!(!view.dirty);
        assert!(!view.can_undo);
        assert!(!view.can_redo);
        assert!(view.partitions.is_empty());
        assert!(view.issues.is_empty());
        assert_eq!(view.metadata.name, "board-d1");
        assert_eq!(view.metadata.user_area_bytes, GIB16);
        assert_eq!(view.metadata.sector_size, 512);
        assert_eq!(view.metadata.alignment, 1024 * 1024);
        assert!(view.root.ends_with("board-d1.etpk"));
        assert!(!session.dirty);

        let json: serde_json::Value = serde_json::from_str(&view.to_json().unwrap()).unwrap();
        assert_eq!(json["dirty"], false);
        assert_eq!(json["canUndo"], false);
        assert_eq!(json["canRedo"], false);
        assert_eq!(json["metadata"]["name"], "board-d1");
        assert_eq!(json["metadata"]["userAreaBytes"], serde_json::json!(GIB16));
        assert_eq!(json["partitions"], serde_json::json!([]));
        assert_eq!(json["issues"], serde_json::json!([]));
    }

    #[test]
    fn dirty_document_is_kept_until_discard_is_confirmed() {
        let tmp = TempDir::new();
        let first = tmp.path().join("first.etpk");
        let second = tmp.path().join("second.etpk");
        let mut session = Session::new();
        session
            .create_package(&first, "first", GIB16, 512, false)
            .unwrap();
        session.dirty = true;

        let err = session
            .create_package(&second, "second", GIB16, 512, false)
            .unwrap_err();
        assert_eq!(err.message(), "有未保存的修改");
        assert!(!second.exists());
        assert!(session.dirty);
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "first");
        assert!(first.join("manifest.json").is_file());

        let view = session
            .create_package(&second, "second", GIB16, 4096, true)
            .unwrap();
        assert_eq!(view.metadata.name, "second");
        assert_eq!(view.metadata.sector_size, 4096);
        assert!(!view.dirty);
        assert!(!session.dirty);
        assert!(first.join("manifest.json").is_file());
        assert!(second.join("manifest.json").is_file());
    }

    #[test]
    fn invalid_name_does_not_replace_the_open_package() {
        let tmp = TempDir::new();
        let first = tmp.path().join("first.etpk");
        let second = tmp.path().join("second.etpk");
        let mut session = Session::new();
        session
            .create_package(&first, "first", GIB16, 512, false)
            .unwrap();

        let err = session
            .create_package(&second, "", GIB16, 512, false)
            .unwrap_err();
        assert_eq!(err.message(), "名称为空");
        assert!(!second.exists());
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "first");
    }
}
