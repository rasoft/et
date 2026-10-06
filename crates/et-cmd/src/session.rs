//! 当前打开的包。
//!
//! 编辑发生在临时目录里的工作副本。`.etpk` 文件是这份目录的打包结果。
//! 撤销栈会在编辑命令接入时放在这里。新建成功后没有可撤销的操作。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use et_core::disk;
use et_core::manifest::Metadata;
use et_core::Error;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentView {
    pub root: String,
    /// 最近一次保存或打开的 etpk 文件。新建后还没保存时为 `None`。
    pub archive: Option<String>,
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
    pub attributes: u64,
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
    archive: Option<PathBuf>,
    manifest: Option<et_core::manifest::Manifest>,
    dirty: bool,
}

impl Session {
    pub const fn new() -> Self {
        Self {
            root: None,
            archive: None,
            manifest: None,
            dirty: false,
        }
    }

    /// 在临时目录创建工作副本并作为当前文档。
    /// 名称固定为 Untitled，不写容量，扇区 512 字节，对齐 1 MiB。还没有 etpk 文件，因此是未保存。
    pub fn create_package(&mut self, discard_unsaved: bool) -> Result<DocumentView, Error> {
        let metadata = Metadata::without_capacity("Untitled", 512)?;
        if self.dirty && !discard_unsaved {
            return Err(Error::new("有未保存的修改"));
        }
        let dir = allocate_work_path();
        let created = disk::create_package(&dir, metadata)?;
        let view = match document_view(&created.root, None, &created.manifest, true) {
            Ok(view) => view,
            Err(err) => {
                let _ = fs::remove_dir_all(&created.root);
                return Err(err);
            }
        };
        self.adopt(created.root, created.manifest, None, true);
        Ok(view)
    }

    /// 把 etpk 文件解包到新的临时目录并作为当前文档。失败时不替换当前会话。
    pub fn open_package(
        &mut self,
        file: &Path,
        discard_unsaved: bool,
    ) -> Result<DocumentView, Error> {
        if self.dirty && !discard_unsaved {
            return Err(Error::new("有未保存的修改"));
        }
        let archive = existing_archive(file)?;
        self.replace_with_archive(archive)
    }

    /// 把当前工作副本打包成 etpk 文件。成功后文档仍然打开，脏标记清掉。
    pub fn save_package(&mut self, file: &Path) -> Result<DocumentView, Error> {
        let root = self
            .root
            .clone()
            .ok_or_else(|| Error::new("没有打开的包"))?;
        let manifest = self
            .manifest
            .clone()
            .ok_or_else(|| Error::new("没有打开的包"))?;
        let archive = et_core::archive::pack_package(&root, file)?;
        let view = document_view(&root, Some(&archive), &manifest, false)?;
        self.archive = Some(archive);
        self.dirty = false;
        Ok(view)
    }

    /// 把工作副本打包到另一个 etpk 文件，然后关掉当前文档并打开这个新文件。
    /// 打包失败或新文件打不开时，当前文档保持不变。
    pub fn save_as_package(&mut self, file: &Path) -> Result<DocumentView, Error> {
        let root = self
            .root
            .clone()
            .ok_or_else(|| Error::new("没有打开的包"))?;
        if self.manifest.is_none() {
            return Err(Error::new("没有打开的包"));
        }
        let archive = et_core::archive::pack_package(&root, file)?;
        self.replace_with_archive(archive)
    }

    /// 丢掉工作副本。退出时调用，避免临时目录留下来。
    pub fn close(&mut self) {
        if let Some(root) = self.root.take() {
            let _ = fs::remove_dir_all(root);
        }
        self.archive = None;
        self.manifest = None;
        self.dirty = false;
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
        self.dirty = true;
        self.view()
    }

    /// 删除勾选的分区。镜像文件从 `images/` 去掉，manifest 立即写回。
    /// 没有打开的包、或 id 不存在时失败，失败时不改会话。
    pub fn remove_partitions(&mut self, ids_json: &str) -> Result<DocumentView, Error> {
        let ids = parse_partition_ids(ids_json)?;
        let root = self
            .root
            .clone()
            .ok_or_else(|| Error::new("没有打开的包"))?;
        let current = self
            .manifest
            .clone()
            .ok_or_else(|| Error::new("没有打开的包"))?;
        let manifest = et_core::remove_partitions(&root, &current, &ids)?;
        self.manifest = Some(manifest);
        self.dirty = true;
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
        document_view(root, self.archive.as_deref(), manifest, self.dirty)
    }

