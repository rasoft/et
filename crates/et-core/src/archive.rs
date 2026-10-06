//! 把工作副本目录打包成单个 `.etpk` 文件，或从文件解包回目录。
//!
//! 文件是不压缩的 zip。条目是 `manifest.json`、`images/`，以及 `images/` 里的常规文件。
//! `images/.trash/` 和未写完的 `.partial` 不打进包。镜像按原样写入，超过 4 GiB 时用 ZIP64。

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::Error;

const MAX_ENTRIES: usize = 256;

/// 把 `root` 下的工作副本写入 `dest`。先写旁边的临时文件，完成后再替换目标。
/// 成功时返回实际写入的绝对路径。
pub fn pack_package(root: &Path, dest: &Path) -> Result<PathBuf, Error> {
    let root = canonicalize_dir(root)?;
    let dest = destination_path(&root, dest)?;
    let partial = sibling(&dest, ".et-partial");
    let write_result = (|| -> Result<(), Error> {
        let file = File::create(&partial).map_err(|err| {
            Error::new(format!(
                "无法写入 etpk 文件（{}）：{err}",
                partial.display()
            ))
        })?;
        write_zip(&root, file)?;
        Ok(())
    })();
    if let Err(err) = write_result {
        let _ = fs::remove_file(&partial);
        return Err(err);
    }
    replace_file(&partial, &dest)?;
    Ok(dest)
}

/// 把 etpk 文件解包到已经存在的空目录。不跟随符号链接，拒绝越出目标目录的路径。
pub fn unpack_package(file: &Path, dest_dir: &Path) -> Result<(), Error> {
    if file.as_os_str().is_empty() {
        return Err(Error::new("没有指定文件"));
    }
    let meta = match fs::symlink_metadata(file) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Err(Error::new("文件不存在"));
        }
        Err(err) => {
            return Err(Error::new(format!(
                "无法读取 etpk 文件（{}）：{err}",
                file.display()
            )));
        }
    };
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(Error::new("不是 etpk 文件"));
    }
    let dest_dir = canonicalize_dir(dest_dir)?;
    if !directory_is_empty(&dest_dir)? {
        return Err(Error::new("目标目录已存在且不为空"));
    }

    let reader = File::open(file)
        .map_err(|err| Error::new(format!("无法读取 etpk 文件（{}）：{err}", file.display())))?;
    let mut archive =
        ZipArchive::new(reader).map_err(|err| Error::new(format!("无法读取 etpk 文件：{err}")))?;
    if archive.len() > MAX_ENTRIES {
        return Err(Error::new("etpk 文件里的条目过多"));
    }
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| Error::new(format!("无法读取 etpk 文件：{err}")))?;
        if entry.compression() != CompressionMethod::Stored {
            return Err(Error::new("etpk 文件必须是不压缩的 zip"));
        }
        if is_symlink_mode(entry.unix_mode()) {
            return Err(Error::new("etpk 文件含有符号链接"));
        }
        let rel = match entry.enclosed_name() {
            Some(path) => path,
            None => return Err(Error::new("压缩包路径越出目标目录")),
        };
        if !is_plain_relative(&rel) {
            return Err(Error::new("压缩包路径越出目标目录"));
        }
        if is_trash(&rel) {
            continue;
        }
        let target = dest_dir.join(&rel);
        if entry.is_dir() || entry.name().ends_with('/') {
            fs::create_dir_all(&target).map_err(|err| {
                Error::new(format!("无法创建目录（{}）：{err}", target.display()))
            })?;
            ensure_inside(&dest_dir, &target)?;
            continue;
        }
        if target.exists() {
            return Err(Error::new("etpk 文件含有重复路径"));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                Error::new(format!("无法创建目录（{}）：{err}", parent.display()))
            })?;
            ensure_inside(&dest_dir, parent)?;
        }
        let mut output = File::create(&target)
            .map_err(|err| Error::new(format!("无法写入文件（{}）：{err}", target.display())))?;
        io::copy(&mut entry, &mut output)
            .map_err(|err| Error::new(format!("无法写入文件（{}）：{err}", target.display())))?;
        output
            .sync_all()
            .map_err(|err| Error::new(format!("无法写入文件（{}）：{err}", target.display())))?;
    }
    Ok(())
}

