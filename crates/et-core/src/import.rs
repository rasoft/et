//! 把 flash.conf 或 genflash merge 的 download.bin 写入已经打开的 `.etpk`。
//!
//! 包目录、名称和说明保持不变，分区和镜像按导入内容替换。
//! `table_type` 为 gpt 时，配置里的地址、大小和 `flash_size` 以 `block_size` 为单位，
//! 这里乘成字节。其他表类型按字节解释。分区起点按配置里的地址固定，不重新自动排布。
//! 最后一条分区的大小可以是 `auto`。包里不写 eMMC 容量，下载时再按开发板的真实容量占满剩余空间。

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Number, Value};

use crate::disk::{self, ImageSpan};
use crate::error::Error;
use crate::flash_conf::{self, FlashConfig, FlashPartition, FlashSize, MAX_TEXT_BYTES};
use crate::layout::MAX_PARTITIONS;
use crate::manifest::{Manifest, Metadata, Partition};
use crate::merge_bin::{self, MergeSubfile};
use crate::validate::{inspect_relative, partition_name_issue, InsidePath};

/// 把源文件里的分区和镜像写入 `root` 上已经打开的包。
///
/// 先完整解析，再复制新镜像。manifest 写成功之后才删除被替换掉的旧镜像。
/// 中途失败时，原来的 manifest 和镜像都还在。
pub fn import_into_package(
    source: &Path,
    root: &Path,
    manifest: &Manifest,
) -> Result<Manifest, Error> {
    let plan = plan(source)?;
    apply_plan(root, manifest, plan)
}

struct Plan {
    sector_size: u32,
    user_area_bytes: Option<u64>,
    partitions: Vec<PlannedPartition>,
    flash_type: Option<String>,
    table_type: String,
    table_version: Option<u64>,
    crc32: Option<bool>,
    write_protect: Option<bool>,
    dtb_file: Option<String>,
}

struct PlannedPartition {
    name: String,
    start_bytes: u64,
    size_bytes: Option<u64>,
    image: Option<Vec<ImageSpan>>,
    extra: Map<String, Value>,
}

fn plan(source: &Path) -> Result<Plan, Error> {
    match classify(source)? {
        Kind::Merge => plan_merge(source),
        Kind::Conf => plan_conf(source),
    }
}

enum Kind {
    Merge,
    Conf,
}

fn classify(path: &Path) -> Result<Kind, Error> {
    let meta = fs::metadata(path).map_err(|err| {
        if err.kind() == ErrorKind::NotFound {
            Error::new(format!("文件不存在：{}", path.display()))
        } else {
            Error::new(format!("无法读取文件（{}）：{err}", path.display()))
        }
    })?;
    if !meta.is_file() {
        return Err(Error::new("请选择 flash.conf 或 download.bin 文件"));
    }
    let mut file = fs::File::open(path)
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    let mut magic = [0u8; 8];
    let read = file
        .read(&mut magic)
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    if read == 8 && &magic == b"mergebin" {
        return Ok(Kind::Merge);
    }
    if meta.len() > MAX_TEXT_BYTES {
        return Err(Error::new(
            "无法识别该文件。需要 flash.conf，或 genflash merge 产生的 download.bin",
        ));
    }
    Ok(Kind::Conf)
}

fn plan_conf(path: &Path) -> Result<Plan, Error> {
    let bytes = fs::read(path)
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    let text = String::from_utf8(bytes).map_err(|_| {
        Error::new("无法识别该文件。需要 flash.conf，或 genflash merge 产生的 download.bin")
    })?;
    let conf = flash_conf::parse_flash_conf(&text)?;
    check_names(&conf.partitions)?;
    let mut images = Vec::with_capacity(conf.partitions.len());
    for part in &conf.partitions {
        images.push(conf_image(path, &conf, part)?);
    }
    finish(conf, images)
}

