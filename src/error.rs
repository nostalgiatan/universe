//! # UNIV 错误处理
//!
//! 定义 UNIV 库中所有可能的错误类型和处理机制。

use thiserror::Error;

/// UNIV 库的结果类型别名
pub type Result<T> = std::result::Result<T, UnivError>;

/// UNIV 库的错误类型
#[derive(Error, Debug)]
pub enum UnivError {
    /// 无效的魔数
    #[error("无效的 UNIV 魔数: 期望 {expected:?}, 实际 {actual:?}")]
    InvalidMagic { expected: [u8; 4], actual: [u8; 4] },

    /// 不支持的 Profile 类型
    #[error("不支持的 Profile 类型: {profile:?}")]
    UnsupportedProfile { profile: [u8; 4] },

    /// 无效的文件头
    #[error("无效的文件头: {reason}")]
    InvalidHeader { reason: String },

    /// 块解析错误
    #[error("块解析失败: {reason}")]
    ChunkParseError { reason: String },

    /// 压缩/解压缩错误
    #[error("压缩操作失败: {reason}")]
    CompressionError { reason: String },

    /// 哈希验证失败
    #[error("哈希验证失败: 期望 {expected}, 实际 {actual}")]
    HashMismatch { expected: String, actual: String },

    /// CRC 校验失败
    #[error("CRC 校验失败: 期望 {expected:08x}, 实际 {actual:08x}")]
    CrcMismatch { expected: u32, actual: u32 },

    /// 超出安全限制
    #[error("超出安全限制: {limit_type} 超过 {max_value}")]
    SecurityLimitExceeded { limit_type: String, max_value: u64 },

    /// 引用循环检测
    #[error("检测到引用循环: {node_id}")]
    CircularReference { node_id: String },

    /// 引用深度超限
    #[error("引用深度超限: 当前深度 {current}, 最大允许 {max}")]
    ReferenceDepthExceeded { current: u32, max: u32 },

    /// 无效的变换配置
    #[error("无效的变换配置: {reason}")]
    InvalidTransform { reason: String },

    /// Schema 相关错误
    #[error("Schema 错误: {reason}")]
    SchemaError { reason: String },

    /// 索引损坏或缺失
    #[error("索引错误: {reason}")]
    IndexError { reason: String },

    /// 不支持的编解码器
    #[error("不支持的编解码器: {codec}")]
    UnsupportedCodec { codec: u8 },

    /// 不支持的哈希算法
    #[error("不支持的哈希算法: {algorithm}")]
    UnsupportedHashAlgorithm { algorithm: u8 },

    /// 版本不兼容
    #[error("版本不兼容: 文件版本 {file_version}, 库版本 {lib_version}")]
    VersionIncompatible { file_version: String, lib_version: String },

    /// 数据截断或不完整
    #[error("数据不完整: 期望 {expected} 字节, 实际 {actual} 字节")]
    IncompleteData { expected: usize, actual: usize },

    /// 无效的 UTF-8 编码
    #[error("无效的 UTF-8 编码: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    /// 无效的 CBOR 数据
    #[error("CBOR 解析失败: {0}")]
    CborError(#[from] ciborium::de::Error<std::io::Error>),

    /// I/O 错误
    #[error("I/O 错误: {0}")]
    IoError(#[from] std::io::Error),

    /// 序列化错误
    #[error("序列化错误: {reason}")]
    SerializationError { reason: String },

    /// 反序列化错误
    #[error("反序列化错误: {reason}")]
    DeserializationError { reason: String },

    /// 通用运行时错误
    #[error("运行时错误: {0}")]
    RuntimeError(#[from] anyhow::Error),
}

impl UnivError {
    /// 创建一个无效头部错误
    pub fn invalid_header<S: Into<String>>(reason: S) -> Self {
        Self::InvalidHeader { reason: reason.into() }
    }

    /// 创建一个块解析错误
    pub fn chunk_parse_error<S: Into<String>>(reason: S) -> Self {
        Self::ChunkParseError { reason: reason.into() }
    }

    /// 创建一个压缩错误
    pub fn compression_error<S: Into<String>>(reason: S) -> Self {
        Self::CompressionError { reason: reason.into() }
    }

    /// 创建一个安全限制错误
    pub fn security_limit_exceeded<S: Into<String>>(limit_type: S, max_value: u64) -> Self {
        Self::SecurityLimitExceeded {
            limit_type: limit_type.into(),
            max_value,
        }
    }

    /// 创建一个Schema错误
    pub fn schema_error<S: Into<String>>(reason: S) -> Self {
        Self::SchemaError { reason: reason.into() }
    }

    /// 创建一个索引错误
    pub fn index_error<S: Into<String>>(reason: S) -> Self {
        Self::IndexError { reason: reason.into() }
    }

    /// 创建一个序列化错误
    pub fn serialization_error<S: Into<String>>(reason: S) -> Self {
        Self::SerializationError { reason: reason.into() }
    }

    /// 创建一个反序列化错误
    pub fn deserialization_error<S: Into<String>>(reason: S) -> Self {
        Self::DeserializationError { reason: reason.into() }
    }

    /// 检查是否为安全相关错误
    pub fn is_security_error(&self) -> bool {
        matches!(
            self,
            Self::SecurityLimitExceeded { .. }
                | Self::CircularReference { .. }
                | Self::ReferenceDepthExceeded { .. }
                | Self::HashMismatch { .. }
                | Self::CrcMismatch { .. }
        )
    }

    /// 检查是否为数据格式错误
    pub fn is_format_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidMagic { .. }
                | Self::InvalidHeader { .. }
                | Self::ChunkParseError { .. }
                | Self::IncompleteData { .. }
                | Self::InvalidUtf8(_)
                | Self::CborError(_)
        )
    }

    /// 检查是否为配置错误
    pub fn is_configuration_error(&self) -> bool {
        matches!(
            self,
            Self::UnsupportedProfile { .. }
                | Self::UnsupportedCodec { .. }
                | Self::UnsupportedHashAlgorithm { .. }
                | Self::InvalidTransform { .. }
                | Self::VersionIncompatible { .. }
        )
    }
}

impl From<serde_json::Error> for UnivError {
    fn from(err: serde_json::Error) -> Self {
        Self::DeserializationError { reason: format!("JSON 处理错误: {}", err) }
    }
}