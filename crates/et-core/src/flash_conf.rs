//! `flash.conf` 文本。分区行的列顺序见 `docs/flash_conf.md`。

use crate::error::Error;

pub(crate) const MAX_TEXT_BYTES: u64 = 16 * 1024 * 1024;

const HEADER_KEYS: &[&str] = &[
    "flash_type",
    "block_size",
    "flash_size",
    "table_type",
    "write_protect",
    "crc32",
    "table_version",
    "dtb_file",
    "zlibmode",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlashConfig {
    pub flash_type: Option<String>,
    pub block_size: u64,
    pub flash_size: Option<u64>,
    pub table_type: String,
    pub write_protect: Option<bool>,
    pub crc32: Option<bool>,
    pub table_version: Option<u64>,
    pub dtb_file: Option<String>,
    pub partitions: Vec<FlashPartition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlashPartition {
    pub name: String,
    pub file: Option<String>,
    pub crc: bool,
    pub fs: String,
    pub mode: String,
    pub update: u64,
    pub version: u64,
    pub address: u64,
    pub size: FlashSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FlashSize {
    Fixed(u64),
    Auto,
}

pub(crate) fn parse_flash_conf(text: &str) -> Result<FlashConfig, Error> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut builder = Builder::default();
    let mut seen_partition = false;
    for (index, line) in text.lines().enumerate() {
        let line_no = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = trimmed.split_whitespace().collect();
        if is_header_key(fields[0]) {
            if seen_partition {
                return Err(Error::new(format!(
                    "第 {line_no} 行：分区表之后不能再写配置项"
                )));
            }
            if fields.len() != 2 {
                return Err(Error::new(format!("第 {line_no} 行的配置项格式不正确")));
            }
            builder.apply(fields[0], fields[1], line_no)?;
            continue;
        }
        if fields.len() != 9 {
            return Err(Error::new(format!("第 {line_no} 行应为 9 列")));
        }
        seen_partition = true;
        builder.partitions.push(parse_partition(&fields, line_no)?);
    }
    let Some(block_size) = builder.block_size else {
        return Err(Error::new("缺少 block_size"));
    };
    if block_size == 0 {
        return Err(Error::new("block_size 必须大于 0"));
    }
    Ok(FlashConfig {
        flash_type: builder.flash_type,
        block_size,
        flash_size: builder.flash_size,
        table_type: builder.table_type.unwrap_or_default(),
        write_protect: builder.write_protect,
        crc32: builder.crc32,
        table_version: builder.table_version,
        dtb_file: builder.dtb_file,
        partitions: builder.partitions,
    })
}

#[derive(Default)]
struct Builder {
    flash_type: Option<String>,
    block_size: Option<u64>,
    flash_size: Option<u64>,
    table_type: Option<String>,
    write_protect: Option<bool>,
    crc32: Option<bool>,
    table_version: Option<u64>,
    dtb_file: Option<String>,
    zlibmode: bool,
    partitions: Vec<FlashPartition>,
}

impl Builder {
    fn apply(&mut self, key: &str, value: &str, line_no: usize) -> Result<(), Error> {
        match key {
            "flash_type" => assign_string(&mut self.flash_type, value, key, line_no)?,
            "block_size" => {
                assign_fresh(self.block_size.is_some(), key, line_no)?;
                self.block_size = Some(parse_u64(value, line_no, "block_size")?);
            }
            "flash_size" => {
                assign_fresh(self.flash_size.is_some(), key, line_no)?;
                self.flash_size = Some(parse_u64(value, line_no, "flash_size")?);
            }
            "table_type" => assign_string(&mut self.table_type, value, key, line_no)?,
            "write_protect" => {
                assign_fresh(self.write_protect.is_some(), key, line_no)?;
                self.write_protect = Some(parse_bool(value, line_no, "write_protect")?);
            }
            "crc32" => {
                assign_fresh(self.crc32.is_some(), key, line_no)?;
                self.crc32 = Some(parse_bool(value, line_no, "crc32")?);
            }
            "table_version" => {
                assign_fresh(self.table_version.is_some(), key, line_no)?;
                self.table_version = Some(parse_u64(value, line_no, "table_version")?);
            }
            "dtb_file" => assign_string(&mut self.dtb_file, value, key, line_no)?,
            "zlibmode" => {
                assign_fresh(self.zlibmode, key, line_no)?;
                self.zlibmode = true;
            }
            _ => return Err(Error::new(format!("第 {line_no} 行无法识别"))),
        }
        Ok(())
    }
}

fn parse_partition(fields: &[&str], line_no: usize) -> Result<FlashPartition, Error> {
    let name = fields[0];
    if name.is_empty() || name.contains('\0') {
        return Err(Error::new(format!("第 {line_no} 行的分区名为空")));
    }
    let file = if fields[1].eq_ignore_ascii_case("NULL") {
        None
    } else if fields[1].contains('\0') {
        return Err(Error::new(format!("第 {line_no} 行的文件名无效")));
    } else {
        Some(fields[1].to_string())
    };
    Ok(FlashPartition {
        name: name.to_string(),
        file,
        crc: parse_bool(fields[2], line_no, "CRC")?,
        fs: fields[3].to_string(),
        mode: fields[4].to_string(),
        update: parse_u64(fields[5], line_no, "UPDATE")?,
        version: parse_u64(fields[6], line_no, "VERSION")?,
        address: parse_u64(fields[7], line_no, "ADDRESS")?,
        size: if fields[8].eq_ignore_ascii_case("auto") {
            FlashSize::Auto
        } else {
            FlashSize::Fixed(parse_u64(fields[8], line_no, "SIZE")?)
        },
    })
}

fn is_header_key(key: &str) -> bool {
    HEADER_KEYS.contains(&key)
}

fn assign_fresh(already: bool, key: &str, line_no: usize) -> Result<(), Error> {
    if already {
        Err(Error::new(format!("第 {line_no} 行的配置项重复：{key}")))
    } else {
        Ok(())
    }
}

fn assign_string(
    slot: &mut Option<String>,
    value: &str,
    key: &str,
    line_no: usize,
) -> Result<(), Error> {
    assign_fresh(slot.is_some(), key, line_no)?;
    if value.contains('\0') {
        return Err(Error::new(format!("第 {line_no} 行的 {key} 无效")));
    }
    *slot = Some(value.to_string());
    Ok(())
}

fn parse_u64(text: &str, line_no: usize, field: &str) -> Result<u64, Error> {
    let parsed = if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        if hex.is_empty() {
            None
        } else {
            u64::from_str_radix(hex, 16).ok()
        }
    } else {
        text.parse::<u64>().ok()
    };
    parsed.ok_or_else(|| Error::new(format!("第 {line_no} 行的 {field} 不是整数")))
}

fn parse_bool(text: &str, line_no: usize, field: &str) -> Result<bool, Error> {
    if text.eq_ignore_ascii_case("true") {
        Ok(true)
    } else if text.eq_ignore_ascii_case("false") {
        Ok(false)
    } else {
        Err(Error::new(format!(
            "第 {line_no} 行的 {field} 必须是 true 或 false"
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{parse_flash_conf, FlashSize};

    #[test]
    fn parses_the_documented_emmc_example() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/flash_conf.md");
        let text = fs::read_to_string(&path).unwrap();
        let body = text
            .lines()
            .skip_while(|line| line.trim() != "```")
            .skip(1)
            .take_while(|line| line.trim() != "```")
            .collect::<Vec<_>>()
            .join("\n");
        let conf = parse_flash_conf(&body).unwrap();
        assert_eq!(conf.flash_type.as_deref(), Some("emmc"));
        assert_eq!(conf.block_size, 0x200);
        assert_eq!(conf.flash_size, None);
        assert_eq!(conf.table_type, "gpt");
        assert_eq!(conf.crc32, Some(true));
        assert_eq!(conf.table_version, Some(5));
        assert_eq!(conf.partitions.len(), 23);

        let security = &conf.partitions[0];
        assert_eq!(security.name, "security");
        assert_eq!(security.file.as_deref(), Some("null.img"));
        assert!(security.crc);
        assert_eq!(security.fs, "RAW");
        assert_eq!(security.mode, "ro");
        assert_eq!(security.address, 0x2000);
        assert_eq!(security.size, FlashSize::Fixed(0x2000));

        let vbmeta = conf
            .partitions
            .iter()
            .find(|part| part.name == "vbmeta_a")
            .unwrap();
        assert_eq!(vbmeta.file.as_deref(), Some("vbmeta.img"));
        assert_eq!(vbmeta.address, 0xe000);
        assert_eq!(vbmeta.size, FlashSize::Fixed(0x800));

        let userdata = conf.partitions.last().unwrap();
        assert_eq!(userdata.name, "userdata");
        assert_eq!(userdata.address, 0x841000);
        assert_eq!(userdata.size, FlashSize::Auto);
        assert_eq!(userdata.mode, "ro");
    }

    #[test]
    fn accepts_crlf_null_and_auto() {
        let text = "block_size 512\r\ntable_type GPT\r\n\r\n# comment\r\nboot NULL false RAW rw 0 0 0x100 0x20\r\n";
        let conf = parse_flash_conf(text).unwrap();
        assert_eq!(conf.table_type, "GPT");
        assert_eq!(conf.partitions[0].file, None);
        assert!(!conf.partitions[0].crc);
        assert_eq!(conf.partitions[0].address, 0x100);
        assert_eq!(conf.partitions[0].size, FlashSize::Fixed(0x20));
    }

    #[test]
    fn reports_duplicate_keys_bad_columns_and_missing_block_size() {
        assert!(parse_flash_conf("crc32 true\n")
            .unwrap_err()
            .message()
            .contains("缺少 block_size"));
        assert!(parse_flash_conf("block_size 512\nblock_size 512\n")
            .unwrap_err()
            .message()
            .contains("重复"));
        assert!(parse_flash_conf("block_size 512\nonly-three a b\n")
            .unwrap_err()
            .message()
            .contains("9 列"));
        assert!(
            parse_flash_conf("block_size 512\nboot NULL true RAW ro 0 0 1 2\nblock_size 1\n")
                .unwrap_err()
                .message()
                .contains("分区表之后")
        );
        let conf =
            parse_flash_conf("block_size 0x200\nboot NULL true RAW ro 1 2 3 auto\n").unwrap();
        assert_eq!(conf.partitions[0].size, FlashSize::Auto);
        assert_eq!(conf.partitions[0].update, 1);
        assert_eq!(conf.partitions[0].version, 2);
        assert_eq!(conf.partitions[0].address, 3);
    }
}
