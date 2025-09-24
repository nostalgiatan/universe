//! # UNIV Profile 系统
//!
//! 定义不同的数据模式（Profile），每种模式针对特定的数据类型和使用场景进行优化。

use crate::constants::profile_codes;
use crate::error::{UnivError, Result};
use serde::{Deserialize, Serialize};

/// UNIV 支持的 Profile 类型
/// 
/// 每种 Profile 定义了特定的数据布局、压缩策略和优化方案。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Profile {
    /// 大对象/媒体文件 Profile
    /// 
    /// 适用于存储大型二进制对象，如图片、视频、文档等。
    /// 特点：支持范围映射、CDC分块、仅数据哈希策略。
    Blob,

    /// 结构化记录 Profile
    /// 
    /// 适用于存储结构化的记录数据，支持Schema引用。
    /// 特点：支持字典-字符串压缩、可选列式化、数据哈希策略。
    Recd,

    /// 列式表 Profile
    /// 
    /// 适用于分析型工作负载的列式数据存储。
    /// 特点：强制列式化、支持多种编码（BitPack、RLE、Delta等）。
    Tabl,

    /// 时间序列 Profile
    /// 
    /// 专为时间序列数据优化。
    /// 特点：时间戳Delta编码、Gorilla浮点压缩、时间窗口索引。
    Tsdb,

    /// 图/DAG Profile
    /// 
    /// 适用于图形数据和有向无环图。
    /// 特点：支持外部引用、可达性索引、DAG验证。
    Grph,

    /// 混合 Profile（遗留）
    /// 
    /// 支持混合数据类型，但不推荐在生产环境使用。
    /// 特点：无特定优化、最大兼容性。
    Mixd,

    /// 类型仓库 Profile
    /// 
    /// 用于存储和分发Schema定义和类型信息。
    /// 特点：Manifest管理、依赖解析、签名验证。
    Type,

    /// 自定义 Profile
    /// 
    /// 实验性的自定义Profile，以'X'开头。
    Custom([u8; 4]),
}

impl Profile {
    /// 从4字节代码创建Profile
    /// 
    /// # 参数
    /// 
    /// * `code` - 4字节的Profile代码
    /// 
    /// # 返回
    /// 
    /// 成功时返回对应的Profile，失败时返回错误
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use universe::Profile;
    /// 
    /// let profile = Profile::from_code(b"RECD").unwrap();
    /// assert_eq!(profile, Profile::Recd);
    /// ```
    pub fn from_code(code: &[u8; 4]) -> Result<Self> {
        match code {
            profile_codes::BLOB => Ok(Profile::Blob),
            profile_codes::RECD => Ok(Profile::Recd),
            profile_codes::TABL => Ok(Profile::Tabl),
            profile_codes::TSDB => Ok(Profile::Tsdb),
            profile_codes::GRPH => Ok(Profile::Grph),
            profile_codes::MIXD => Ok(Profile::Mixd),
            profile_codes::TYPE => Ok(Profile::Type),
            custom if custom[0] == b'X' => Ok(Profile::Custom(*custom)),
            unknown => Err(UnivError::UnsupportedProfile { profile: *unknown }),
        }
    }

    /// 获取Profile的4字节代码
    /// 
    /// # 返回
    /// 
    /// Profile对应的4字节代码
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use universe::Profile;
    /// 
    /// let profile = Profile::Recd;
    /// assert_eq!(profile.to_code(), *b"RECD");
    /// ```
    pub fn to_code(&self) -> [u8; 4] {
        match self {
            Profile::Blob => *profile_codes::BLOB,
            Profile::Recd => *profile_codes::RECD,
            Profile::Tabl => *profile_codes::TABL,
            Profile::Tsdb => *profile_codes::TSDB,
            Profile::Grph => *profile_codes::GRPH,
            Profile::Mixd => *profile_codes::MIXD,
            Profile::Type => *profile_codes::TYPE,
            Profile::Custom(code) => *code,
        }
    }