fn write_zip(root: &Path, file: File) -> Result<(), Error> {
    let mut zip = ZipWriter::new(file);
    let manifest = root.join("manifest.json");
    let manifest_meta = fs::symlink_metadata(&manifest).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            Error::new("找不到 manifest.json")
        } else {
            Error::new(format!("无法读取 manifest.json：{err}"))
        }
    })?;
    if manifest_meta.file_type().is_symlink() || !manifest_meta.is_file() {
        return Err(Error::new("manifest.json 不是常规文件"));
    }
    write_stored_file(&mut zip, "manifest.json", &manifest, manifest_meta.len())?;

    let images = root.join("images");
    match fs::symlink_metadata(&images) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            return Err(Error::new("images 不是目录"));
        }
        Ok(_) => {
            let dir_options = stored_options(0);
            zip.add_directory("images/", dir_options)
                .map_err(|err| Error::new(format!("无法写入 etpk 文件：{err}")))?;
            let mut names = Vec::new();
            for entry in fs::read_dir(&images)
                .map_err(|err| Error::new(format!("无法读取 images：{err}")))?
            {
                let entry = entry.map_err(|err| Error::new(format!("无法读取 images：{err}")))?;
                names.push(entry.file_name());
            }
            names.sort();
            for name in names {
                let name_text = name
                    .to_str()
                    .ok_or_else(|| Error::new("镜像文件名不是合法的 UTF-8"))?;
                if name_text == ".trash"
                    || name_text.starts_with('.')
                    || name_text.ends_with(".partial")
                {
                    continue;
                }
                let path = images.join(&name);
                let meta = fs::symlink_metadata(&path).map_err(|err| {
                    Error::new(format!("无法读取镜像（{}）：{err}", path.display()))
                })?;
                if meta.file_type().is_symlink() {
                    return Err(Error::new(format!("不打包符号链接：{}", path.display())));
                }
                if meta.is_dir() {
                    return Err(Error::new(format!(
                        "无法打包 images 里的目录：{}",
                        path.display()
                    )));
                }
                if !meta.is_file() {
                    continue;
                }
                let entry_name = format!("images/{name_text}");
                write_stored_file(&mut zip, &entry_name, &path, meta.len())?;
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(Error::new(format!("无法读取 images：{err}"))),
    }

    let finished = zip
        .finish()
        .map_err(|err| Error::new(format!("无法写入 etpk 文件：{err}")))?;
    finished
        .sync_all()
        .map_err(|err| Error::new(format!("无法写入 etpk 文件：{err}")))?;
    Ok(())
}

fn write_stored_file<W: Write + io::Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    path: &Path,
    len: u64,
) -> Result<(), Error> {
    zip.start_file(name, stored_options(len))
        .map_err(|err| Error::new(format!("无法写入 etpk 文件：{err}")))?;
    let mut input = File::open(path)
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    io::copy(&mut input, zip).map_err(|err| Error::new(format!("无法写入 etpk 文件：{err}")))?;
    Ok(())
}

fn stored_options(len: u64) -> FileOptions {
    FileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(len >= 0xFFFF_FFFF)
}

fn canonicalize_dir(dir: &Path) -> Result<PathBuf, Error> {
    if dir.as_os_str().is_empty() {
        return Err(Error::new("没有指定目录"));
    }
    let meta = fs::symlink_metadata(dir).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            Error::new("目录不存在")
        } else {
            Error::new(format!("无法检查目录（{}）：{err}", dir.display()))
        }
    })?;
    if !meta.is_dir() {
        return Err(Error::new("目标路径不是目录"));
    }
    dir.canonicalize()
        .map_err(|err| Error::new(format!("无法解析目录（{}）：{err}", dir.display())))
}

fn destination_path(root: &Path, dest: &Path) -> Result<PathBuf, Error> {
    if dest.as_os_str().is_empty() {
        return Err(Error::new("没有指定文件"));
    }
    let name = match dest.file_name() {
        Some(name) if !name.is_empty() && name != "." && name != ".." => name,
        _ => return Err(Error::new("文件路径无效")),
    };
    let parent = match dest.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    if !parent.exists() {
        return Err(Error::new(format!("上级目录不存在：{}", parent.display())));
    }
    let parent = parent
        .canonicalize()
        .map_err(|err| Error::new(format!("无法解析上级目录（{}）：{err}", parent.display())))?;
    if !parent.is_dir() {
        return Err(Error::new(format!(
            "上级路径不是目录：{}",
            parent.display()
        )));
    }
    if parent == *root || parent.starts_with(root) {
        return Err(Error::new("不能把包保存到工作副本里面"));
    }
    let dest = parent.join(name);
    match fs::symlink_metadata(&dest) {
        Ok(meta) if meta.file_type().is_symlink() => Err(Error::new(format!(
            "目标路径是符号链接：{}",
            dest.display()
        ))),
        Ok(meta) if meta.is_dir() => Err(Error::new(format!("目标路径是目录：{}", dest.display()))),
        Ok(_) => Ok(dest),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(dest),
        Err(err) => Err(Error::new(format!(
            "无法检查文件（{}）：{err}",
            dest.display()
        ))),
    }
}

