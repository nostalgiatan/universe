//! # UNIV 格式常量定义
//!
//! 包含所有 UNIV 容器格式的常量定义，包括魔数、枚举值、限制等。

/// UNIV 格式的魔数（前4字节）
pub const MAGIC: &[u8; 4] = b"UNV1";

/// 支持的 Profile 代码
pub mod profile_codes {
    /// 大对象/媒体文件 Profile
    pub const BLOB: &[u8; 4] = b"BLOB";
    /// 结构化记录 Profile
    pub const RECD: &[u8; 4] = b"RECD";
    /// 列式表 Profile
    pub const TABL: &[u8; 4] = b"TABL";
    /// 时间序列 Profile
    pub const TSDB: &[u8; 4] = b"TSDB";
    /// 图/DAG Profile
    pub const GRPH: &[u8; 4] = b"GRPH";
    /// 混合 Profile（遗留）
    pub const MIXD: &[u8; 4] = b"MIXD";
    /// 类型仓库 Profile
    pub const TYPE: &[u8; 4] = b"TYPE";
}

/// 文件头标志位
pub mod header_flags {
    /// 包含头部扩展
    pub const HAS_HEADER_EXT: u16 = 0x1;
    /// 流式模式，无 TOC
    pub const STREAMED_WITHOUT_TOC: u16 = 0x2;
    /// 包含加密块
    pub const CONTAINS_ENCRYPTED_CHUNKS: u16 = 0x4;
    /// 包含签名
    pub const CONTAINS_SIGNATURES: u16 = 0x8;
    /// Profile 次版本在头部扩展中
    pub const PROFILE_MINOR_IN_HEADER_EXT: u16 = 0x10;
}

/// 头部扩展 TLV 类型
pub mod header_ext_types {
    /// 生产者信息（UTF-8）
    pub const PRODUCER: u8 = 1;
    /// 创建时间戳（uint64 纳秒）
    pub const CREATION_TIMESTAMP: u8 = 2;
    /// 应用提示（UTF-8）
    pub const APP_HINT: u8 = 3;
    /// 默认编解码器（uint8+参数）
    pub const DEFAULT_CODEC: u8 = 4;
    /// 默认哈希算法
    pub const DEFAULT_HASH_ALG: u8 = 5;
    /// Profile 次版本/选项（CBOR）
    pub const PROFILE_MINOR_OPTIONS: u8 = 6;
    /// 命名空间根（UTF-8）
    pub const NAMESPACE_ROOT: u8 = 10;
    /// 解析器提示（CBOR）
    pub const RESOLVER_HINTS: u8 = 11;
    /// 时间窗口大小秒数（TSDB）
    pub const WINDOW_SIZE_SECONDS: u8 = 12;
    /// 列组提示（TABL）
    pub const COLUMN_GROUP_HINT: u8 = 13;
}

/// Chunk 类型标识
pub mod chunk_kinds {
    /// 数据节点
    pub const DATA_NODE: u8 = 1;
    /// 二进制大对象
    pub const BLOB: u8 = 2;
    /// Schema 定义
    pub const SCHEMA: u8 = 3;
    /// 字符串表
    pub const STRING_TABLE: u8 = 4;
    /// 索引分片
    pub const INDEX_SHARD: u8 = 5;
    /// 附件
    pub const ATTACHMENT: u8 = 6;
}

/// 压缩算法标识
pub mod codecs {
    /// 无压缩
    pub const NONE: u8 = 0;
    /// Zstandard 压缩（推荐）
    pub const ZSTD: u8 = 1;
    /// LZ4 压缩
    pub const LZ4: u8 = 2;
    /// Deflate 压缩
    pub const DEFLATE: u8 = 3;
}

/// 变换标志位
pub mod transform_flags {
    /// 字典-字符串变换
    pub const DICT_STRING: u16 = 0x1;
    /// 整数可变长编码
    pub const INTEGER_VARINT: u16 = 0x2;
    /// 列式化变换
    pub const COLUMNARIZE: u16 = 0x4;
    /// BitPack 变换
    pub const BIT_PACK: u16 = 0x8;
    /// 行程长度编码
    pub const RLE: u16 = 0x10;
    /// Delta 编码
    pub const DELTA: u16 = 0x20;
    /// Gorilla 浮点压缩
    pub const GORILLA: u16 = 0x40;
    /// 内容定义分块（CDC）
    pub const CDC: u16 = 0x80;
}

/// 哈希算法标识
pub mod hash_algorithms {
    /// BLAKE3-256（默认推荐）
    pub const BLAKE3: u8 = 1;
    /// SHA-256
    pub const SHA256: u8 = 2;
    /// CRC32C（仅用于校验）
    pub const CRC32C: u8 = 3;
}

/// 哈希策略
pub mod hash_policy {
    /// 仅数据内容
    pub const DATA_ONLY: u8 = 0;
    /// 包含负载元数据
    pub const PAYLOAD_INCLUSIVE: u8 = 1;
}

/// 默认安全限制
pub mod security_limits {
    /// 最大块数量
    pub const MAX_CHUNKS: u32 = 1_000_000;
    /// 最大原始数据大小（字节）
    pub const MAX_RAW_SIZE: u64 = 256 * 1024 * 1024 * 1024; // 256 GiB
    /// 最大单个块原始大小（字节）
    pub const MAX_CHUNK_RAW: u32 = 32 * 1024 * 1024; // 32 MiB
    /// 最大引用深度
    pub const MAX_REF_DEPTH: u32 = 1024;
    /// 最大字符串表大小（字节）
    pub const MAX_STRING_TABLE: u32 = 256 * 1024 * 1024; // 256 MiB
    /// 最大时间序列窗口跨度倍数
    pub const MAX_SERIES_WINDOW_SPAN: u32 = 10;
    /// 压缩膨胀率阈值
    pub const COMPRESSION_RATIO_THRESHOLD: f32 = 32.0;
}

/// 块大小建议
pub mod chunk_size {
    /// 推荐块大小
    pub const RECOMMENDED: u32 = 256 * 1024; // 256 KiB
    /// 最小块大小
    pub const MIN: u32 = 64 * 1024; // 64 KiB
    /// 最大块大小
    pub const MAX: u32 = 4 * 1024 * 1024; // 4 MiB
}

/// Chunk 帧结构的固定部分长度
pub const CHUNK_FRAME_HEADER_SIZE: usize = 4 + 1 + 1 + 2 + 4 + 4 + 1 + 2; // "CK01" + 其他固定字段
/// Chunk 帧尾部 CRC32C 长度
pub const CHUNK_FRAME_CRC_SIZE: usize = 4;

/// TOC Footer 标识
pub const TOC_MAGIC: &[u8; 4] = b"TOC1";