    /// 获取Profile的描述信息
    /// 
    /// # 返回
    /// 
    /// Profile的中文描述
    pub fn description(&self) -> &'static str {
        match self {
            Profile::Blob => "大对象/媒体文件",
            Profile::Recd => "结构化记录",
            Profile::Tabl => "列式表",
            Profile::Tsdb => "时间序列",
            Profile::Grph => "图/DAG",
            Profile::Mixd => "混合（遗留）",
            Profile::Type => "类型仓库",
            Profile::Custom(_) => "自定义",
        }
    }

    /// 检查Profile是否稳定
    /// 
    /// # 返回
    /// 
    /// 如果Profile是稳定版本返回true，否则返回false
    pub fn is_stable(&self) -> bool {
        matches!(
            self,
            Profile::Blob | Profile::Recd | Profile::Tabl | Profile::Tsdb | Profile::Type
        )
    }

    /// 检查Profile是否为实验性质
    /// 
    /// # 返回
    /// 
    /// 如果Profile是实验性的返回true，否则返回false
    pub fn is_experimental(&self) -> bool {
        matches!(self, Profile::Custom(_))
    }

    /// 检查Profile是否为遗留版本
    /// 
    /// # 返回
    /// 
    /// 如果Profile是遗留版本返回true，否则返回false
    pub fn is_legacy(&self) -> bool {
        matches!(self, Profile::Mixd)
    }

    /// 获取Profile支持的Chunk类型
    /// 
    /// # 返回
    /// 
    /// 该Profile支持的Chunk类型列表
    pub fn supported_chunk_kinds(&self) -> Vec<u8> {
        use crate::constants::chunk_kinds::*;
        
        match self {
            Profile::Blob => vec![BLOB, INDEX_SHARD, ATTACHMENT],
            Profile::Recd => vec![DATA_NODE, SCHEMA, STRING_TABLE, INDEX_SHARD, ATTACHMENT],
            Profile::Tabl => vec![DATA_NODE, SCHEMA, STRING_TABLE, INDEX_SHARD],
            Profile::Tsdb => vec![DATA_NODE, SCHEMA, INDEX_SHARD, STRING_TABLE],
            Profile::Grph => vec![DATA_NODE, SCHEMA, STRING_TABLE, INDEX_SHARD, ATTACHMENT],
            Profile::Mixd => vec![DATA_NODE, BLOB, SCHEMA, STRING_TABLE, INDEX_SHARD, ATTACHMENT],
            Profile::Type => vec![SCHEMA, STRING_TABLE, INDEX_SHARD, ATTACHMENT],
            Profile::Custom(_) => vec![DATA_NODE, BLOB, SCHEMA, STRING_TABLE, INDEX_SHARD, ATTACHMENT],
        }
    }

    /// 获取Profile推荐的变换标志
    /// 
    /// # 返回
    /// 
    /// 该Profile推荐使用的变换标志组合
    pub fn recommended_transforms(&self) -> u16 {
        use crate::constants::transform_flags::*;
        
        match self {
            Profile::Blob => CDC,
            Profile::Recd => DICT_STRING | INTEGER_VARINT,
            Profile::Tabl => COLUMNARIZE | BIT_PACK | RLE | DELTA | DICT_STRING,
            Profile::Tsdb => DELTA | GORILLA | INTEGER_VARINT | COLUMNARIZE,
            Profile::Grph => DICT_STRING | INTEGER_VARINT | COLUMNARIZE,
            Profile::Mixd => 0, // 无特定推荐
            Profile::Type => DICT_STRING,
            Profile::Custom(_) => 0, // 由用户定义
        }
    }

    /// 获取Profile的默认哈希策略
    /// 
    /// # 返回
    /// 
    /// 该Profile的默认哈希策略
    pub fn default_hash_policy(&self) -> u8 {
        use crate::constants::hash_policy::*;
        
        match self {
            Profile::Type => PAYLOAD_INCLUSIVE, // TYPE Profile需要包含Schema等元数据
            _ => DATA_ONLY, // 其他Profile默认仅对数据内容哈希
        }
    }

    /// 检查给定的Chunk类型是否被此Profile支持
    /// 
    /// # 参数
    /// 
    /// * `chunk_kind` - 要检查的Chunk类型
    /// 
    /// # 返回
    /// 
    /// 如果支持返回true，否则返回false
    pub fn supports_chunk_kind(&self, chunk_kind: u8) -> bool {
        self.supported_chunk_kinds().contains(&chunk_kind)
    }

    /// 检查给定的变换标志是否被此Profile禁止
    /// 
    /// # 参数
    /// 
    /// * `transform_flags` - 要检查的变换标志
    /// 
    /// # 返回
    /// 
    /// 如果有禁止的变换返回错误，否则返回Ok
    pub fn validate_transforms(&self, transform_flags: u16) -> Result<()> {
        use crate::constants::transform_flags::*;
        
        match self {
            Profile::Tsdb => {
                // TSDB禁止对非时间字段使用Gorilla压缩
                // 这里简化处理，实际实现需要结合Schema信息
                Ok(())
            }
            Profile::Type => {
                // TYPE Profile 禁止某些变换
                if transform_flags & (COLUMNARIZE | CDC | GORILLA) != 0 {
                    return Err(UnivError::InvalidTransform {
                        reason: "TYPE Profile 禁止使用 Columnarize、CDC 或 Gorilla 变换".to_string(),
                    });
                }
                Ok(())
            }
            _ => Ok(()), // 其他Profile暂无特殊限制
        }
    }
}

