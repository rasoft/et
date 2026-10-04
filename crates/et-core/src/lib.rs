//! 包模型、分区布局、校验、镜像复制，以及 flash.conf / download.bin 导入。
//!
//! 磁盘操作收拢在 [`disk`]。其余模块针对内存中的文档做纯计算，便于测试。

#![forbid(unsafe_code)]

pub mod disk;
mod error;
mod flash_conf;
pub mod import;
pub mod layout;
pub mod manifest;
mod merge_bin;
pub mod validate;

pub use error::Error;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn version() -> &'static str {
    VERSION
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_not_empty() {
        assert!(!crate::version().is_empty());
    }
}
