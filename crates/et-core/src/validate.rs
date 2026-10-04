//! 布局与镜像的校验项。
//!
//! 校验失败不是命令失败：打开仍然成功，错误留在校验列表里。
//! 镜像路径越出包根是例外，打开直接失败，并且不读取包外文件。

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use crate::error::Error;
use crate::layout::{
    backup_reserved_bytes, place_partitions, primary_reserved_bytes, PlacedPartition,
};
use crate::manifest::{Manifest, Partition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub partition_index: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    pub issues: Vec<Issue>,
    pub image_lengths: Vec<Option<u64>>,
}

#[derive(Debug)]
pub enum InsidePath {
    File { path: PathBuf, len: u64 },
    Missing,
    NotFile,
}

#[derive(Debug)]
pub enum InspectError {
    Escape,
    Failed(String),
}

/// 解析包内相对路径。符号链接若指向包外，返回 [`InspectError::Escape`]，不读取目标内容。
pub fn inspect_relative(root: &Path, relative: &str) -> Result<InsidePath, InspectError> {
    if relative.is_empty() || relative.contains('\0') {
        return Err(InspectError::Failed("路径为空或含有空字符".to_string()));
    }
    let mut path = push_lexical(root, root, Path::new(relative))?;
    for _ in 0..16 {
        match fs::symlink_metadata(&path) {
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(InsidePath::Missing),
            Err(err) => {
                return Err(InspectError::Failed(format!(
                    "无法检查路径（{relative}）：{err}"
                )))
            }
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = fs::read_link(&path).map_err(|err| {
                    InspectError::Failed(format!("无法读取符号链接（{relative}）：{err}"))
                })?;
                path = resolve_link(root, &path, &target)?;
            }
            Ok(meta) if meta.is_file() => {
                return Ok(InsidePath::File {
                    len: meta.len(),
                    path,
                })
            }
            Ok(_) => return Ok(InsidePath::NotFile),
        }
    }
    Err(InspectError::Failed("符号链接层数过多".to_string()))
}

pub fn check(root: &Path, manifest: &Manifest) -> Result<CheckReport, Error> {
    let sector_size = u64::from(manifest.metadata.sector_size);
    let placed = place_partitions(
        sector_size,
        manifest.metadata.alignment,
        &manifest.partitions,
    );
    let mut issues = Vec::new();
    check_names_and_types(&manifest.partitions, &mut issues);
    check_layout(manifest, &placed, &mut issues);
    let image_lengths = check_images(root, &manifest.partitions, &mut issues)?;
    check_boot_image(
        root,
        "Boot1 镜像路径",
        manifest.metadata.boot1_image.as_deref(),
    )?;
    check_boot_image(
        root,
        "Boot2 镜像路径",
        manifest.metadata.boot2_image.as_deref(),
    )?;
    Ok(CheckReport {
        issues,
        image_lengths,
    })
}

pub fn placed_partitions(manifest: &Manifest) -> Vec<PlacedPartition> {
    place_partitions(
        u64::from(manifest.metadata.sector_size),
        manifest.metadata.alignment,
        &manifest.partitions,
    )
}

fn check_names_and_types(partitions: &[Partition], issues: &mut Vec<Issue>) {
    let mut seen_names = Vec::new();
    for (index, partition) in partitions.iter().enumerate() {
        if !is_lowercase_uuid(&partition.id) {
            issues.push(issue(index, format!("分区 id 不是 UUID：{}", partition.id)));
        }
        if let Some(message) = partition_name_issue(&partition.name) {
            issues.push(issue(
                index,
                format!("分区 {} 的{message}", display_name(partition)),
            ));
        } else if seen_names.iter().any(|name| name == &partition.name) {
            issues.push(issue(
                index,
                format!("分区 {} 的名称与其他分区重复", partition.name),
            ));
        } else {
            seen_names.push(partition.name.clone());
        }
        if !partition_type_ok(&partition.partition_type) {
            issues.push(issue(
                index,
                format!(
                    "分区 {} 不认识的分区类型：{}",
                    display_name(partition),
                    partition.partition_type
                ),
            ));
        }
    }
}

