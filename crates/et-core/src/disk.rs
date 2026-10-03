//! 镜像复制、删除，以及 manifest 的原子改名。
//!
//! 打开包时只做存在性和长度检查，不把镜像内容读进内存。

use std::fs::{self, File};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::manifest::{Manifest, Metadata};

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
}