fn plan_merge(path: &Path) -> Result<Plan, Error> {
    let merge = merge_bin::parse_merge(path)?;
    let first = merge
        .subfiles
        .first()
        .ok_or_else(|| Error::new("download.bin 没有配置文件"))?;
    let bytes = merge_bin::read_range(path, first.start, first.len)?;
    let text =
        String::from_utf8(bytes).map_err(|_| Error::new("merge 文件的第一段不是 flash.conf"))?;
    let conf = flash_conf::parse_flash_conf(&text).map_err(|err| {
        Error::new(format!(
            "merge 文件的第一段不是 flash.conf：{}",
            err.message()
        ))
    })?;
    check_names(&conf.partitions)?;
    let mut pool = image_pool(&merge.subfiles);
    let mut images = Vec::with_capacity(conf.partitions.len());
    for part in &conf.partitions {
        images.push(merge_image(path, &mut pool, part)?);
    }
    finish(conf, images)
}

fn image_pool(subfiles: &[MergeSubfile]) -> HashMap<String, VecDeque<(u64, u64)>> {
    let mut pool = HashMap::new();
    for sub in subfiles.iter().skip(1) {
        pool.entry(sub.name.clone())
            .or_insert_with(VecDeque::new)
            .push_back((sub.start, sub.len));
    }
    pool
}

fn merge_image(
    path: &Path,
    pool: &mut HashMap<String, VecDeque<(u64, u64)>>,
    part: &FlashPartition,
) -> Result<Option<Vec<ImageSpan>>, Error> {
    let Some(file_name) = &part.file else {
        return Ok(None);
    };
    let base = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::new(format!("分区 {} 的镜像文件名无效", part.name)))?;
    let (start, len) = pool
        .get_mut(base)
        .and_then(VecDeque::pop_front)
        .ok_or_else(|| Error::new(format!("merge 文件里没有分区 {} 的镜像 {base}", part.name)))?;
    Ok(Some(vec![ImageSpan {
        path: path.to_path_buf(),
        offset: start,
        length: len,
    }]))
}

fn conf_image(
    conf_path: &Path,
    conf: &FlashConfig,
    part: &FlashPartition,
) -> Result<Option<Vec<ImageSpan>>, Error> {
    let Some(file_name) = &part.file else {
        return Ok(None);
    };
    let mut spans = Vec::new();
    if part.name == "KERNEL" {
        if let Some(dtb) = &conf.dtb_file {
            spans.push(resolve_sibling(conf_path, dtb, &part.name)?);
        }
    }
    spans.push(resolve_sibling(conf_path, file_name, &part.name)?);
    Ok(Some(spans))
}

fn resolve_sibling(conf_path: &Path, file_name: &str, partition: &str) -> Result<ImageSpan, Error> {
    if file_name.contains('\0') {
        return Err(Error::new(format!("分区 {partition} 的镜像路径无效")));
    }
    let relative = Path::new(file_name);
    if relative.is_absolute() {
        return Err(Error::new(format!(
            "分区 {partition} 的镜像路径不能是绝对路径：{file_name}"
        )));
    }
    let dir = conf_path.parent().filter(|dir| !dir.as_os_str().is_empty());
    let full = match dir {
        Some(dir) => dir.join(relative),
        None => relative.to_path_buf(),
    };
    let meta = fs::metadata(&full).map_err(|err| {
        if err.kind() == ErrorKind::NotFound {
            Error::new(format!("找不到分区 {partition} 的镜像文件 {file_name}"))
        } else {
            Error::new(format!(
                "无法读取分区 {partition} 的镜像文件 {file_name}：{err}"
            ))
        }
    })?;
    if !meta.is_file() {
        return Err(Error::new(format!(
            "分区 {partition} 的镜像不是常规文件：{file_name}"
        )));
    }
    Ok(ImageSpan {
        path: full,
        offset: 0,
        length: meta.len(),
    })
}