fn check_layout(manifest: &Manifest, placed: &[PlacedPartition], issues: &mut Vec<Issue>) {
    let sector_size = u64::from(manifest.metadata.sector_size);
    let alignment = manifest.metadata.alignment;
    let primary = primary_reserved_bytes(sector_size).unwrap_or(u64::MAX);
    let backup = backup_reserved_bytes(sector_size).unwrap_or(0);
    let usable_end = manifest
        .metadata
        .user_area_bytes
        .map(|bytes| bytes.saturating_sub(backup));
    let last = manifest.partitions.len().saturating_sub(1);

    for (index, (partition, place)) in manifest.partitions.iter().zip(placed).enumerate() {
        let name = display_name(partition);
        match partition.size_bytes {
            None if index != last => issues.push(issue(
                index,
                format!("分区 {name} 的大小留空时必须是最后一个分区"),
            )),
            None => {}
            Some(0) => issues.push(issue(index, format!("分区 {name} 的大小必须大于 0"))),
            Some(size) if sector_size != 0 && size % sector_size != 0 => issues.push(issue(
                index,
                format!("分区 {name} 的大小必须是扇区大小的整数倍"),
            )),
            Some(_) => {}
        }
        let Some(start) = place.start_bytes else {
            issues.push(issue(index, format!("分区 {name} 无法计算起点")));
            continue;
        };
        if place.start_fixed {
            if start % sector_size != 0 {
                issues.push(issue(
                    index,
                    format!("分区 {name} 的起点必须是扇区大小的整数倍"),
                ));
            }
        } else if alignment != 0 && start % alignment != 0 {
            issues.push(issue(index, format!("分区 {name} 自动排布的起点未按对齐")));
        }
        if start < primary {
            issues.push(issue(
                index,
                format!("分区 {name} 的起点落在主 GPT 保留区内"),
            ));
        }
        match (partition.size_bytes, place.end_bytes, usable_end) {
            (None, _, _) => {}
            (_, None, _) => {
                issues.push(issue(index, format!("分区 {name} 的大小导致偏移溢出")));
            }
            (_, Some(end), Some(limit)) if end > limit => {
                issues.push(issue(index, format!("分区 {name} 超出可用空间")));
            }
            _ => {}
        }
    }

    for left in 0..placed.len() {
        for right in (left + 1)..placed.len() {
            let (Some(left_start), Some(right_start)) =
                (placed[left].start_bytes, placed[right].start_bytes)
            else {
                continue;
            };
            let (Some(left_end), Some(right_end)) = (
                span_end(left_start, manifest.partitions[left].size_bytes),
                span_end(right_start, manifest.partitions[right].size_bytes),
            ) else {
                continue;
            };
            if starts_before(left_start, right_end) && starts_before(right_start, left_end) {
                issues.push(Issue {
                    partition_index: Some(left),
                    message: format!(
                        "分区 {} 与分区 {} 重叠",
                        display_name(&manifest.partitions[left]),
                        display_name(&manifest.partitions[right])
                    ),
                });
            }
        }
    }
}

enum SpanEnd {
    Finite(u64),
    Unbounded,
}

fn span_end(start: u64, size: Option<u64>) -> Option<SpanEnd> {
    match size {
        None => Some(SpanEnd::Unbounded),
        Some(size) => start.checked_add(size).map(SpanEnd::Finite),
    }
}

fn starts_before(start: u64, end: SpanEnd) -> bool {
    match end {
        SpanEnd::Finite(end) => start < end,
        SpanEnd::Unbounded => true,
    }
}

