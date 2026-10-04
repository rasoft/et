//! 分区编辑。删除会改包里的 manifest 和对应镜像文件。

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::disk;
use crate::error::Error;
use crate::manifest::{Manifest, Partition};
use crate::validate::{inspect_relative, InsidePath};

/// 按 id 删掉分区，并写回 manifest。被删分区的镜像若在包内，一并从 `images/` 删除。
///
/// `ids` 里不认识的 id 会使本次删除失败，磁盘保持原样。固定起点的其余分区保持原起点。
pub fn remove_partitions(
    root: &Path,
    manifest: &Manifest,
    ids: &[String],
) -> Result<Manifest, Error> {
    if ids.is_empty() {
        return Err(Error::new("没有选中的分区"));
    }
    let wanted: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let known: HashSet<&str> = manifest
        .partitions
        .iter()
        .map(|partition| partition.id.as_str())
        .collect();
    for id in &wanted {
        if !known.contains(id) {
            return Err(Error::new(format!("找不到分区：{id}")));
        }
    }

    let removed: Vec<Partition> = manifest
        .partitions
        .iter()
        .filter(|partition| wanted.contains(partition.id.as_str()))
        .cloned()
        .collect();
    let mut updated = manifest.clone();
    updated
        .partitions
        .retain(|partition| !wanted.contains(partition.id.as_str()));
    disk::save_manifest(root, &updated)?;
    for partition in &removed {
        delete_image(root, partition);
    }
    Ok(updated)
}

fn delete_image(root: &Path, partition: &Partition) {
    let Some(image) = partition.image.as_deref() else {
        return;
    };
    if let Ok(InsidePath::File { path, .. }) = inspect_relative(root, image) {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::Map;

    use super::remove_partitions;
    use crate::disk::{create_package, open_package};
    use crate::manifest::{Metadata, Partition};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("et-edit-{nanos}-{n}"));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn partition(id: &str, name: &str, start: Option<u64>, image: bool) -> Partition {
        Partition {
            id: id.to_string(),
            name: name.to_string(),
            size_bytes: Some(1024 * 1024),
            start_bytes: start,
            partition_type: "linux-filesystem".to_string(),
            attributes: 0,
            image: image.then(|| format!("images/{id}.img")),
            extra: Map::new(),
        }
    }

    #[test]
    fn remove_drops_the_partition_and_its_image_and_keeps_a_fixed_start() {
        let tmp = TempDir::new();
        let created = create_package(
            &tmp.0.join("pkg.etpk"),
            Metadata::try_new("board", 16 * 1024 * 1024 * 1024, 512).unwrap(),
        )
        .unwrap();
        let boot = "11111111-1111-4111-8111-111111111111";
        let system = "22222222-2222-4222-8222-222222222222";
        let mut manifest = created.manifest;
        manifest
            .partitions
            .push(partition(boot, "boot", None, true));
        manifest
            .partitions
            .push(partition(system, "system", Some(8 * 1024 * 1024), true));
        let boot_image = created.root.join("images").join(format!("{boot}.img"));
        let system_image = created.root.join("images").join(format!("{system}.img"));
        fs::write(&boot_image, b"boot").unwrap();
        fs::write(&system_image, b"system").unwrap();
        crate::disk::save_manifest(&created.root, &manifest).unwrap();

        let updated = remove_partitions(&created.root, &manifest, &[boot.to_string()]).unwrap();
        assert_eq!(updated.partitions.len(), 1);
        assert_eq!(updated.partitions[0].id, system);
        assert_eq!(updated.partitions[0].start_bytes, Some(8 * 1024 * 1024));
        assert!(!boot_image.exists());
        assert_eq!(fs::read(&system_image).unwrap(), b"system");

        let opened = open_package(&created.root).unwrap();
        assert_eq!(opened.manifest.partitions.len(), 1);
        assert_eq!(opened.manifest.partitions[0].name, "system");

        let err = remove_partitions(&created.root, &opened.manifest, &["missing".to_string()])
            .unwrap_err();
        assert!(err.message().contains("找不到分区"));
        assert!(system_image.exists());
        assert_eq!(
            open_package(&created.root)
                .unwrap()
                .manifest
                .partitions
                .len(),
            1
        );
    }
}
