//! 镜像复制、删除，以及 manifest 的原子改名。
//!
//! 打开包时只做存在性和长度检查，不把镜像内容读进内存。

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::layout::MAX_PARTITIONS;
use crate::manifest::{Manifest, Metadata};
use crate::validate::{inspect_relative, InsidePath, InspectError};

#[derive(Debug)]
pub struct CreatedPackage {
    pub root: PathBuf,
    pub manifest: Manifest,
}

/// 创建包目录、`manifest.json` 和空的 `images/`。
///
/// 目标必须还不存在，或是一个空目录。失败时删掉这次创建出来的文件，不留半份包。
pub fn create_package(dir: &Path, metadata: Metadata) -> Result<CreatedPackage, Error> {
    metadata.validate()?;
    let root = resolve_package_dir(dir)?;
    if root.to_str().is_none() {
        return Err(Error::new("路径不是合法的 UTF-8"));
    }

    let mut cleanup = prepare_root(&root)?;
    let images = root.join("images");
    fs::create_dir(&images).map_err(|err| {
        Error::new(format!(
            "创建 images 目录失败（{}）：{err}",
            images.display()
        ))
    })?;
    let manifest = Manifest::new(metadata);
    let bytes = manifest.to_bytes()?;
    write_manifest_atomic(&root, &bytes)?;
    cleanup.disarm();
    Ok(CreatedPackage { root, manifest })
}

#[derive(Debug)]
pub struct OpenedPackage {
    pub root: PathBuf,
    pub manifest: Manifest,
}

/// 打开已有包。只读 manifest，并拒绝空 id、重复 id 和超过 128 条分区。
/// 镜像是否越出包目录由随后的校验决定，这里不读取镜像内容。
pub fn open_package(dir: &Path) -> Result<OpenedPackage, Error> {
    if dir.as_os_str().is_empty() {
        return Err(Error::new("没有指定目录"));
    }
    let root = resolve_existing_dir(dir)?;
    if root.to_str().is_none() {
        return Err(Error::new("路径不是合法的 UTF-8"));
    }
    let bytes = read_manifest(&root)?;
    let manifest = Manifest::from_slice(&bytes)?;
    if manifest.partitions.len() > MAX_PARTITIONS {
        return Err(Error::new("分区不能超过 128 条"));
    }
    let mut seen = HashSet::new();
    for partition in &manifest.partitions {
        if partition.id.is_empty() {
            return Err(Error::new("分区 id 为空"));
        }
        if !seen.insert(partition.id.clone()) {
            return Err(Error::new(format!("分区 id 重复：{}", partition.id)));
        }
    }
    Ok(OpenedPackage { root, manifest })
}

fn resolve_existing_dir(dir: &Path) -> Result<PathBuf, Error> {
    match fs::symlink_metadata(dir) {
        Err(err) if err.kind() == ErrorKind::NotFound => Err(Error::new("目录不存在")),
        Err(err) => Err(Error::new(format!(
            "无法检查目录（{}）：{err}",
            dir.display()
        ))),
        Ok(meta) if meta.file_type().is_symlink() || meta.is_dir() => {
            let root = dir.canonicalize().map_err(|err| {
                if err.kind() == ErrorKind::NotFound {
                    Error::new("目录不存在")
                } else {
                    Error::new(format!("无法解析目录（{}）：{err}", dir.display()))
                }
            })?;
            if root.is_dir() {
                Ok(root)
            } else {
                Err(Error::new("目标路径不是目录"))
            }
        }
        Ok(_) => Err(Error::new("目标路径不是目录")),
    }
}

fn read_manifest(root: &Path) -> Result<Vec<u8>, Error> {
    match inspect_relative(root, "manifest.json") {
        Ok(InsidePath::File { path, .. }) => {
            fs::read(path).map_err(|err| Error::new(format!("无法读取 manifest.json：{err}")))
        }
        Ok(InsidePath::Missing) => Err(Error::new("找不到 manifest.json")),
        Ok(InsidePath::NotFile) => Err(Error::new("manifest.json 不是常规文件")),
        Err(InspectError::Escape) => Err(Error::new("manifest.json 越出包目录")),
        Err(InspectError::Failed(message)) => Err(Error::new(message)),
    }
}