fn replace_file(partial: &Path, dest: &Path) -> Result<(), Error> {
    if !dest.exists() {
        return fs::rename(partial, dest).map_err(|err| {
            let _ = fs::remove_file(partial);
            Error::new(format!("无法写入 etpk 文件（{}）：{err}", dest.display()))
        });
    }
    let backup = sibling(dest, ".et-bak");
    let _ = fs::remove_file(&backup);
    if let Err(err) = fs::rename(dest, &backup) {
        let _ = fs::remove_file(partial);
        return Err(Error::new(format!(
            "无法写入 etpk 文件（{}）：{err}",
            dest.display()
        )));
    }
    if let Err(err) = fs::rename(partial, dest) {
        let _ = fs::rename(&backup, dest);
        let _ = fs::remove_file(partial);
        return Err(Error::new(format!(
            "无法写入 etpk 文件（{}）：{err}",
            dest.display()
        )));
    }
    let _ = fs::remove_file(&backup);
    Ok(())
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    match name {
        Some(name) => path.with_file_name(format!("{name}{suffix}")),
        None => path.with_extension(suffix.trim_start_matches('.')),
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

fn ensure_inside(root: &Path, path: &Path) -> Result<(), Error> {
    let canonical = path
        .canonicalize()
        .map_err(|err| Error::new(format!("无法解析路径（{}）：{err}", path.display())))?;
    if canonical == *root || canonical.starts_with(root) {
        Ok(())
    } else {
        Err(Error::new("压缩包路径越出目标目录"))
    }
}

fn is_plain_relative(path: &Path) -> bool {
    let mut saw_normal = false;
    for component in path.components() {
        match component {
            Component::Normal(name) if !name.is_empty() && name != "." && name != ".." => {
                saw_normal = true;
            }
            _ => return false,
        }
    }
    saw_normal
}

fn is_trash(path: &Path) -> bool {
    let mut components = path.components();
    matches!(components.next(), Some(Component::Normal(name)) if name == "images")
        && matches!(components.next(), Some(Component::Normal(name)) if name == ".trash")
}

fn is_symlink_mode(mode: Option<u32>) -> bool {
    const S_IFMT: u32 = 0o170000;
    const S_IFLNK: u32 = 0o120000;
    match mode {
        Some(mode) => mode & S_IFMT == S_IFLNK,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{pack_package, unpack_package};
    use crate::disk::create_package;
    use crate::manifest::Metadata;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("et-archive-{nanos}-{n}"));
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
    fn pack_and_unpack_roundtrip_keeps_manifest_and_image_bytes() {
        let tmp = TempDir::new();
        let created = create_package(
            &tmp.0.join("work"),
            Metadata::try_new("board", 16 * 1024 * 1024, 512).unwrap(),
        )
        .unwrap();
        let image = created.root.join("images").join("boot.img");
        fs::write(&image, b"boot-bytes").unwrap();
        fs::create_dir(created.root.join("images").join(".trash")).unwrap();
        fs::write(
            created.root.join("images").join(".trash").join("old.img"),
            b"gone",
        )
        .unwrap();
        fs::write(
            created.root.join("images").join("partial.img.partial"),
            b"nope",
        )
        .unwrap();

        let packed = tmp.0.join("board.etpk");
        let written = pack_package(&created.root, &packed).unwrap();
        assert_eq!(written, packed.canonicalize().unwrap());

        let dest = tmp.0.join("out");
        fs::create_dir(&dest).unwrap();
        unpack_package(&packed, &dest).unwrap();
        assert_eq!(
            fs::read(dest.join("manifest.json")).unwrap(),
            fs::read(created.root.join("manifest.json")).unwrap()
        );
        assert_eq!(
            fs::read(dest.join("images").join("boot.img")).unwrap(),
            b"boot-bytes"
        );
        assert!(!dest.join("images").join(".trash").exists());
        assert!(!dest.join("images").join("partial.img.partial").exists());
    }

    #[test]
    fn pack_refuses_to_write_inside_the_work_directory() {
        let tmp = TempDir::new();
        let created = create_package(
            &tmp.0.join("work"),
            Metadata::try_new("board", 16 * 1024 * 1024, 512).unwrap(),
        )
        .unwrap();
        let err = pack_package(&created.root, &created.root.join("inside.etpk")).unwrap_err();
        assert_eq!(err.message(), "不能把包保存到工作副本里面");
    }

    #[test]
    fn unpack_rejects_paths_that_escape_the_destination() {
        let tmp = TempDir::new();
        let packed = tmp.0.join("bad.etpk");
        let file = fs::File::create(&packed).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("../evil.txt", options).unwrap();
        zip.write_all(b"no").unwrap();
        zip.finish().unwrap();

        let dest = tmp.0.join("out");
        fs::create_dir(&dest).unwrap();
        let err = unpack_package(&packed, &dest).unwrap_err();
        assert_eq!(err.message(), "压缩包路径越出目标目录");
        assert!(!tmp.0.join("evil.txt").exists());
    }

    #[test]
    fn unpack_rejects_a_directory_and_a_non_zip() {
        let tmp = TempDir::new();
        let dest = tmp.0.join("out");
        fs::create_dir(&dest).unwrap();
        let dir = tmp.0.join("dir.etpk");
        fs::create_dir(&dir).unwrap();
        assert_eq!(
            unpack_package(&dir, &dest).unwrap_err().message(),
            "不是 etpk 文件"
        );
        let text = tmp.0.join("note.etpk");
        fs::write(&text, b"hello").unwrap();
        assert!(unpack_package(&text, &dest)
            .unwrap_err()
            .message()
            .contains("无法读取 etpk 文件"));
    }
}
