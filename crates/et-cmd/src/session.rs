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
    pub user_area_bytes: Option<u64>,
    pub alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewPartition {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub partition_type: String,
    pub start_bytes: u64,
    pub start_fixed: bool,
    pub size_bytes: Option<u64>,
    pub image: Option<String>,
    pub image_bytes: Option<u64>,
}

/// 校验项。打开含错误的包仍然成功，路径越出包目录则整次打开失败。
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

    pub fn open_package(
        &mut self,
        dir: &Path,
        discard_unsaved: bool,
    ) -> Result<DocumentView, Error> {
        if self.dirty && !discard_unsaved {
            return Err(Error::new("有未保存的修改"));
        }
        let opened = disk::open_package(dir)?;
        let view = document_view(&opened.root, &opened.manifest, false)?;
        self.root = Some(opened.root);
        self.manifest = Some(opened.manifest);
        self.dirty = false;
        Ok(view)
    }

    /// 列出源文件里的分区表和镜像。不改当前包。
    pub fn preview_import(source: &Path) -> Result<String, Error> {
        et_core::import::preview_import(source)
    }

    /// 按勾选把 flash.conf 或 download.bin 写入当前打开的包。目录和名称不变。
    /// 没有打开的包时失败，失败时不改会话。
    pub fn import_package(
        &mut self,
        source: &Path,
        selection_json: &str,
    ) -> Result<DocumentView, Error> {
        let selection = et_core::import::selection_from_json(selection_json)?;
        let root = self
            .root
            .clone()
            .ok_or_else(|| Error::new("没有打开的包"))?;
        let current = self
            .manifest
            .clone()
            .ok_or_else(|| Error::new("没有打开的包"))?;
        let manifest = et_core::import::import_selected(source, &root, &current, &selection)?;
        self.manifest = Some(manifest);
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
    let report = et_core::validate::check(root, manifest)?;
    let placed = et_core::validate::placed_partitions(manifest);
    let root = root
        .to_str()
        .ok_or_else(|| Error::new("路径不是合法的 UTF-8"))?
        .to_string();
    let partitions = manifest
        .partitions
        .iter()
        .enumerate()
        .map(|(index, partition)| ViewPartition {
            id: partition.id.clone(),
            name: partition.name.clone(),
            partition_type: partition.partition_type.clone(),
            start_bytes: placed
                .get(index)
                .and_then(|place| place.start_bytes)
                .unwrap_or(0),
            start_fixed: placed
                .get(index)
                .map(|place| place.start_fixed)
                .unwrap_or(false),
            size_bytes: partition.size_bytes,
            image: partition.image.clone(),
            image_bytes: report.image_lengths.get(index).copied().unwrap_or(None),
        })
        .collect();
    let issues = report
        .issues
        .iter()
        .map(|issue| ViewIssue {
            severity: "error".to_string(),
            partition_id: issue.partition_index.and_then(|index| {
                manifest
                    .partitions
                    .get(index)
                    .map(|partition| partition.id.clone())
            }),
            message: issue.message.clone(),
        })
        .collect();
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
        partitions,
        issues,
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
        assert_eq!(view.metadata.user_area_bytes, Some(GIB16));
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

    #[test]
    fn open_package_reports_layout_and_keeps_the_previous_document_on_failure() {
        let tmp = TempDir::new();
        let first = tmp.path().join("first.etpk");
        let second = tmp.path().join("second.etpk");
        let mut session = Session::new();
        session
            .create_package(&first, "first", GIB16, 512, false)
            .unwrap();

        fs::create_dir_all(second.join("images")).unwrap();
        let mut manifest = et_core::manifest::Manifest::new(
            et_core::manifest::Metadata::try_new("second", GIB16, 512).unwrap(),
        );
        manifest.partitions.push(et_core::manifest::Partition {
            id: "6f1c2a0e-7b4d-4e3a-9c1f-2a8b0d5e6f70".to_string(),
            name: "boot".to_string(),
            size_bytes: Some(64 * 1024 * 1024),
            start_bytes: None,
            partition_type: "linux-filesystem".to_string(),
            attributes: 0,
            image: Some("../secret.img".to_string()),
            extra: serde_json::Map::new(),
        });
        fs::write(second.join("manifest.json"), manifest.to_bytes().unwrap()).unwrap();

        let err = session.open_package(&second, false).unwrap_err();
        assert!(err.message().contains("越出包目录"));
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "first");

        manifest.partitions[0].image =
            Some("images/6f1c2a0e-7b4d-4e3a-9c1f-2a8b0d5e6f70.img".to_string());
        fs::write(second.join("manifest.json"), manifest.to_bytes().unwrap()).unwrap();
        session.dirty = true;
        let err = session.open_package(&second, false).unwrap_err();
        assert_eq!(err.message(), "有未保存的修改");
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "first");

        let view = session.open_package(&second, true).unwrap();
        assert_eq!(view.metadata.name, "second");
        assert!(!view.dirty);
        assert_eq!(view.partitions.len(), 1);
        assert_eq!(view.partitions[0].name, "boot");
        assert_eq!(view.partitions[0].start_bytes, 1024 * 1024);
        assert!(!view.partitions[0].start_fixed);
        assert!(view.partitions[0].image_bytes.is_none());
        assert!(view
            .issues
            .iter()
            .any(|issue| issue.message.contains("不存在")));
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "second");
    }

    #[test]
    fn import_package_writes_the_open_package_and_keeps_it_when_import_fails() {
        let tmp = TempDir::new();
        let mut closed = Session::new();
        let err = closed
            .import_package(
                &tmp.path().join("flash.conf"),
                r#"{"importTable":true,"images":[]}"#,
            )
            .unwrap_err();
        assert_eq!(err.message(), "没有打开的包");

        let first = tmp.path().join("first.etpk");
        let mut session = Session::new();
        let created = session
            .create_package(&first, "first", GIB16, 512, false)
            .unwrap();
        let root = created.root.clone();
        session.dirty = true;

        let conf = tmp.path().join("flash.conf");
        fs::write(tmp.path().join("boot.img"), b"boot-bytes").unwrap();
        fs::write(
            &conf,
            "\
block_size 512
table_type gpt
flash_size 0x10000

boot boot.img true RAW ro 1 7 0x800 0x400
",
        )
        .unwrap();
        let view = session
            .import_package(&conf, r#"{"importTable":true,"images":[0]}"#)
            .unwrap();
        assert!(!view.dirty);
        assert_eq!(view.root, root);
        assert_eq!(view.metadata.name, "first");
        assert_eq!(view.metadata.sector_size, 512);
        assert_eq!(view.partitions.len(), 1);
        assert_eq!(view.partitions[0].name, "boot");
        assert!(view.partitions[0].start_fixed);
        assert_eq!(view.partitions[0].start_bytes, 0x800 * 512);
        assert_eq!(view.partitions[0].size_bytes, Some(0x400 * 512));
        assert_eq!(view.partitions[0].image_bytes, Some(10));
        assert!(view.issues.is_empty());
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "first");
        assert!(!tmp.path().join("imported.etpk").exists());

        let missing = tmp.path().join("missing.conf");
        fs::write(&missing, b"not a conf").unwrap();
        let err = session
            .import_package(&missing, r#"{"importTable":true,"images":[]}"#)
            .unwrap_err();
        assert!(err.message().contains("9 列") || err.message().contains("无法识别"));
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "first");
        assert_eq!(
            session.manifest.as_ref().unwrap().partitions[0].name,
            "boot"
        );
    }
}