fn resolve_package_dir(dir: &Path) -> Result<PathBuf, Error> {
    if dir.as_os_str().is_empty() {
        return Err(Error::new("没有指定目录"));
    }
    let name = match dir.file_name() {
        Some(name) if !name.is_empty() => name,
        _ => return Err(Error::new("目录路径无效")),
    };
    let parent = match dir.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    if !parent.exists() {
        return Err(Error::new(format!("上级目录不存在：{}", parent.display())));
    }
    if !parent.is_dir() {
        return Err(Error::new(format!(
            "上级路径不是目录：{}",
            parent.display()
        )));
    }
    let parent = parent
        .canonicalize()
        .map_err(|err| Error::new(format!("无法解析上级目录（{}）：{err}", parent.display())))?;
    Ok(parent.join(name))
}

/// 新建失败时把这次留下的文件清掉。用户原先就有的空目录不会被删。
struct PackageCleanup {
    root: PathBuf,
    remove_root: bool,
    armed: bool,
}

impl PackageCleanup {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PackageCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let _ = fs::remove_file(self.root.join("manifest.json.tmp"));
        let _ = fs::remove_file(self.root.join("manifest.json"));
        let _ = fs::remove_dir(self.root.join("images"));
        if self.remove_root {
            let _ = fs::remove_dir(&self.root);
        }
    }
}

fn prepare_root(root: &Path) -> Result<PackageCleanup, Error> {
    match fs::symlink_metadata(root) {
        Ok(meta) if meta.file_type().is_symlink() => Err(Error::new(format!(
            "目标路径是符号链接：{}",
            root.display()
        ))),
        Ok(meta) if !meta.is_dir() => Err(Error::new(format!(
            "目标路径已存在，且不是目录：{}",
            root.display()
        ))),
        Ok(_) => {
            if !directory_is_empty(root)? {
                return Err(Error::new(format!(
                    "目标目录已存在且不为空，请换一个路径：{}",
                    root.display()
                )));
            }
            Ok(PackageCleanup {
                root: root.to_path_buf(),
                remove_root: false,
                armed: true,
            })
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            fs::create_dir(root)
                .map_err(|err| Error::new(format!("创建目录失败（{}）：{err}", root.display())))?;
            Ok(PackageCleanup {
                root: root.to_path_buf(),
                remove_root: true,
                armed: true,
            })
        }
        Err(err) => Err(Error::new(format!(
            "无法检查目录（{}）：{err}",
            root.display()
        ))),
    }
}

fn directory_is_empty(dir: &Path) -> Result<bool, Error> {
    let mut entries = fs::read_dir(dir)
        .map_err(|err| Error::new(format!("无法读取目录（{}）：{err}", dir.display())))?;
    match entries.next() {
        None => Ok(true),
        Some(Ok(_)) => Ok(false),
        Some(Err(err)) => Err(Error::new(format!(
            "无法读取目录（{}）：{err}",
            dir.display()
        ))),
    }
}

/// 把 manifest 原子写到包目录。调用方已经检查过路径和字段。
pub(crate) fn save_manifest(root: &Path, manifest: &Manifest) -> Result<(), Error> {
    let bytes = manifest.to_bytes()?;
    write_manifest_atomic(root, &bytes)
}

/// 镜像在源文件中的一段。`offset` 从文件头算起，长度单位是字节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageSpan {
    pub path: PathBuf,
    pub offset: u64,
    pub length: u64,
}

/// 把若干段按顺序复制到 `images/<id>.img`。先写 `.partial`，完成后改名。
pub(crate) fn install_image(root: &Path, id: &str, spans: &[ImageSpan]) -> Result<(), Error> {
    if spans.is_empty() {
        return Err(Error::new("没有可复制的镜像"));
    }
    if !crate::validate::is_lowercase_uuid(id) {
        return Err(Error::new("分区 id 无效"));
    }
    let images = root.join("images");
    let partial = images.join(format!("{id}.img.partial"));
    let final_path = images.join(format!("{id}.img"));
    let write_result = (|| -> Result<(), Error> {
        let mut output = File::create(&partial)
            .map_err(|err| Error::new(format!("写入镜像失败（{}）：{err}", partial.display())))?;
        for span in spans {
            copy_span(span, &mut output)?;
        }
        output
            .sync_all()
            .map_err(|err| Error::new(format!("写入镜像失败（{}）：{err}", partial.display())))?;
        Ok(())
    })();
    if let Err(err) = write_result {
        let _ = fs::remove_file(&partial);
        return Err(err);
    }
    if let Err(err) = fs::rename(&partial, &final_path) {
        let _ = fs::remove_file(&partial);
        return Err(Error::new(format!(
            "写入镜像失败（{}）：{err}",
            final_path.display()
        )));
    }
    Ok(())
}