fn check_images(
    root: &Path,
    partitions: &[Partition],
    issues: &mut Vec<Issue>,
) -> Result<Vec<Option<u64>>, Error> {
    let mut lengths = Vec::with_capacity(partitions.len());
    for (index, partition) in partitions.iter().enumerate() {
        let Some(image) = &partition.image else {
            lengths.push(None);
            continue;
        };
        let expected = format!("images/{}.img", partition.id);
        if image != &expected {
            match inspect_relative(root, image) {
                Err(InspectError::Escape) => {
                    return Err(Error::new(format!(
                        "分区 {} 的镜像路径越出包目录：{image}",
                        display_name(partition)
                    )));
                }
                Err(InspectError::Failed(message)) => return Err(Error::new(message)),
                Ok(_) => issues.push(issue(
                    index,
                    format!(
                        "分区 {} 的镜像路径必须是 {expected}",
                        display_name(partition)
                    ),
                )),
            }
            lengths.push(None);
            continue;
        }
        match inspect_relative(root, image) {
            Err(InspectError::Escape) => {
                return Err(Error::new(format!(
                    "分区 {} 的镜像路径越出包目录：{image}",
                    display_name(partition)
                )));
            }
            Err(InspectError::Failed(message)) => return Err(Error::new(message)),
            Ok(InsidePath::Missing) => {
                issues.push(issue(
                    index,
                    format!("分区 {} 的镜像不存在", display_name(partition)),
                ));
                lengths.push(None);
            }
            Ok(InsidePath::NotFile) => {
                issues.push(issue(
                    index,
                    format!("分区 {} 的镜像不是常规文件", display_name(partition)),
                ));
                lengths.push(None);
            }
            Ok(InsidePath::File { len, .. }) => {
                if partition.size_bytes.is_some_and(|size| len > size) {
                    issues.push(issue(
                        index,
                        format!("分区 {} 的镜像大于分区", display_name(partition)),
                    ));
                }
                lengths.push(Some(len));
            }
        }
    }
    Ok(lengths)
}

fn check_boot_image(root: &Path, label: &str, path: Option<&str>) -> Result<(), Error> {
    let Some(path) = path else {
        return Ok(());
    };
    match inspect_relative(root, path) {
        Err(InspectError::Escape) => Err(Error::new(format!("{label}越出包目录：{path}"))),
        Err(InspectError::Failed(message)) => Err(Error::new(message)),
        Ok(_) => Ok(()),
    }
}

fn issue(index: usize, message: String) -> Issue {
    Issue {
        partition_index: Some(index),
        message,
    }
}

fn display_name(partition: &Partition) -> String {
    if partition.name.is_empty() {
        "未命名".to_string()
    } else {
        partition.name.clone()
    }
}

pub(crate) fn partition_name_issue(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("名称不能为空");
    }
    if name.chars().count() > 36 {
        return Some("名称最长 36 个字符");
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Some("名称只能包含字母、数字、-、_、.");
    }
    None
}

fn partition_type_ok(partition_type: &str) -> bool {
    matches!(
        partition_type,
        "linux-filesystem" | "efi-system" | "bios-boot"
    ) || partition_type
        .strip_prefix("guid:")
        .is_some_and(is_lowercase_uuid)
}

pub(crate) fn is_lowercase_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(index, byte)| match index {
        8 | 13 | 18 | 23 => *byte == b'-',
        _ => byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase(),
    })
}

fn push_lexical(root: &Path, base: &Path, relative: &Path) -> Result<PathBuf, InspectError> {
    if !base.starts_with(root) {
        return Err(InspectError::Escape);
    }
    let mut out = base.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if out.as_path() == root || !out.starts_with(root) {
                    return Err(InspectError::Escape);
                }
                out.pop();
                if !out.starts_with(root) {
                    return Err(InspectError::Escape);
                }
            }
            _ => return Err(InspectError::Escape),
        }
    }
    if !out.starts_with(root) {
        Err(InspectError::Escape)
    } else {
        Ok(out)
    }
}

