//! 分区起点计算。
//!
//! 自动分区从可用区按对齐顺延；固定起点不参与重算。界面不实现第二套布局算法。

use crate::manifest::Partition;

/// 主 GPT：保护 MBR + 头 + 128 项条目区。
pub const PRIMARY_GPT_SECTORS: u64 = 34;

/// 备份 GPT：头 + 128 项条目区。
pub const BACKUP_GPT_SECTORS: u64 = 33;

pub const MAX_PARTITIONS: usize = 128;

/// 主 GPT 与备份 GPT 占用的字节数。用户区容量必须严格大于这个值。
pub fn gpt_reserved_bytes(sector_size: u64) -> Option<u64> {
    sector_size.checked_mul(PRIMARY_GPT_SECTORS + BACKUP_GPT_SECTORS)
}

pub fn primary_reserved_bytes(sector_size: u64) -> Option<u64> {
    sector_size.checked_mul(PRIMARY_GPT_SECTORS)
}

pub fn backup_reserved_bytes(sector_size: u64) -> Option<u64> {
    sector_size.checked_mul(BACKUP_GPT_SECTORS)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedPartition {
    pub start_bytes: Option<u64>,
    pub end_bytes: Option<u64>,
    pub start_fixed: bool,
}

/// 按分区数组顺序计算起点。算不出起点时对应字段为 `None`，由校验解释原因。
pub fn place_partitions(
    sector_size: u64,
    alignment: u64,
    partitions: &[Partition],
) -> Vec<PlacedPartition> {
    let mut cursor = primary_reserved_bytes(sector_size).unwrap_or(u64::MAX);
    let mut placed = Vec::with_capacity(partitions.len());
    for partition in partitions {
        let start_fixed = partition.start_bytes.is_some();
        let start = if let Some(fixed) = partition.start_bytes {
            Some(fixed)
        } else {
            align_up(cursor, alignment)
        };
        let end = start.and_then(|value| value.checked_add(partition.size_bytes));
        if let Some(end) = end {
            cursor = cursor.max(end);
        } else {
            cursor = u64::MAX;
        }
        placed.push(PlacedPartition {
            start_bytes: start,
            end_bytes: end,
            start_fixed,
        });
    }
    placed
}

fn align_up(value: u64, alignment: u64) -> Option<u64> {
    if alignment == 0 || value == u64::MAX {
        return None;
    }
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}

#[cfg(test)]
mod tests {
    use super::{gpt_reserved_bytes, place_partitions};
    use crate::manifest::{Manifest, Metadata, Partition, DEFAULT_ALIGNMENT};
    use serde_json::Map;

    const GIB: u64 = 1024 * 1024 * 1024;
    const MIB: u64 = 1024 * 1024;

    fn partition(id: &str, name: &str, size: u64, start: Option<u64>) -> Partition {
        Partition {
            id: id.to_string(),
            name: name.to_string(),
            size_bytes: size,
            start_bytes: start,
            partition_type: "linux-filesystem".to_string(),
            attributes: 0,
            image: None,
            extra: Map::new(),
        }
    }

    fn place(user_area: u64, partitions: Vec<Partition>) -> Vec<super::PlacedPartition> {
        let _ = user_area;
        place_partitions(512, DEFAULT_ALIGNMENT, &partitions)
    }

    #[test]
    fn standard_512_reserved_is_34_plus_33_sectors() {
        assert_eq!(gpt_reserved_bytes(512), Some(34 * 512 + 33 * 512));
    }

    #[test]
    fn first_auto_partition_starts_at_one_mib() {
        let placed = place(16 * GIB, vec![partition("a", "boot", 64 * MIB, None)]);
        assert_eq!(placed[0].start_bytes, Some(MIB));
        assert!(!placed[0].start_fixed);
        assert_eq!(placed[0].end_bytes, Some(MIB + 64 * MIB));
    }

    #[test]
    fn second_auto_partition_follows_the_first() {
        let placed = place(
            16 * GIB,
            vec![
                partition("a", "boot", 64 * MIB, None),
                partition("b", "system", 2 * GIB, None),
            ],
        );
        assert_eq!(placed[1].start_bytes, Some(MIB + 64 * MIB));
    }

    #[test]
    fn fixed_start_does_not_move_when_an_earlier_partition_grows() {
        let fixed = 8 * MIB;
        let small = place(
            16 * GIB,
            vec![
                partition("a", "boot", 64 * MIB, None),
                partition("b", "data", 512, Some(fixed)),
            ],
        );
        let grown = place(
            16 * GIB,
            vec![
                partition("a", "boot", 128 * MIB, None),
                partition("b", "data", 512, Some(fixed)),
            ],
        );
        assert_eq!(small[1].start_bytes, Some(fixed));
        assert_eq!(grown[1].start_bytes, Some(fixed));
        assert!(grown[1].start_fixed);
    }

    #[test]
    fn one_hundred_twenty_eight_partitions_are_placed() {
        let mut partitions = Vec::new();
        for index in 0..128 {
            partitions.push(partition(
                &format!("id-{index}"),
                &format!("p{index}"),
                512,
                None,
            ));
        }
        let placed = place_partitions(512, DEFAULT_ALIGNMENT, &partitions);
        assert_eq!(placed.len(), 128);
        assert_eq!(placed[127].start_bytes, Some(128 * MIB));
    }

    #[test]
    fn metadata_for_layout_examples_is_valid() {
        assert!(Metadata::try_new("board", 16 * GIB, 512).is_ok());
        let _ = Manifest::new(Metadata::try_new("board", 16 * GIB, 512).unwrap());
    }
}