fn copy_span(span: &ImageSpan, output: &mut File) -> Result<(), Error> {
    let mut input = File::open(&span.path)
        .map_err(|err| Error::new(format!("无法读取镜像（{}）：{err}", span.path.display())))?;
    input
        .seek(SeekFrom::Start(span.offset))
        .map_err(|err| Error::new(format!("无法读取镜像（{}）：{err}", span.path.display())))?;
    let mut remaining = span.length;
    let mut buffer = vec![0u8; 1024 * 1024];
    while remaining > 0 {
        let chunk = remaining.min(buffer.len() as u64) as usize;
        input.read_exact(&mut buffer[..chunk]).map_err(|err| {
            if err.kind() == ErrorKind::UnexpectedEof {
                Error::new(format!("镜像长度不足（{}）", span.path.display()))
            } else {
                Error::new(format!("无法读取镜像（{}）：{err}", span.path.display()))
            }
        })?;
        output
            .write_all(&buffer[..chunk])
            .map_err(|err| Error::new(format!("写入镜像失败（{}）：{err}", span.path.display())))?;
        remaining -= chunk as u64;
    }
    Ok(())
}

/// 先把内容写到 `manifest.json.tmp` 并 fsync，再改名为 `manifest.json`。
fn write_manifest_atomic(dir: &Path, bytes: &[u8]) -> Result<(), Error> {
    let tmp = dir.join("manifest.json.tmp");
    let final_path = dir.join("manifest.json");
    let write_result = (|| -> Result<(), Error> {
        let mut file = File::create(&tmp)
            .map_err(|err| Error::new(format!("写入 manifest 失败（{}）：{err}", tmp.display())))?;
        file.write_all(bytes)
            .map_err(|err| Error::new(format!("写入 manifest 失败（{}）：{err}", tmp.display())))?;
        file.sync_all()
            .map_err(|err| Error::new(format!("写入 manifest 失败（{}）：{err}", tmp.display())))?;
        Ok(())
    })();
    if let Err(err) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    if let Err(err) = fs::rename(&tmp, &final_path) {
        let _ = fs::remove_file(&tmp);
        return Err(Error::new(format!(
            "写入 manifest 失败（{}）：{err}",
            final_path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::create_package;
    use crate::manifest::{Manifest, Metadata};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("et-pkg-{nanos}-{n}"));
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

    #[test]
    fn creates_manifest_and_empty_images() {
        let tmp = TempDir::new();
        let dir = tmp.path().join("board-d1.etpk");
        let created = create_package(
            &dir,
            Metadata::try_new("board-d1", 16 * 1024 * 1024 * 1024, 512).unwrap(),
        )
        .unwrap();

        assert_eq!(created.root, dir.canonicalize().unwrap());
        assert!(created.root.join("images").is_dir());
        assert!(fs::read_dir(created.root.join("images"))
            .unwrap()
            .next()
            .is_none());
        assert!(!created.root.join("images").join(".trash").exists());
        assert!(!created.root.join("manifest.json.tmp").exists());

        let bytes = fs::read(created.root.join("manifest.json")).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.contains("\"userAreaBytes\": 17179869184"));
        assert!(!text.contains("e+") && !text.contains("E+"));
        let loaded = Manifest::from_slice(&bytes).unwrap();
        assert_eq!(loaded.metadata.name, "board-d1");
        assert!(loaded.partitions.is_empty());
    }

    #[test]
    fn rejects_missing_parent_file_and_non_empty_dir() {
        let tmp = TempDir::new();
        let metadata = Metadata::try_new("board", 68 * 512, 512).unwrap();
        let missing = tmp.path().join("no-such").join("board.etpk");
        assert!(create_package(&missing, metadata.clone())
            .unwrap_err()
            .message()
            .contains("上级目录不存在"));
        assert!(!missing.exists());

        let file_path = tmp.path().join("not-a-dir.etpk");
        fs::write(&file_path, b"x").unwrap();
        assert!(create_package(&file_path, metadata.clone())
            .unwrap_err()
            .message()
            .contains("不是目录"));
        assert_eq!(fs::read(&file_path).unwrap(), b"x");

        let occupied = tmp.path().join("occupied.etpk");
        fs::create_dir(&occupied).unwrap();
        fs::write(occupied.join("keep.txt"), b"keep").unwrap();
        let err = create_package(&occupied, metadata).unwrap_err();
        assert!(err.message().contains("不为空"));
        assert_eq!(fs::read(occupied.join("keep.txt")).unwrap(), b"keep");
        assert!(!occupied.join("manifest.json").exists());
    }

    #[test]
    fn accepts_an_existing_empty_directory() {
        let tmp = TempDir::new();
        let dir = tmp.path().join("empty.etpk");
        fs::create_dir(&dir).unwrap();
        create_package(&dir, Metadata::try_new("empty", 68 * 512, 512).unwrap()).unwrap();
        assert!(dir.join("manifest.json").is_file());
        assert!(dir.join("images").is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink() {
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new();
        let real = tmp.path().join("real");
        fs::create_dir(&real).unwrap();
        let link = tmp.path().join("link.etpk");
        symlink(&real, &link).unwrap();
        let err =
            create_package(&link, Metadata::try_new("link", 68 * 512, 512).unwrap()).unwrap_err();
        assert!(err.message().contains("符号链接"));
        assert!(fs::read_dir(&real).unwrap().next().is_none());
    }

    #[test]
    fn opens_a_created_package_and_rejects_a_missing_manifest() {
        let tmp = TempDir::new();
        let dir = tmp.path().join("board.etpk");
        create_package(
            &dir,
            Metadata::try_new("board", 16 * 1024 * 1024 * 1024, 512).unwrap(),
        )
        .unwrap();
        let opened = super::open_package(&dir).unwrap();
        assert_eq!(opened.manifest.metadata.name, "board");
        assert!(opened.root.ends_with("board.etpk"));

        let empty = tmp.path().join("empty");
        fs::create_dir(&empty).unwrap();
        assert!(super::open_package(&empty)
            .unwrap_err()
            .message()
            .contains("找不到 manifest.json"));

        let file_path = tmp.path().join("not-dir");
        fs::write(&file_path, b"x").unwrap();
        assert!(super::open_package(&file_path)
            .unwrap_err()
            .message()
            .contains("不是目录"));

        let missing = tmp.path().join("missing.etpk");
        assert_eq!(
            super::open_package(&missing).unwrap_err().message(),
            "目录不存在"
        );
    }

    #[test]
    fn rejects_too_many_partitions_and_duplicate_ids() {
        let tmp = TempDir::new();
        let dir = tmp.path().join("wide.etpk");
        fs::create_dir_all(dir.join("images")).unwrap();
        let mut manifest =
            Manifest::new(Metadata::try_new("wide", 16 * 1024 * 1024 * 1024, 512).unwrap());
        for index in 0..129 {
            manifest.partitions.push(crate::manifest::Partition {
                id: format!("00000000-0000-4000-8000-{index:012}"),
                name: format!("p{index}"),
                size_bytes: Some(512),
                start_bytes: None,
                partition_type: "linux-filesystem".to_string(),
                attributes: 0,
                image: None,
                extra: serde_json::Map::new(),
            });
        }
        fs::write(dir.join("manifest.json"), manifest.to_bytes().unwrap()).unwrap();
        assert!(super::open_package(&dir)
            .unwrap_err()
            .message()
            .contains("128"));

        manifest.partitions.truncate(2);
        manifest.partitions[1].id = manifest.partitions[0].id.clone();
        fs::write(dir.join("manifest.json"), manifest.to_bytes().unwrap()).unwrap();
        assert!(super::open_package(&dir)
            .unwrap_err()
            .message()
            .contains("分区 id 重复"));

        manifest.partitions.truncate(1);
        manifest.partitions[0].id.clear();
        fs::write(dir.join("manifest.json"), manifest.to_bytes().unwrap()).unwrap();
        assert_eq!(
            super::open_package(&dir).unwrap_err().message(),
            "分区 id 为空"
        );
    }

    #[test]
    fn install_image_removes_partial_when_the_source_is_short() {
        let tmp = TempDir::new();
        let dir = tmp.path().join("board.etpk");
        let created = create_package(
            &dir,
            Metadata::try_new("board", 16 * 1024 * 1024, 512).unwrap(),
        )
        .unwrap();
        let src = tmp.path().join("short.bin");
        fs::write(&src, b"ab").unwrap();
        let id = "6f1c2a0e-7b4d-4e3a-9c1f-2a8b0d5e6f70";
        let err = super::install_image(
            &created.root,
            id,
            &[super::ImageSpan {
                path: src,
                offset: 0,
                length: 10,
            }],
        )
        .unwrap_err();
        assert!(err.message().contains("长度不足"));
        assert!(!created
            .root
            .join("images")
            .join(format!("{id}.img"))
            .exists());
        assert!(!created
            .root
            .join("images")
            .join(format!("{id}.img.partial"))
            .exists());
    }
}