impl std::fmt::Display for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Profile::Custom(code) => {
                write!(f, "Custom({})", String::from_utf8_lossy(code))
            }
            _ => {
                let code = self.to_code();
                write!(f, "{}", String::from_utf8_lossy(&code))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_from_code() {
        assert_eq!(Profile::from_code(b"RECD").unwrap(), Profile::Recd);
        assert_eq!(Profile::from_code(b"BLOB").unwrap(), Profile::Blob);
        assert_eq!(Profile::from_code(b"TYPE").unwrap(), Profile::Type);
        
        // 测试自定义Profile
        assert_eq!(Profile::from_code(b"XABC").unwrap(), Profile::Custom(*b"XABC"));
        
        // 测试不支持的Profile
        assert!(Profile::from_code(b"UNKN").is_err());
    }

    #[test]
    fn test_profile_to_code() {
        assert_eq!(Profile::Recd.to_code(), *b"RECD");
        assert_eq!(Profile::Blob.to_code(), *b"BLOB");
        assert_eq!(Profile::Type.to_code(), *b"TYPE");
    }

    #[test]
    fn test_profile_properties() {
        assert!(Profile::Recd.is_stable());
        assert!(!Profile::Recd.is_experimental());
        assert!(!Profile::Recd.is_legacy());
        
        assert!(Profile::Mixd.is_legacy());
        assert!(!Profile::Mixd.is_stable());
        
        assert!(Profile::Custom(*b"XABC").is_experimental());
    }

    #[test]
    fn test_supported_chunk_kinds() {
        let recd_chunks = Profile::Recd.supported_chunk_kinds();
        assert!(recd_chunks.contains(&crate::constants::chunk_kinds::DATA_NODE));
        assert!(recd_chunks.contains(&crate::constants::chunk_kinds::SCHEMA));
        
        let blob_chunks = Profile::Blob.supported_chunk_kinds();
        assert!(blob_chunks.contains(&crate::constants::chunk_kinds::BLOB));
        assert!(!blob_chunks.contains(&crate::constants::chunk_kinds::DATA_NODE));
    }

    #[test]
    fn test_transform_validation() {
        // TYPE Profile 不应该支持某些变换
        let result = Profile::Type.validate_transforms(crate::constants::transform_flags::COLUMNARIZE);
        assert!(result.is_err());
        
        // RECD Profile 应该支持大多数变换
        let result = Profile::Recd.validate_transforms(crate::constants::transform_flags::DICT_STRING);
        assert!(result.is_ok());
    }
}