    fn replace_with_archive(&mut self, archive: PathBuf) -> Result<DocumentView, Error> {
        let mut work = WorkDir::create()?;
        et_core::archive::unpack_package(&archive, work.path())?;
        let opened = disk::open_package(work.path())?;
        let view = document_view(work.path(), Some(&archive), &opened.manifest, false)?;
        let root = work.disarm();
        self.adopt(root, opened.manifest, Some(archive), false);
        Ok(view)
    }

    fn adopt(
        &mut self,
        root: PathBuf,
        manifest: et_core::manifest::Manifest,
        archive: Option<PathBuf>,
        dirty: bool,
    ) {
        if let Some(old) = self.root.take() {
            if old != root {
                let _ = fs::remove_dir_all(old);
            }
        }
        self.root = Some(root);
        self.manifest = Some(manifest);
        self.archive = archive;
        self.dirty = dirty;
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(root) = self.root.take() {
            let _ = fs::remove_dir_all(root);
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

fn allocate_work_path() -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("et-work-{}-{nanos}-{n}", std::process::id()))
}

struct WorkDir {
    path: PathBuf,
    armed: bool,
}

impl WorkDir {
    fn create() -> Result<Self, Error> {
        let path = allocate_work_path();
        fs::create_dir(&path)
            .map_err(|err| Error::new(format!("无法创建临时目录（{}）：{err}", path.display())))?;
        Ok(Self { path, armed: true })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn disarm(&mut self) -> PathBuf {
        self.armed = false;
        self.path.clone()
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn existing_archive(file: &Path) -> Result<PathBuf, Error> {
    if file.as_os_str().is_empty() {
        return Err(Error::new("没有指定文件"));
    }
    match fs::symlink_metadata(file) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(Error::new("文件不存在")),
        Err(err) => Err(Error::new(format!(
            "无法读取 etpk 文件（{}）：{err}",
            file.display()
        ))),
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
            Err(Error::new("不是 etpk 文件"))
        }
        Ok(_) => file
            .canonicalize()
            .map_err(|err| Error::new(format!("无法解析文件（{}）：{err}", file.display()))),
    }
}

fn parse_partition_ids(json: &str) -> Result<Vec<String>, Error> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| Error::new("分区选择无法解析"))?;
    let array = value
        .as_array()
        .ok_or_else(|| Error::new("分区选择无法解析"))?;
    let mut ids = Vec::with_capacity(array.len());
    for item in array {
        let id = item
            .as_str()
            .ok_or_else(|| Error::new("分区选择无法解析"))?;
        if id.is_empty() {
            return Err(Error::new("分区 id 为空"));
        }
        ids.push(id.to_string());
    }
    Ok(ids)
}

