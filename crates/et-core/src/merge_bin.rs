//! genflash `merge` 产生的 download.bin。布局见 `docs/genflash_merge.md`。
//!
//! 索引里的 `uint64` 只保证 4 字节对齐，这里按字节取出，不把文件映射成结构体。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::Error;
use crate::flash_conf::MAX_TEXT_BYTES;

const MAGIC: &[u8; 8] = b"mergebin";
const HEADER_LEN: u64 = 12;
const RECORD_LEN: usize = 272;
const MAX_SUBFILES: i32 = 1024;
const FOOTER_LEN: u64 = 64;
const MAX_BOOT_LEN: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MergeSubfile {
    pub name: String,
    pub start: u64,
    pub len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MergeFile {
    pub payload_len: u64,
    pub subfiles: Vec<MergeSubfile>,
}

pub(crate) fn parse_merge(path: &Path) -> Result<MergeFile, Error> {
    let file_len = file_len(path)?;
    let mut file = File::open(path)
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    let payload_len = payload_len(file_len, &mut file)?;
    if payload_len < HEADER_LEN {
        return Err(Error::new("download.bin 太短，不是 merge 文件"));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    let mut header = [0u8; 12];
    file.read_exact(&mut header)
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    if &header[..8] != MAGIC {
        return Err(Error::new(
            "不是 download.bin（文件头不是 mergebin），也不是 flash.conf",
        ));
    }
    let count = read_i32(&header[8..12]);
    if count <= 0 || count > MAX_SUBFILES {
        return Err(Error::new("download.bin 的子文件数量无效"));
    }
    let count = count as u64;
    let index_len = count
        .checked_mul(RECORD_LEN as u64)
        .ok_or_else(|| Error::new("download.bin 的索引长度溢出"))?;
    let index_end = HEADER_LEN
        .checked_add(index_len)
        .ok_or_else(|| Error::new("download.bin 的索引长度溢出"))?;
    if index_end > payload_len {
        return Err(Error::new("download.bin 的索引超出文件"));
    }
    let mut index = vec![0u8; index_len as usize];
    file.read_exact(&mut index)
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    let mut subfiles = Vec::with_capacity(count as usize);
    for index_slot in 0..count as usize {
        let rec = &index[index_slot * RECORD_LEN..(index_slot + 1) * RECORD_LEN];
        let name = subfile_name(rec)?;
        let start = read_u64(&rec[256..264]);
        let len = read_u64(&rec[264..272]);
        let end = start
            .checked_add(len)
            .ok_or_else(|| Error::new(format!("子文件 {name} 的范围溢出")))?;
        if start < index_end || end > payload_len {
            return Err(Error::new(format!("子文件 {name} 的数据超出 merge 文件")));
        }
        subfiles.push(MergeSubfile { name, start, len });
    }
    Ok(MergeFile {
        payload_len,
        subfiles,
    })
}

pub(crate) fn read_range(path: &Path, offset: u64, len: u64) -> Result<Vec<u8>, Error> {
    if len > MAX_TEXT_BYTES {
        return Err(Error::new("配置文件过大"));
    }
    let len = usize::try_from(len).map_err(|_| Error::new("配置文件过大"))?;
    let mut file = File::open(path)
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf)
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    Ok(buf)
}

fn file_len(path: &Path) -> Result<u64, Error> {
    let meta = std::fs::metadata(path)
        .map_err(|err| Error::new(format!("无法读取文件（{}）：{err}", path.display())))?;
    if !meta.is_file() {
        return Err(Error::new("请选择 flash.conf 或 download.bin 文件"));
    }
    Ok(meta.len())
}

/// 带 `-b` 时文件末尾是 64 字节 `GXBf`。纯 merge 没有这段，返回整个文件长度。
fn payload_len(file_len: u64, file: &mut File) -> Result<u64, Error> {
    if file_len < FOOTER_LEN {
        return Ok(file_len);
    }
    file.seek(SeekFrom::Start(file_len - FOOTER_LEN))
        .map_err(|err| Error::new(format!("无法读取文件：{err}")))?;
    let mut tail = [0u8; FOOTER_LEN as usize];
    file.read_exact(&mut tail)
        .map_err(|err| Error::new(format!("无法读取文件：{err}")))?;
    if &tail[0..4] != b"GXBf" {
        return Ok(file_len);
    }
    let original = read_u64(&tail[12..20]);
    let boot_offset = read_u64(&tail[20..28]);
    let boot_size = read_u64(&tail[28..36]);
    let spans = original
        .checked_add(boot_size)
        .and_then(|sum| sum.checked_add(FOOTER_LEN));
    if boot_offset == original && (1..=MAX_BOOT_LEN).contains(&boot_size) && spans == Some(file_len)
    {
        return Ok(original);
    }
    Err(Error::new("download.bin 的 GXBf 尾部与文件大小不符"))
}

fn subfile_name(record: &[u8]) -> Result<String, Error> {
    let end = record[..256]
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(256);
    let name = std::str::from_utf8(&record[..end]).map_err(|_| Error::new("子文件名不是 UTF-8"))?;
    if name.is_empty() {
        return Err(Error::new("子文件名为空"));
    }
    Ok(name.to_string())
}

fn read_u64(bytes: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(bytes);
    u64::from_le_bytes(buf)
}

fn read_i32(bytes: &[u8]) -> i32 {
    let mut buf = [0u8; 4];
    buf.copy_from_slice(bytes);
    i32::from_le_bytes(buf)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::parse_merge;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("et-merge-{nanos}-{n}"));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
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
            let name_bytes = name.as_bytes();
            assert!(name_bytes.len() < 256);
            record[..name_bytes.len()].copy_from_slice(name_bytes);
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
    fn reads_unaligned_offsets_and_stops_before_the_boot_footer() {
        let tmp = TempDir::new();
        let path = tmp.0.join("download.bin");
        write_merge(
            &path,
            &[("flash.conf", b"block_size 512\n"), ("boot.img", b"XYZ")],
            Some(b"boot-tail"),
        );
        let parsed = parse_merge(&path).unwrap();
        assert_eq!(parsed.subfiles.len(), 2);
        assert_eq!(parsed.subfiles[0].name, "flash.conf");
        assert_eq!(parsed.subfiles[1].name, "boot.img");
        assert_eq!(parsed.subfiles[1].len, 3);
        let file_len = fs::metadata(&path).unwrap().len();
        assert!(parsed.payload_len < file_len);
        assert_eq!(
            parsed.subfiles[1].start + parsed.subfiles[1].len,
            parsed.payload_len
        );
        assert_eq!(parsed.subfiles[0].start % 8, 4);
    }

    #[test]
    fn rejects_a_footer_whose_sizes_do_not_add_up() {
        let tmp = TempDir::new();
        let path = tmp.0.join("download.bin");
        write_merge(&path, &[("flash.conf", b"block_size 512\n")], None);
        let mut bytes = fs::read(&path).unwrap();
        let mut footer = vec![0u8; 64];
        footer[..4].copy_from_slice(b"GXBf");
        bytes.extend_from_slice(&footer);
        fs::write(&path, bytes).unwrap();
        assert!(parse_merge(&path).unwrap_err().message().contains("GXBf"));
    }

    #[test]
    fn rejects_a_range_that_runs_into_the_index() {
        let tmp = TempDir::new();
        let path = tmp.0.join("download.bin");
        write_merge(&path, &[("flash.conf", b"abc")], None);
        let mut bytes = fs::read(&path).unwrap();
        bytes[12 + 256..12 + 264].copy_from_slice(&0u64.to_le_bytes());
        fs::write(&path, bytes).unwrap();
        assert!(parse_merge(&path).unwrap_err().message().contains("超出"));
    }
}