fn finish(conf: FlashConfig, images: Vec<Option<Vec<ImageSpan>>>) -> Result<Plan, Error> {
    let sector = sector_size(conf.block_size)?;
    let gpt = conf.table_type.eq_ignore_ascii_case("gpt");
    let flash_bytes = match conf.flash_size {
        Some(units) => Some(units_to_bytes(gpt, conf.block_size, units)?),
        None => None,
    };
    let last = conf.partitions.len().saturating_sub(1);
    let mut partitions = Vec::with_capacity(conf.partitions.len());
    for (index, (part, image)) in conf.partitions.into_iter().zip(images).enumerate() {
        let start_bytes = units_to_bytes(gpt, conf.block_size, part.address)?;
        let image_len = match &image {
            Some(spans) => Some(span_len(spans)?),
            None => None,
        };
        let size_bytes = partition_size(gpt, sector, index == last, &part, image_len)?;
        partitions.push(PlannedPartition {
            name: part.name.clone(),
            start_bytes,
            size_bytes,
            image,
            extra: partition_extra(&part),
        });
    }
    let user_area_bytes = user_area(sector, flash_bytes, &partitions)?;
    Ok(Plan {
        sector_size: sector as u32,
        user_area_bytes,
        partitions,
        flash_type: conf.flash_type,
        table_type: conf.table_type,
        table_version: conf.table_version,
        crc32: conf.crc32,
        write_protect: conf.write_protect,
        dtb_file: conf.dtb_file,
    })
}

fn check_names(partitions: &[FlashPartition]) -> Result<(), Error> {
    if partitions.len() > MAX_PARTITIONS {
        return Err(Error::new("分区不能超过 128 条"));
    }
    let mut seen = HashSet::new();
    for part in partitions {
        if let Some(message) = partition_name_issue(&part.name) {
            let shown = if part.name.is_empty() {
                "未命名"
            } else {
                part.name.as_str()
            };
            return Err(Error::new(format!("分区 {shown} 的{message}")));
        }
        if !seen.insert(part.name.clone()) {
            return Err(Error::new(format!(
                "分区 {} 的名称与其他分区重复",
                part.name
            )));
        }
    }
    Ok(())
}

fn sector_size(block_size: u64) -> Result<u64, Error> {
    if block_size == 512 || block_size == 4096 {
        Ok(block_size)
    } else {
        Err(Error::new(format!(
            "block_size 只能是 512 或 4096，当前是 {block_size}"
        )))
    }
}

fn units_to_bytes(gpt: bool, block_size: u64, units: u64) -> Result<u64, Error> {
    if gpt {
        units
            .checked_mul(block_size)
            .ok_or_else(|| Error::new("分区地址或大小溢出"))
    } else {
        Ok(units)
    }
}

fn partition_size(
    gpt: bool,
    sector: u64,
    is_last: bool,
    part: &FlashPartition,
    image_len: Option<u64>,
) -> Result<Option<u64>, Error> {
    match part.size {
        FlashSize::Fixed(units) => {
            let size = units_to_bytes(gpt, sector, units)?;
            if size == 0 {
                return Err(Error::new(format!("分区 {} 的大小必须大于 0", part.name)));
            }
            Ok(Some(size))
        }
        // 容量来自开发板。这里只记下“占满剩余空间”，不根据 flash_size 写死。
        FlashSize::Auto if is_last => Ok(None),
        FlashSize::Auto => {
            let len = image_len.ok_or_else(|| {
                Error::new(format!("分区 {} 的大小是 auto，但没有镜像文件", part.name))
            })?;
            let size = align_up(len, sector).ok_or_else(|| Error::new("分区大小溢出"))?;
            if size == 0 {
                return Err(Error::new(format!("分区 {} 的大小必须大于 0", part.name)));
            }
            Ok(Some(size))
        }
    }
}

