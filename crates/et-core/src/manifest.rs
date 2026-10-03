//! 内存中的文档模型，对应包目录里的 `manifest.json`。
//!
//! 打开时算出的绝对路径、起点和校验项不写回 manifest。同一主版本里不认识的字段
//! 保存在 `extra` 中，下次序列化时原样写回。

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::Error;
use crate::layout::gpt_reserved_bytes;

pub const FORMAT_NAME: &str = "etpack";
pub const FORMAT_VERSION: u32 = 1;
pub const DEFAULT_ALIGNMENT: u64 = 1024 * 1024;
const MAX_NAME_SCALARS: usize = 128;
const MAX_DESCRIPTION_SCALARS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    pub metadata: Metadata,
    pub partitions: Vec<Partition>,
    #[serde(default, flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub name: String,
    pub description: String,
    pub sector_size: u32,
    pub user_area_bytes: u64,
    pub alignment: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot1_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot2_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot1_image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot2_image: Option<String>,
    #[serde(default, flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Partition {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    /// `None` 表示自动排布。序列化时写成 JSON `null`，不省略字段。
    #[serde(default)]
    pub start_bytes: Option<u64>,
    #[serde(rename = "type")]
    pub partition_type: String,
    pub attributes: u64,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default, flatten)]
    pub extra: Map<String, Value>,
}

impl Manifest {
    pub fn new(metadata: Metadata) -> Self {
        Self {
            format: FORMAT_NAME.to_string(),
            format_version: FORMAT_VERSION,
            metadata,
            partitions: Vec::new(),
            extra: Map::new(),
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|err| Error::new(format!("无法生成 manifest：{err}")))?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        let manifest: Self = serde_json::from_slice(bytes)
            .map_err(|err| Error::new(format!("manifest 不是合法的包文件：{err}")))?;
        if manifest.format != FORMAT_NAME {
            return Err(Error::new("format 必须是 etpack"));
        }
        if manifest.format_version != FORMAT_VERSION {
            return Err(Error::new("formatVersion 必须是 1"));
        }
        manifest.metadata.validate()?;
        Ok(manifest)
    }
}

impl Metadata {
    /// 新建包使用的元数据。说明为空，对齐为 1 MiB。
    pub fn try_new(
        name: impl Into<String>,
        user_area_bytes: u64,
        sector_size: u32,
    ) -> Result<Self, Error> {
        let metadata = Self {
            name: name.into(),
            description: String::new(),
            sector_size,
            user_area_bytes,
            alignment: DEFAULT_ALIGNMENT,
            boot1_bytes: None,
            boot2_bytes: None,
            boot1_image: None,
            boot2_image: None,
            extra: Map::new(),
        };
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.name.is_empty() {
            return Err(Error::new("名称为空"));
        }
        if self.name.contains('\0') {
            return Err(Error::new("名称含有空字符"));
        }
        if scalar_len(&self.name) > MAX_NAME_SCALARS {
            return Err(Error::new("名称最长 128 个字符"));
        }
        if self.description.contains('\0') {
            return Err(Error::new("说明含有空字符"));
        }
        if scalar_len(&self.description) > MAX_DESCRIPTION_SCALARS {
            return Err(Error::new("说明最长 4096 个字符"));
        }
        if self.sector_size != 512 && self.sector_size != 4096 {
            return Err(Error::new("扇区大小只能是 512 或 4096"));
        }
        if self.user_area_bytes == 0 {
            return Err(Error::new("容量必须大于 0"));
        }
        let sector = u64::from(self.sector_size);
        if self.user_area_bytes % sector != 0 {
            return Err(Error::new("容量必须是扇区大小的整数倍"));
        }
        let reserved = gpt_reserved_bytes(sector).ok_or_else(|| Error::new("容量计算溢出"))?;
        if self.user_area_bytes <= reserved {
            return Err(Error::new(format!(
                "容量必须大于主 GPT 与备份 GPT 占用的 {reserved} 字节"
            )));
        }
        if self.alignment == 0 || self.alignment % sector != 0 || self.alignment % 512 != 0 {
            return Err(Error::new("对齐必须大于 0，且是扇区大小和 512 的整数倍"));
        }
        Ok(())
    }
}