fn resolve_link(root: &Path, link_path: &Path, target: &Path) -> Result<PathBuf, InspectError> {
    if target.is_absolute() {
        let rest = match target.strip_prefix(root) {
            Ok(rest) => rest,
            Err(_) => return Err(InspectError::Escape),
        };
        return push_lexical(root, root, rest);
    }
    let base = link_path.parent().unwrap_or(root);
    push_lexical(root, base, target)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::check;
    use crate::manifest::{Manifest, Metadata, Partition, DEFAULT_ALIGNMENT};
    use serde_json::Map;

    const GIB: u64 = 1024 * 1024 * 1024;
    const MIB: u64 = 1024 * 1024;
    const ID: &str = "6f1c2a0e-7b4d-4e3a-9c1f-2a8b0d5e6f70";

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("et-check-{nanos}-{n}"));
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

    fn package(partitions: Vec<Partition>) -> Manifest {
        let mut manifest = Manifest::new(Metadata::try_new("board", 16 * GIB, 512).unwrap());
        manifest.metadata.alignment = DEFAULT_ALIGNMENT;
        manifest.partitions = partitions;
        manifest
    }

    fn partition(name: &str, size: u64, image: Option<&str>) -> Partition {
        Partition {
            id: ID.to_string(),
            name: name.to_string(),
            size_bytes: Some(size),
            start_bytes: None,
            partition_type: "linux-filesystem".to_string(),
            attributes: 0,
            image: image.map(str::to_string),
            extra: Map::new(),
        }
    }

    #[test]
    fn missing_image_is_an_issue_and_escape_refuses_without_the_file() {
        let tmp = TempDir::new();
        fs::create_dir(tmp.path().join("images")).unwrap();
        let manifest = package(vec![partition(
            "boot",
            64 * MIB,
            Some(&format!("images/{ID}.img")),
        )]);
        let report = check(tmp.path(), &manifest).unwrap();
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("不存在")));
        assert_eq!(report.image_lengths, vec![None]);

        let mut escaped = partition("boot", 64 * MIB, Some("../secret.img"));
        escaped.id = ID.to_string();
        let manifest = package(vec![escaped]);
        let err = check(tmp.path(), &manifest).unwrap_err();
        assert!(err.message().contains("越出包目录"));
        assert!(err.message().contains("../secret.img"));
        assert!(!tmp.path().join("../secret.img").exists());
    }

    #[test]
    fn image_larger_than_partition_is_an_issue() {
        let tmp = TempDir::new();
        let images = tmp.path().join("images");
        fs::create_dir(&images).unwrap();
        fs::write(images.join(format!("{ID}.img")), vec![0_u8; 1024]).unwrap();
        let manifest = package(vec![partition(
            "boot",
            512,
            Some(&format!("images/{ID}.img")),
        )]);
        let report = check(tmp.path(), &manifest).unwrap();
        assert_eq!(report.image_lengths, vec![Some(1024)]);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("大于分区")));
    }

    #[test]
    fn fixed_partition_overlap_and_backup_gpt_are_issues() {
        let mut first = partition("boot", 64 * MIB, None);
        first.id = "11111111-1111-4111-8111-111111111111".to_string();
        let mut second = partition("data", 512, None);
        second.id = "22222222-2222-4222-8222-222222222222".to_string();
        second.start_bytes = Some(1024 * 1024);
        let manifest = package(vec![first, second]);
        let report = check(std::env::temp_dir().as_path(), &manifest).unwrap();
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("重叠")));

        let huge = partition("boot", 16 * GIB, None);
        let manifest = package(vec![huge]);
        let report = check(std::env::temp_dir().as_path(), &manifest).unwrap();
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("超出可用空间")));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_outside_is_an_escape_even_if_the_target_is_missing() {
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new();
        let images = tmp.path().join("images");
        fs::create_dir(&images).unwrap();
        let outside =
            std::env::temp_dir().join(format!("et-outside-missing-{}", std::process::id()));
        symlink(&outside, images.join(format!("{ID}.img"))).unwrap();
        assert!(!outside.exists());
        assert!(!outside.starts_with(tmp.path()));
        let manifest = package(vec![partition(
            "boot",
            64 * MIB,
            Some(&format!("images/{ID}.img")),
        )]);
        let err = check(tmp.path(), &manifest).unwrap_err();
        assert!(err.message().contains("越出包目录"));
    }
}