fn user_area(
    sector: u64,
    flash_bytes: Option<u64>,
    partitions: &[PlannedPartition],
) -> Result<Option<u64>, Error> {
    if partitions.iter().any(|part| part.size_bytes.is_none()) {
        return Ok(None);
    }
    if let Some(bytes) = flash_bytes {
        if bytes == 0 {
            return Err(Error::new("flash_size 必须大于 0"));
        }
        if bytes % sector != 0 {
            return Err(Error::new("flash_size 必须是扇区大小的整数倍"));
        }
        return Ok(Some(bytes));
    }
    let mut end = 0u64;
    for part in partitions {
        let Some(size) = part.size_bytes else {
            return Ok(None);
        };
        let part_end = part
            .start_bytes
            .checked_add(size)
            .ok_or_else(|| Error::new("分区大小溢出"))?;
        end = end.max(part_end);
    }
    if end == 0 {
        return Err(Error::new("配置里没有 flash_size，也无法从分区推断容量"));
    }
    align_up(end, sector)
        .map(Some)
        .ok_or_else(|| Error::new("容量溢出"))
}

fn span_len(spans: &[ImageSpan]) -> Result<u64, Error> {
    let mut total = 0u64;
    for span in spans {
        total = total
            .checked_add(span.length)
            .ok_or_else(|| Error::new("镜像长度溢出"))?;
    }
    Ok(total)
}

fn align_up(value: u64, alignment: u64) -> Option<u64> {
    if alignment == 0 {
        return None;
    }
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}

fn partition_extra(part: &FlashPartition) -> Map<String, Value> {
    let mut extra = Map::new();
    if let Some(file) = &part.file {
        extra.insert("sourceFile".to_string(), Value::String(file.clone()));
    }
    extra.insert("crc".to_string(), Value::Bool(part.crc));
    extra.insert("fs".to_string(), Value::String(part.fs.clone()));
    extra.insert("mode".to_string(), Value::String(part.mode.clone()));
    extra.insert(
        "update".to_string(),
        Value::Number(Number::from(part.update)),
    );
    extra.insert(
        "version".to_string(),
        Value::Number(Number::from(part.version)),
    );
    extra
}

fn apply_plan(root: &Path, current: &Manifest, plan: Plan) -> Result<Manifest, Error> {
    let mut next = current.clone();
    next.metadata.sector_size = plan.sector_size;
    next.metadata.user_area_bytes = plan.user_area_bytes;
    clear_vendor_keys(&mut next.metadata);
    fill_metadata(&mut next.metadata, &plan);
    next.metadata.validate()?;
    next.partitions.clear();

    let mut installed = InstalledImages {
        root: root.to_path_buf(),
        ids: Vec::new(),
        armed: true,
    };
    for planned in plan.partitions {
        let id = new_partition_id();
        let image = if let Some(spans) = planned.image {
            disk::install_image(root, &id, &spans)?;
            installed.ids.push(id.clone());
            Some(format!("images/{id}.img"))
        } else {
            None
        };
        next.partitions.push(Partition {
            id,
            name: planned.name,
            size_bytes: planned.size_bytes,
            start_bytes: Some(planned.start_bytes),
            partition_type: "linux-filesystem".to_string(),
            attributes: 0,
            image,
            extra: planned.extra,
        });
    }
    disk::save_manifest(root, &next)?;
    installed.disarm();
    remove_replaced_images(root, &current.partitions, &next.partitions);
    Ok(next)
}

fn clear_vendor_keys(metadata: &mut Metadata) {
    for key in [
        "flashType",
        "tableType",
        "tableVersion",
        "crc32",
        "writeProtect",
        "dtbFile",
    ] {
        metadata.extra.remove(key);
    }
}

