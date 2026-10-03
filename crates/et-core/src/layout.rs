//! 分区起点计算。
//!
//! 自动分区从可用区按对齐顺延；固定起点不参与重算。界面不实现第二套布局算法。
//! 顺延计算随分区编辑实现。主备 GPT 保留区的大小在这里固定，新建包的容量校验用同一组常数。

/// 主 GPT：保护 MBR + 头 + 128 项条目区。
pub const PRIMARY_GPT_SECTORS: u64 = 34;

/// 备份 GPT：头 + 128 项条目区。
pub const BACKUP_GPT_SECTORS: u64 = 33;

/// 主 GPT 与备份 GPT 占用的字节数。用户区容量必须严格大于这个值。
pub fn gpt_reserved_bytes(sector_size: u64) -> Option<u64> {
    sector_size.checked_mul(PRIMARY_GPT_SECTORS + BACKUP_GPT_SECTORS)
}

#[cfg(test)]
mod tests {
    use super::gpt_reserved_bytes;

    #[test]
    fn standard_512_reserved_is_34_plus_33_sectors() {
        assert_eq!(gpt_reserved_bytes(512), Some(34 * 512 + 33 * 512));
    }
}