fn document_view(
    root: &Path,
    archive: Option<&Path>,
    manifest: &et_core::manifest::Manifest,
    dirty: bool,
) -> Result<DocumentView, Error> {
    let report = et_core::validate::check(root, manifest)?;
    let placed = et_core::validate::placed_partitions(manifest);
    let root = root
        .to_str()
        .ok_or_else(|| Error::new("路径不是合法的 UTF-8"))?
        .to_string();
    let archive = match archive {
        Some(path) => Some(
            path.to_str()
                .ok_or_else(|| Error::new("路径不是合法的 UTF-8"))?
                .to_string(),
        ),
        None => None,
    };
    let partitions = manifest
        .partitions
        .iter()
        .enumerate()
        .map(|(index, partition)| ViewPartition {
            id: partition.id.clone(),
            name: partition.name.clone(),
            partition_type: partition.partition_type.clone(),
            attributes: partition.attributes,
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
        archive,
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
    fn create_package_opens_an_unsaved_document_and_save_keeps_it_open() {
        let tmp = TempDir::new();
        let mut session = Session::new();
        let view = session.create_package(false).unwrap();

        assert!(view.dirty);
        assert!(view.archive.is_none());
        assert!(!view.can_undo);
        assert!(!view.can_redo);
        assert!(view.partitions.is_empty());
        assert!(view.issues.is_empty());
        assert_eq!(view.metadata.name, "Untitled");
        assert_eq!(view.metadata.user_area_bytes, None);
        assert_eq!(view.metadata.sector_size, 512);
        assert_eq!(view.metadata.alignment, 1024 * 1024);
        assert!(view.root.contains("et-work-"));
        assert!(session.dirty);
        let root = view.root.clone();

        let json: serde_json::Value = serde_json::from_str(&view.to_json().unwrap()).unwrap();
        assert_eq!(json["dirty"], true);
        assert_eq!(json["archive"], serde_json::Value::Null);
        assert_eq!(json["canUndo"], false);
        assert_eq!(json["canRedo"], false);
        assert_eq!(json["metadata"]["name"], "Untitled");
        assert_eq!(json["metadata"]["userAreaBytes"], serde_json::Value::Null);
        assert_eq!(json["metadata"]["sectorSize"], 512);
        assert_eq!(json["metadata"]["alignment"], 1024 * 1024);
        assert_eq!(json["partitions"], serde_json::json!([]));
        assert_eq!(json["issues"], serde_json::json!([]));

        let dest = tmp.path().join("Untitled.etpk");
        let saved = session.save_package(&dest).unwrap();
        assert!(!saved.dirty);
        assert!(!session.dirty);
        assert_eq!(saved.root, root);
        assert!(std::path::Path::new(&root).join("manifest.json").is_file());
        assert!(saved.archive.as_ref().unwrap().ends_with("Untitled.etpk"));
        assert!(dest.is_file());

        let mut other = Session::new();
        let opened = other.open_package(&dest, false).unwrap();
        assert!(!opened.dirty);
        assert_eq!(opened.metadata.name, "Untitled");
        assert_eq!(opened.metadata.user_area_bytes, None);
        assert_ne!(opened.root, root);
        assert!(opened.archive.as_ref().unwrap().ends_with("Untitled.etpk"));
    }

    #[test]
    fn save_as_closes_the_previous_file_and_opens_the_new_one() {
        let tmp = TempDir::new();
        let mut session = Session::new();
        let created = session.create_package(false).unwrap();
        let first_root = created.root.clone();
        let first = tmp.path().join("first.etpk");
        session.save_package(&first).unwrap();

        let missing = tmp.path().join("missing").join("second.etpk");
        let err = session.save_as_package(&missing).unwrap_err();
        assert!(err.message().contains("上级目录不存在"));
        assert_eq!(session.root.as_ref().unwrap().to_str().unwrap(), first_root);
        assert!(session.archive.as_ref().unwrap().ends_with("first.etpk"));
        assert!(!missing.exists());

        let second = tmp.path().join("second.etpk");
        let saved = session.save_as_package(&second).unwrap();
        assert!(!saved.dirty);
        assert!(!session.dirty);
        assert_ne!(saved.root, first_root);
        assert!(!std::path::Path::new(&first_root).exists());
        assert!(first.is_file());
        assert!(second.is_file());
        assert!(saved.archive.as_ref().unwrap().ends_with("second.etpk"));
        assert!(std::path::Path::new(&saved.root)
            .join("manifest.json")
            .is_file());
        assert_eq!(
            session.archive.as_ref().unwrap().to_str().unwrap(),
            saved.archive.as_deref().unwrap()
        );

        let mut other = Session::new();
        let opened = other.open_package(&second, false).unwrap();
        assert_eq!(opened.metadata.name, "Untitled");
        assert!(opened.archive.as_ref().unwrap().ends_with("second.etpk"));
    }

    #[test]
    fn dirty_document_is_kept_until_discard_is_confirmed() {
        let mut session = Session::new();
        let first = session.create_package(false).unwrap();
        let first_root = std::path::PathBuf::from(&first.root);

        let err = session.create_package(false).unwrap_err();
        assert_eq!(err.message(), "有未保存的修改");
        assert!(session.dirty);
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "Untitled");
        assert!(first_root.join("manifest.json").is_file());

        let view = session.create_package(true).unwrap();
        assert_eq!(view.metadata.name, "Untitled");
        assert_eq!(view.metadata.sector_size, 512);
        assert_eq!(view.metadata.user_area_bytes, None);
        assert_eq!(view.metadata.alignment, 1024 * 1024);
        assert_ne!(view.root, first.root);
        assert!(view.dirty);
        assert!(session.dirty);
        assert!(view.archive.is_none());
        assert!(!first_root.exists());
        assert!(std::path::Path::new(&view.root)
            .join("manifest.json")
            .is_file());
    }

    #[test]
    fn open_package_reports_layout_and_keeps_the_previous_document_on_failure() {
        let tmp = TempDir::new();
        let mut session = Session::new();
        let first = session.create_package(false).unwrap();
        let first_root = first.root.clone();
        let saved = tmp.path().join("first.etpk");
        session.save_package(&saved).unwrap();

        let second_dir = tmp.path().join("second-src");
        fs::create_dir_all(second_dir.join("images")).unwrap();
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
        fs::write(
            second_dir.join("manifest.json"),
            manifest.to_bytes().unwrap(),
        )
        .unwrap();
        let second = tmp.path().join("second.etpk");
        et_core::archive::pack_package(&second_dir, &second).unwrap();

        let err = session.open_package(&second, false).unwrap_err();
        assert!(err.message().contains("越出包目录"));
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "Untitled");
        assert_eq!(session.root.as_ref().unwrap().to_str().unwrap(), first_root);

        manifest.partitions[0].image =
            Some("images/6f1c2a0e-7b4d-4e3a-9c1f-2a8b0d5e6f70.img".to_string());
        fs::write(
            second_dir.join("manifest.json"),
            manifest.to_bytes().unwrap(),
        )
        .unwrap();
        et_core::archive::pack_package(&second_dir, &second).unwrap();
        session.dirty = true;
        let err = session.open_package(&second, false).unwrap_err();
        assert_eq!(err.message(), "有未保存的修改");
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "Untitled");

        let view = session.open_package(&second, true).unwrap();
        assert_eq!(view.metadata.name, "second");
        assert!(!view.dirty);
        assert_eq!(view.partitions.len(), 1);
        assert_eq!(view.partitions[0].name, "boot");
        assert_eq!(view.partitions[0].attributes, 0);
        assert_eq!(view.partitions[0].start_bytes, 1024 * 1024);
        assert!(!view.partitions[0].start_fixed);
        assert!(view.partitions[0].image_bytes.is_none());
        assert!(view
            .issues
            .iter()
            .any(|issue| issue.message.contains("不存在")));
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "second");
        assert!(view.archive.as_ref().unwrap().ends_with("second.etpk"));
        assert!(!std::path::Path::new(&first_root).exists());
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

        let mut session = Session::new();
        let created = session.create_package(false).unwrap();
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
        assert!(view.dirty);
        assert_eq!(view.root, root);
        assert_eq!(view.metadata.name, "Untitled");
        assert_eq!(view.metadata.sector_size, 512);
        assert_eq!(view.partitions.len(), 1);
        assert_eq!(view.partitions[0].name, "boot");
        assert!(view.partitions[0].start_fixed);
        assert_eq!(view.partitions[0].start_bytes, 0x800 * 512);
        assert_eq!(view.partitions[0].size_bytes, Some(0x400 * 512));
        assert_eq!(view.partitions[0].image_bytes, Some(10));
        assert!(view.issues.is_empty());
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "Untitled");
        assert!(!tmp.path().join("imported.etpk").exists());

        let missing = tmp.path().join("missing.conf");
        fs::write(&missing, b"not a conf").unwrap();
        let err = session
            .import_package(&missing, r#"{"importTable":true,"images":[]}"#)
            .unwrap_err();
        assert!(err.message().contains("9 列") || err.message().contains("无法识别"));
        assert_eq!(session.manifest.as_ref().unwrap().metadata.name, "Untitled");
        assert_eq!(
            session.manifest.as_ref().unwrap().partitions[0].name,
            "boot"
        );
    }

    #[test]
    fn remove_partitions_drops_checked_rows_and_reflows_the_next_one() {
        let mut session = Session::new();
        session.create_package(false).unwrap();
        let boot = "11111111-1111-4111-8111-111111111111";
        let system = "22222222-2222-4222-8222-222222222222";
        let manifest = session.manifest.as_mut().unwrap();
        manifest.partitions.push(et_core::manifest::Partition {
            id: boot.to_string(),
            name: "boot".to_string(),
            size_bytes: Some(1024 * 1024),
            start_bytes: None,
            partition_type: "linux-filesystem".to_string(),
            attributes: 0,
            image: None,
            extra: serde_json::Map::new(),
        });
        manifest.partitions.push(et_core::manifest::Partition {
            id: system.to_string(),
            name: "system".to_string(),
            size_bytes: Some(1024 * 1024),
            start_bytes: None,
            partition_type: "linux-filesystem".to_string(),
            attributes: 0,
            image: None,
            extra: serde_json::Map::new(),
        });
        let view = session
            .remove_partitions(&format!(r#"["{boot}"]"#))
            .unwrap();
        assert_eq!(view.partitions.len(), 1);
        assert_eq!(view.partitions[0].name, "system");
        assert_eq!(view.partitions[0].start_bytes, 1024 * 1024);
        assert!(view.dirty);

        let err = session.remove_partitions("[]").unwrap_err();
        assert_eq!(err.message(), "没有选中的分区");
        assert_eq!(session.manifest.as_ref().unwrap().partitions.len(), 1);
    }
}