fn fill_metadata(metadata: &mut Metadata, plan: &Plan) {
    if let Some(value) = &plan.flash_type {
        metadata
            .extra
            .insert("flashType".to_string(), Value::String(value.clone()));
    }
    if !plan.table_type.is_empty() {
        metadata.extra.insert(
            "tableType".to_string(),
            Value::String(plan.table_type.clone()),
        );
    }
    if let Some(value) = plan.table_version {
        metadata.extra.insert(
            "tableVersion".to_string(),
            Value::Number(Number::from(value)),
        );
    }
    if let Some(value) = plan.crc32 {
        metadata
            .extra
            .insert("crc32".to_string(), Value::Bool(value));
    }
    if let Some(value) = plan.write_protect {
        metadata
            .extra
            .insert("writeProtect".to_string(), Value::Bool(value));
    }
    if let Some(value) = &plan.dtb_file {
        metadata
            .extra
            .insert("dtbFile".to_string(), Value::String(value.clone()));
    }
}

fn new_partition_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed) as u128;
    let ticks = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let mixed = ticks.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(n);
    let a = (mixed >> 96) as u32;
    let b = (mixed >> 80) as u16;
    let c = (mixed >> 64) as u16 & 0x0fff;
    let d = ((mixed >> 48) as u16 & 0x3fff) | 0x8000;
    let e = mixed & 0x0000_ffff_ffff_ffff;
    format!("{a:08x}-{b:04x}-4{c:03x}-{d:04x}-{e:012x}")
}

struct InstalledImages {
    root: PathBuf,
    ids: Vec<String>,
    armed: bool,
}

impl InstalledImages {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for InstalledImages {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        for id in &self.ids {
            let images = self.root.join("images");
            let _ = fs::remove_file(images.join(format!("{id}.img")));
            let _ = fs::remove_file(images.join(format!("{id}.img.partial")));
        }
    }
}