fn scalar_len(text: &str) -> usize {
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use super::{Manifest, Metadata, DEFAULT_ALIGNMENT};

    const MIN_512: u64 = 68 * 512;

    #[test]
    fn new_package_metadata_uses_one_mib_alignment() {
        let metadata = Metadata::try_new("board-d1", 16 * 1024 * 1024 * 1024, 512).unwrap();
        assert_eq!(metadata.alignment, DEFAULT_ALIGNMENT);
        assert_eq!(metadata.description, "");
        assert!(metadata.boot1_bytes.is_none());
    }

    #[test]
    fn rejects_empty_name_bad_sector_and_capacity() {
        assert!(Metadata::try_new("", MIN_512, 512).is_err());
        assert!(Metadata::try_new("a", MIN_512, 1024).is_err());
        assert!(Metadata::try_new("a", 0, 512).is_err());
        assert!(Metadata::try_new("a", MIN_512 + 1, 512).is_err());
        assert!(Metadata::try_new("a", 67 * 512, 512).is_err());
        assert!(Metadata::try_new("a", MIN_512, 512).is_ok());
        assert!(Metadata::try_new("a", 68 * 4096, 4096).is_ok());
    }

    #[test]
    fn name_limit_counts_unicode_scalars() {
        assert!(Metadata::try_new("中".repeat(128), MIN_512, 512).is_ok());
        let err = Metadata::try_new("中".repeat(129), MIN_512, 512).unwrap_err();
        assert_eq!(err.message(), "名称最长 128 个字符");
    }

    #[test]
    fn description_and_alignment_are_checked() {
        let mut metadata = Metadata::try_new("board", MIN_512, 512).unwrap();
        metadata.description = "x".repeat(4097);
        assert!(metadata.validate().is_err());

        metadata.description.clear();
        metadata.alignment = 1000;
        assert!(metadata.validate().is_err());
        metadata.alignment = 512;
        assert!(metadata.validate().is_ok());
    }

    #[test]
    fn new_manifest_json_matches_the_package_format() {
        let manifest = Manifest::new(Metadata::try_new("board", MIN_512, 512).unwrap());
        let text = String::from_utf8(manifest.to_bytes().unwrap()).unwrap();
        assert_eq!(
            text,
            "{\n  \"format\": \"etpack\",\n  \"formatVersion\": 1,\n  \"metadata\": {\n    \"name\": \"board\",\n    \"description\": \"\",\n    \"sectorSize\": 512,\n    \"userAreaBytes\": 34816,\n    \"alignment\": 1048576\n  },\n  \"partitions\": []\n}\n"
        );
        assert!(!text.as_bytes().starts_with(&[0xEF, 0xBB, 0xBF]));
    }

    #[test]
    fn unknown_fields_roundtrip() {
        let raw = br#"{
          "format": "etpack",
          "formatVersion": 1,
          "vendor": "acme",
          "metadata": {
            "name": "board",
            "description": "",
            "sectorSize": 512,
            "userAreaBytes": 34816,
            "alignment": 1048576,
            "customMeta": {"keep": true}
          },
          "partitions": []
        }"#;
        let manifest = Manifest::from_slice(raw).unwrap();
        assert_eq!(
            manifest
                .extra
                .get("vendor")
                .and_then(|value| value.as_str()),
            Some("acme")
        );
        assert_eq!(
            manifest
                .metadata
                .extra
                .get("customMeta")
                .and_then(|value| value.get("keep"))
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        let again = Manifest::from_slice(&manifest.to_bytes().unwrap()).unwrap();
        assert_eq!(again.extra.get("vendor"), manifest.extra.get("vendor"));
        assert_eq!(
            again.metadata.extra.get("customMeta"),
            manifest.metadata.extra.get("customMeta")
        );
    }

    #[test]
    fn wrong_format_or_version_is_rejected() {
        let wrong_format = br#"{"format":"nope","formatVersion":1,"metadata":{"name":"a","description":"","sectorSize":512,"userAreaBytes":34816,"alignment":1048576},"partitions":[]}"#;
        assert!(Manifest::from_slice(wrong_format)
            .unwrap_err()
            .message()
            .contains("etpack"));
        let wrong_version = br#"{"format":"etpack","formatVersion":2,"metadata":{"name":"a","description":"","sectorSize":512,"userAreaBytes":34816,"alignment":1048576},"partitions":[]}"#;
        assert!(Manifest::from_slice(wrong_version)
            .unwrap_err()
            .message()
            .contains("formatVersion"));
    }

    #[test]
    fn partition_null_fields_are_written_back() {
        let raw = r#"{
          "format": "etpack",
          "formatVersion": 1,
          "metadata": {
            "name": "board",
            "description": "",
            "sectorSize": 512,
            "userAreaBytes": 34816,
            "alignment": 1048576
          },
          "partitions": [{
            "id": "6f1c2a0e-7b4d-4e3a-9c1f-2a8b0d5e6f70",
            "name": "boot",
            "sizeBytes": 512,
            "startBytes": null,
            "type": "linux-filesystem",
            "attributes": 0,
            "image": null
          }]
        }"#;
        let manifest = Manifest::from_slice(raw.as_bytes()).unwrap();
        assert_eq!(manifest.partitions[0].start_bytes, None);
        assert_eq!(manifest.partitions[0].image, None);
        let text = String::from_utf8(manifest.to_bytes().unwrap()).unwrap();
        assert!(text.contains("\"startBytes\": null"));
        assert!(text.contains("\"image\": null"));
    }
}