fn remove_replaced_images(root: &Path, old: &[Partition], new: &[Partition]) {
    let keep: HashSet<&str> = new
        .iter()
        .filter_map(|partition| partition.image.as_deref())
        .collect();
    for partition in old {
        let Some(image) = partition.image.as_deref() else {
            continue;
        };
        if keep.contains(image) {
            continue;
        }
        if let Ok(InsidePath::File { path, .. }) = inspect_relative(root, image) {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::import_into_package;
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
            let path = std::env::temp_dir().join(format!("et-import-{nanos}-{n}"));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct Imported {
        root: PathBuf,
        manifest: crate::manifest::Manifest,
    }

    fn import_into(parent: &Path, source: &Path, name: &str) -> Imported {
        let root = parent.join(format!("{name}.etpk"));
        let created = create_package(
            &root,
            Metadata::try_new(name, 32 * 1024 * 1024, 512).unwrap(),
        )
        .unwrap();
        let mut current = created.manifest;
        current.metadata.description = "keep".to_string();
        let manifest = import_into_package(source, &created.root, &current).unwrap();
        Imported {
            root: created.root,
            manifest,
        }
    }

    fn write_merge(path: &Path, files: &[(&str, &[u8])], boot: Option<&[u8]>) {
        let count = files.len() as u64;
        let base = 12 + count * 272;
        let mut index = Vec::new();
        let mut data = Vec::new();
        for (name, bytes) in files {
            let start = base + data.len() as u64;
            let mut record = vec![0u8; 272];
            record[..name.len()].copy_from_slice(name.as_bytes());
            record[256..264].copy_from_slice(&start.to_le_bytes());
            record[264..272].copy_from_slice(&(bytes.len() as u64).to_le_bytes());
            index.extend_from_slice(&record);
            data.extend_from_slice(bytes);
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"mergebin");
        out.extend_from_slice(&(files.len() as u32).to_le_bytes());
        out.extend_from_slice(&index);
        out.extend_from_slice(&data);
        if let Some(boot) = boot {
            let original = out.len() as u64;
            out.extend_from_slice(boot);
            let mut footer = vec![0u8; 64];
            footer[..4].copy_from_slice(b"GXBf");
            footer[4..8].copy_from_slice(&1u32.to_le_bytes());
            footer[12..20].copy_from_slice(&original.to_le_bytes());
            footer[20..28].copy_from_slice(&original.to_le_bytes());
            footer[28..36].copy_from_slice(&(boot.len() as u64).to_le_bytes());
            out.extend_from_slice(&footer);
        }
        fs::write(path, out).unwrap();
    }

    #[test]
    fn gpt_conf_uses_block_units_and_copies_images() {
        let tmp = TempDir::new();
        fs::write(tmp.0.join("super.img"), b"SUPER").unwrap();
        let conf = tmp.0.join("flash.conf");
        fs::write(
            &conf,
            "\
flash_type emmc
block_size 0x200
flash_size 0x900000
table_type gpt
crc32 true
table_version 5

super super.img true RAW ro 1 4 0x240800 0x600000
frp NULL false RAW rw 0 0 0x840800 0x800
",
        )
        .unwrap();
        let created = import_into(&tmp.0, &conf, "board");
        assert_eq!(created.manifest.metadata.name, "board");
        assert_eq!(created.manifest.metadata.description, "keep");
        assert_eq!(created.manifest.metadata.sector_size, 512);
        assert_eq!(
            created.manifest.metadata.user_area_bytes,
            Some(0x900000 * 512)
        );
        assert_eq!(created.manifest.partitions.len(), 2);

        let super_part = &created.manifest.partitions[0];
        assert_eq!(super_part.name, "super");
        assert_eq!(super_part.start_bytes, Some(0x240800 * 512));
        assert_eq!(super_part.size_bytes, Some(0x600000 * 512));
        assert!(super_part.start_bytes.is_some());
        let image = super_part.image.as_deref().unwrap();
        assert_eq!(fs::read(created.root.join(image)).unwrap(), b"SUPER");
        assert_eq!(
            super_part
                .extra
                .get("sourceFile")
                .and_then(serde_json::Value::as_str),
            Some("super.img")
        );
        assert_eq!(
            super_part
                .extra
                .get("mode")
                .and_then(serde_json::Value::as_str),
            Some("ro")
        );

        let frp = &created.manifest.partitions[1];
        assert_eq!(frp.name, "frp");
        assert_eq!(frp.start_bytes, Some(0x840800 * 512));
        assert_eq!(frp.size_bytes, Some(0x800 * 512));
        assert!(frp.image.is_none());

        let opened = open_package(&created.root).unwrap();
        assert_eq!(opened.manifest.partitions[0].name, "super");
        assert_eq!(opened.manifest.metadata.name, "board");
    }

    #[test]
    fn raw_table_keeps_byte_addresses_and_rounds_auto_size() {
        let tmp = TempDir::new();
        fs::write(tmp.0.join("mid.bin"), vec![7u8; 100]).unwrap();
        let conf = tmp.0.join("flash.conf");
        fs::write(
            &conf,
            "\
block_size 512
table_type nor
flash_size 0x2000000

mid mid.bin true RAW ro 0 0 0x100000 auto
tail NULL true RAW ro 0 0 0x200000 0x1000
",
        )
        .unwrap();
        let created = import_into(&tmp.0, &conf, "raw");
        assert_eq!(created.manifest.metadata.user_area_bytes, Some(0x2000000));
        assert_eq!(created.manifest.partitions[0].start_bytes, Some(0x100000));
        assert_eq!(created.manifest.partitions[0].size_bytes, Some(512));
        assert_eq!(created.manifest.partitions[1].start_bytes, Some(0x200000));
        assert_eq!(created.manifest.partitions[1].size_bytes, Some(0x1000));
        assert_eq!(
            fs::read(
                created
                    .root
                    .join(created.manifest.partitions[0].image.as_deref().unwrap())
            )
            .unwrap()
            .len(),
            100
        );
    }

    #[test]
    fn kernel_conf_prepends_dtb_but_merge_uses_the_packed_bytes() {
        let tmp = TempDir::new();
        fs::write(tmp.0.join("board.dtb"), [1, 2, 3]).unwrap();
        fs::write(tmp.0.join("kernel.bin"), [4, 5]).unwrap();
        let conf = tmp.0.join("flash.conf");
        fs::write(
            &conf,
            "\
block_size 512
table_type gpt
flash_size 0x10000
dtb_file board.dtb

KERNEL kernel.bin true RAW ro 1 0 0x800 0x400
",
        )
        .unwrap();
        let created = import_into(&tmp.0, &conf, "kern");
        let image = created.manifest.partitions[0].image.as_deref().unwrap();
        assert_eq!(fs::read(created.root.join(image)).unwrap(), [1, 2, 3, 4, 5]);

        let text = fs::read_to_string(&conf).unwrap();
        let bin = tmp.0.join("download.bin");
        write_merge(
            &bin,
            &[("flash.conf", text.as_bytes()), ("kernel.bin", b"PACKED")],
            Some(b"boot"),
        );
        let merged = import_into(&tmp.0, &bin, "merged");
        let image = merged.manifest.partitions[0].image.as_deref().unwrap();
        assert_eq!(fs::read(merged.root.join(image)).unwrap(), b"PACKED");
        assert_eq!(merged.manifest.metadata.name, "merged");
        assert_eq!(merged.manifest.metadata.description, "keep");
    }

    #[test]
    fn duplicate_merge_names_are_consumed_in_order() {
        let tmp = TempDir::new();
        let conf = "\
block_size 512
table_type gpt
flash_size 0x10000

a boot.img true RAW ro 0 0 0x800 0x100
b boot.img true RAW ro 0 0 0x900 0x100
";
        let bin = tmp.0.join("download.bin");
        write_merge(
            &bin,
            &[
                ("flash.conf", conf.as_bytes()),
                ("boot.img", b"first"),
                ("boot.img", b"second"),
            ],
            None,
        );
        let created = import_into(&tmp.0, &bin, "dup");
        let first = created.manifest.partitions[0].image.as_deref().unwrap();
        let second = created.manifest.partitions[1].image.as_deref().unwrap();
        assert_eq!(fs::read(created.root.join(first)).unwrap(), b"first");
        assert_eq!(fs::read(created.root.join(second)).unwrap(), b"second");
    }

    #[test]
    fn missing_image_fails_and_last_auto_does_not_need_flash_size() {
        let tmp = TempDir::new();
        let conf = tmp.0.join("flash.conf");
        fs::write(
            &conf,
            "block_size 512\ntable_type gpt\nflash_size 0x10000\nboot missing.img true RAW ro 0 0 0x800 0x100\n",
        )
        .unwrap();
        let dest = tmp.0.join("missing.etpk");
        let created_empty = create_package(
            &dest,
            Metadata::try_new("missing", 32 * 1024 * 1024, 512).unwrap(),
        )
        .unwrap();
        let err = import_into_package(&conf, &dest, &created_empty.manifest).unwrap_err();
        assert!(err.message().contains("找不到"));
        let opened = open_package(&dest).unwrap();
        assert_eq!(opened.manifest.metadata.name, "missing");
        assert!(opened.manifest.partitions.is_empty());

        fs::write(
            &conf,
            "block_size 512\ntable_type gpt\nboot NULL true RAW ro 0 0 0x800 0x100\ndata NULL true RAW ro 0 0 0x900 auto\n",
        )
        .unwrap();
        let manifest = import_into_package(&conf, &dest, &opened.manifest).unwrap();
        let created = Imported {
            root: dest.clone(),
            manifest,
        };
        assert_eq!(created.manifest.metadata.user_area_bytes, None);
        assert_eq!(created.manifest.partitions[0].size_bytes, Some(0x100 * 512));
        assert_eq!(created.manifest.partitions[1].name, "data");
        assert_eq!(created.manifest.partitions[1].size_bytes, None);
        assert_eq!(
            created.manifest.partitions[1].start_bytes,
            Some(0x900 * 512)
        );
        assert!(crate::validate::check(&created.root, &created.manifest)
            .unwrap()
            .issues
            .is_empty());
        let opened = open_package(&dest).unwrap();
        assert_eq!(opened.manifest.metadata.user_area_bytes, None);
        assert_eq!(opened.manifest.partitions[1].size_bytes, None);
    }

    #[test]
    fn last_auto_partition_stays_open_even_when_flash_size_is_present() {
        let tmp = TempDir::new();
        let conf = tmp.0.join("flash.conf");
        fs::write(
            &conf,
            "block_size 0x200\ntable_type gpt\nflash_size 0x10000\ndata NULL true RAW rw 0 0 0x800 auto\n",
        )
        .unwrap();
        let created = import_into(&tmp.0, &conf, "auto");
        let part = &created.manifest.partitions[0];
        assert_eq!(created.manifest.metadata.user_area_bytes, None);
        assert_eq!(part.start_bytes, Some(0x800 * 512));
        assert_eq!(part.size_bytes, None);
        assert!(crate::validate::check(&created.root, &created.manifest)
            .unwrap()
            .issues
            .is_empty());
        assert_eq!(
            part.extra.get("mode").and_then(serde_json::Value::as_str),
            Some("rw")
        );
    }

    #[test]
    fn import_replaces_partitions_and_keeps_the_open_package() {
        let tmp = TempDir::new();
        let root = tmp.0.join("board.etpk");
        let mut created = create_package(
            &root,
            Metadata::try_new("board", 32 * 1024 * 1024, 512).unwrap(),
        )
        .unwrap();
        created.manifest.metadata.description = "保留说明".to_string();
        let old_id = "11111111-2222-4333-8444-555555555555";
        let old_image = format!("images/{old_id}.img");
        fs::write(root.join(&old_image), b"OLD").unwrap();
        created.manifest.partitions.push(Partition {
            id: old_id.to_string(),
            name: "old".to_string(),
            size_bytes: Some(512),
            start_bytes: Some(1024 * 1024),
            partition_type: "linux-filesystem".to_string(),
            attributes: 0,
            image: Some(old_image.clone()),
            extra: serde_json::Map::new(),
        });
        crate::disk::save_manifest(&root, &created.manifest).unwrap();

        let bad = tmp.0.join("bad.conf");
        fs::write(&bad, b"not a conf").unwrap();
        let err = import_into_package(&bad, &root, &created.manifest).unwrap_err();
        assert!(err.message().contains("9 列") || err.message().contains("无法识别"));
        assert_eq!(fs::read(root.join(&old_image)).unwrap(), b"OLD");
        assert_eq!(
            open_package(&root).unwrap().manifest.partitions[0].name,
            "old"
        );

        fs::write(tmp.0.join("boot.img"), b"NEW").unwrap();
        let conf = tmp.0.join("flash.conf");
        fs::write(
            &conf,
            "block_size 512\ntable_type gpt\nflash_size 0x10000\nboot boot.img true RAW ro 0 0 0x800 0x100\n",
        )
        .unwrap();
        let manifest = import_into_package(&conf, &root, &created.manifest).unwrap();
        assert_eq!(manifest.metadata.name, "board");
        assert_eq!(manifest.metadata.description, "保留说明");
        assert_eq!(manifest.partitions.len(), 1);
        assert_eq!(manifest.partitions[0].name, "boot");
        assert!(!root.join(&old_image).exists());
        let image = manifest.partitions[0].image.as_deref().unwrap();
        assert_eq!(fs::read(root.join(image)).unwrap(), b"NEW");
        assert_eq!(open_package(&root).unwrap().manifest.metadata.name, "board");
    }
}
