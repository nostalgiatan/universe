//! # UNIV Chunk 系统
//!
//! 处理 UNIV 容器中的数据块，包括压缩、变换、哈希验证等功能。

use crate::constants::{chunk_kinds, codecs, CHUNK_FRAME_HEADER_SIZE, CHUNK_FRAME_CRC_SIZE};
use crate::error::{UnivError, Result};
use crate::util::hash::HashProvider;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};

/// Chunk 类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChunkKind {
    /// 数据节点
    DataNode,
    /// 二进制大对象
    Blob,
    /// Schema 定义
    Schema,
    /// 字符串表
    StringTable,
    /// 索引分片
    IndexShard,
    /// 附件
    Attachment,
    /// 未知类型
    Unknown(u8),
}

impl ChunkKind {
    /// 从字节值创建 ChunkKind
    pub fn from_u8(value: u8) -> Self {
        match value {
            chunk_kinds::DATA_NODE => ChunkKind::DataNode,
            chunk_kinds::BLOB => ChunkKind::Blob,
            chunk_kinds::SCHEMA => ChunkKind::Schema,
            chunk_kinds::STRING_TABLE => ChunkKind::StringTable,
            chunk_kinds::INDEX_SHARD => ChunkKind::IndexShard,
            chunk_kinds::ATTACHMENT => ChunkKind::Attachment,
            unknown => ChunkKind::Unknown(unknown),
        }
    }

    /// 转换为字节值
    pub fn to_u8(&self) -> u8 {
        match self {
            ChunkKind::DataNode => chunk_kinds::DATA_NODE,
            ChunkKind::Blob => chunk_kinds::BLOB,
            ChunkKind::Schema => chunk_kinds::SCHEMA,
            ChunkKind::StringTable => chunk_kinds::STRING_TABLE,
            ChunkKind::IndexShard => chunk_kinds::INDEX_SHARD,
            ChunkKind::Attachment => chunk_kinds::ATTACHMENT,
            ChunkKind::Unknown(value) => *value,
        }
    }

    /// 获取 ChunkKind 的描述
    pub fn description(&self) -> &'static str {
        match self {
            ChunkKind::DataNode => "数据节点",
            ChunkKind::Blob => "二进制大对象",
            ChunkKind::Schema => "Schema定义",
            ChunkKind::StringTable => "字符串表",
            ChunkKind::IndexShard => "索引分片",
            ChunkKind::Attachment => "附件",
            ChunkKind::Unknown(_) => "未知类型",
        }
    }
}

/// 压缩算法枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Codec {
    /// 无压缩
    None,
    /// Zstandard 压缩
    Zstd,
    /// LZ4 压缩
    Lz4,
    /// Deflate 压缩
    Deflate,
    /// 未知压缩算法
    Unknown(u8),
}

impl Codec {
    /// 从字节值创建 Codec
    pub fn from_u8(value: u8) -> Self {
        match value {
            codecs::NONE => Codec::None,
            codecs::ZSTD => Codec::Zstd,
            codecs::LZ4 => Codec::Lz4,
            codecs::DEFLATE => Codec::Deflate,
            unknown => Codec::Unknown(unknown),
        }
    }

    /// 转换为字节值
    pub fn to_u8(&self) -> u8 {
        match self {
            Codec::None => codecs::NONE,
            Codec::Zstd => codecs::ZSTD,
            Codec::Lz4 => codecs::LZ4,
            Codec::Deflate => codecs::DEFLATE,
            Codec::Unknown(value) => *value,
        }
    }

    /// 压缩数据
    /// 
    /// # 参数
    /// 
    /// * `data` - 要压缩的原始数据
    /// 
    /// # 返回
    /// 
    /// 压缩后的数据
    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            Codec::None => Ok(data.to_vec()),
            Codec::Zstd => {
                zstd::bulk::compress(data, 3)
                    .map_err(|e| UnivError::compression_error(format!("Zstd压缩失败: {}", e)))
            }
            Codec::Lz4 => {
                Ok(lz4_flex::compress_prepend_size(data))
            }
            Codec::Deflate => {
                use flate2::write::DeflateEncoder;
                use flate2::Compression;
                use std::io::Write;
                
                let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(data)
                    .map_err(|e| UnivError::compression_error(format!("Deflate压缩写入失败: {}", e)))?;
                encoder.finish()
                    .map_err(|e| UnivError::compression_error(format!("Deflate压缩完成失败: {}", e)))
            }
            Codec::Unknown(codec) => Err(UnivError::UnsupportedCodec { codec: *codec }),
        }
    }

    /// 解压缩数据
    /// 
    /// # 参数
    /// 
    /// * `data` - 压缩后的数据
    /// * `expected_size` - 预期的解压后大小（用于验证）
    /// 
    /// # 返回
    /// 
    /// 解压缩后的数据
    pub fn decompress(&self, data: &[u8], expected_size: Option<usize>) -> Result<Vec<u8>> {
        let result = match self {
            Codec::None => Ok(data.to_vec()),
            Codec::Zstd => {
                zstd::bulk::decompress(data, expected_size.unwrap_or(data.len() * 4))
                    .map_err(|e| UnivError::compression_error(format!("Zstd解压失败: {}", e)))
            }
            Codec::Lz4 => {
                lz4_flex::decompress_size_prepended(data)
                    .map_err(|e| UnivError::compression_error(format!("LZ4解压失败: {}", e)))
            }
            Codec::Deflate => {
                use flate2::read::DeflateDecoder;
                use std::io::Read;
                
                let mut decoder = DeflateDecoder::new(data);
                let mut result = Vec::new();
                decoder.read_to_end(&mut result)
                    .map_err(|e| UnivError::compression_error(format!("Deflate解压失败: {}", e)))?;
                Ok(result)
            }
            Codec::Unknown(codec) => Err(UnivError::UnsupportedCodec { codec: *codec }),
        }?;

        // 验证解压后的大小
        if let Some(expected) = expected_size {
            if result.len() != expected {
                return Err(UnivError::compression_error(format!(
                    "解压后大小不匹配: 期望 {}, 实际 {}",
                    expected,
                    result.len()
                )));
            }
        }

        Ok(result)
    }
}

/// UNIV 数据块
/// 
/// 包含类型、压缩信息、变换标志、原始数据和压缩数据。
#[derive(Debug, Clone)]
pub struct Chunk {
    /// 块类型
    pub kind: ChunkKind,
    /// 压缩算法
    pub codec: Codec,
    /// 变换标志
    pub transform_flags: u16,
    /// 原始数据大小
    pub raw_size: u32,
    /// 压缩后数据大小
    pub compressed_size: u32,
    /// 哈希算法
    pub hash_algorithm: u8,
    /// 内容哈希
    pub content_hash: Vec<u8>,
    /// 实际载荷数据（压缩后）
    pub payload: Bytes,
}

impl Chunk {
    /// 创建新的数据块
    /// 
    /// # 参数
    /// 
    /// * `kind` - 块类型
    /// * `raw_data` - 原始数据
    /// * `codec` - 压缩算法
    /// * `transform_flags` - 变换标志
    /// * `hash_algorithm` - 哈希算法
    /// 
    /// # 返回
    /// 
    /// 新创建的数据块
    pub fn new(
        kind: ChunkKind,
        raw_data: &[u8],
        codec: Codec,
        transform_flags: u16,
        hash_algorithm: u8,
    ) -> Result<Self> {
        // 压缩数据
        let compressed_data = codec.compress(raw_data)?;
        
        // 计算内容哈希（对原始数据）
        let content_hash = HashProvider::hash(hash_algorithm, raw_data)?;

        Ok(Self {
            kind,
            codec,
            transform_flags,
            raw_size: raw_data.len() as u32,
            compressed_size: compressed_data.len() as u32,
            hash_algorithm,
            content_hash,
            payload: Bytes::from(compressed_data),
        })
    }

    /// 从已有数据创建块（不进行压缩）
    /// 
    /// # 参数
    /// 
    /// * `kind` - 块类型
    /// * `codec` - 压缩算法
    /// * `transform_flags` - 变换标志
    /// * `raw_size` - 原始数据大小
    /// * `hash_algorithm` - 哈希算法
    /// * `content_hash` - 内容哈希
    /// * `payload` - 载荷数据（已压缩）
    /// 
    /// # 返回
    /// 
    /// 新创建的数据块
    pub fn from_parts(
        kind: ChunkKind,
        codec: Codec,
        transform_flags: u16,
        raw_size: u32,
        hash_algorithm: u8,
        content_hash: Vec<u8>,
        payload: Bytes,
    ) -> Self {
        Self {
            kind,
            codec,
            transform_flags,
            raw_size,
            compressed_size: payload.len() as u32,
            hash_algorithm,
            content_hash,
            payload,
        }
    }

    /// 获取原始数据
    /// 
    /// # 返回
    /// 
    /// 解压缩后的原始数据
    pub fn get_raw_data(&self) -> Result<Vec<u8>> {
        let decompressed = self.codec.decompress(&self.payload, Some(self.raw_size as usize))?;
        
        // 验证哈希
        let computed_hash = HashProvider::hash(self.hash_algorithm, &decompressed)?;
        if computed_hash != self.content_hash {
            return Err(UnivError::HashMismatch {
                expected: hex::encode(&self.content_hash),
                actual: hex::encode(&computed_hash),
            });
        }

        Ok(decompressed)
    }

    /// 序列化块到字节流（包含帧结构）
    /// 
    /// # 返回
    /// 
    /// 序列化后的字节数据
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = BytesMut::new();

        // 写入帧头 "CK01"
        buf.put_slice(b"CK01");

        // 写入块信息
        buf.put_u8(self.kind.to_u8());
        buf.put_u8(self.codec.to_u8());
        buf.put_u16_le(self.transform_flags);
        buf.put_u32_le(self.raw_size);
        buf.put_u32_le(self.compressed_size);
        buf.put_u8(self.hash_algorithm);

        // 写入内容哈希（可变长度）
        let hash_len = self.content_hash.len() as u8;
        buf.put_u8(hash_len);
        buf.put_slice(&self.content_hash);

        // 写入保留字段（2字节）
        buf.put_u16_le(0);

        // 写入载荷数据
        buf.put_slice(&self.payload);

        // 计算并写入CRC32C校验
        let frame_data = &buf[..];
        let crc = crc32c::crc32c(frame_data);
        buf.put_u32_le(crc);

        Ok(buf.to_vec())
    }

    /// 从字节流反序列化块
    /// 
    /// # 参数
    /// 
    /// * `data` - 要解析的字节数据
    /// 
    /// # 返回
    /// 
    /// 解析结果，包含数据块和消费的字节数
    pub fn deserialize(data: &[u8]) -> Result<(Self, usize)> {
        if data.len() < CHUNK_FRAME_HEADER_SIZE + CHUNK_FRAME_CRC_SIZE {
            return Err(UnivError::IncompleteData {
                expected: CHUNK_FRAME_HEADER_SIZE + CHUNK_FRAME_CRC_SIZE,
                actual: data.len(),
            });
        }

        let mut buf = &data[..];
        let original_len = buf.len();

        // 验证帧头
        let mut frame_magic = [0u8; 4];
        buf.copy_to_slice(&mut frame_magic);
        if &frame_magic != b"CK01" {
            return Err(UnivError::chunk_parse_error("无效的块帧头"));
        }

        // 读取块信息
        let kind = ChunkKind::from_u8(buf.get_u8());
        let codec = Codec::from_u8(buf.get_u8());
        let transform_flags = buf.get_u16_le();
        let raw_size = buf.get_u32_le();
        let compressed_size = buf.get_u32_le();
        let hash_algorithm = buf.get_u8();

        // 读取内容哈希
        let hash_len = buf.get_u8() as usize;
        if buf.remaining() < hash_len {
            return Err(UnivError::IncompleteData {
                expected: hash_len,
                actual: buf.remaining(),
            });
        }
        let content_hash = buf[..hash_len].to_vec();
        buf.advance(hash_len);

        // 跳过保留字段
        buf.advance(2);

        // 计算到此为止的头部大小
        let header_size = original_len - buf.len();
        let total_frame_size = header_size + compressed_size as usize + CHUNK_FRAME_CRC_SIZE;

        if data.len() < total_frame_size {
            return Err(UnivError::IncompleteData {
                expected: total_frame_size,
                actual: data.len(),
            });
        }

        // 读取载荷数据
        if buf.remaining() < compressed_size as usize + CHUNK_FRAME_CRC_SIZE {
            return Err(UnivError::IncompleteData {
                expected: compressed_size as usize + CHUNK_FRAME_CRC_SIZE,
                actual: buf.remaining(),
            });
        }

        let payload = Bytes::copy_from_slice(&buf[..compressed_size as usize]);
        buf.advance(compressed_size as usize);

        // 验证CRC32C
        let expected_crc = buf.get_u32_le();
        let frame_data = &data[..total_frame_size - CHUNK_FRAME_CRC_SIZE];
        let actual_crc = crc32c::crc32c(frame_data);
        
        if expected_crc != actual_crc {
            return Err(UnivError::CrcMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }

        let chunk = Self::from_parts(
            kind,
            codec,
            transform_flags,
            raw_size,
            hash_algorithm,
            content_hash,
            payload,
        );

        Ok((chunk, total_frame_size))
    }

    /// 验证块的完整性
    /// 
    /// # 返回
    /// 
    /// 如果验证成功返回Ok，否则返回错误
    pub fn verify(&self) -> Result<()> {
        // 解压缩并验证哈希
        self.get_raw_data()?;
        Ok(())
    }

    /// 获取压缩比
    /// 
    /// # 返回
    /// 
    /// 压缩比（原始大小/压缩大小）
    pub fn compression_ratio(&self) -> f32 {
        if self.compressed_size == 0 {
            return 1.0;
        }
        self.raw_size as f32 / self.compressed_size as f32
    }

    /// 检查是否应用了指定的变换
    /// 
    /// # 参数
    /// 
    /// * `flag` - 要检查的变换标志
    /// 
    /// # 返回
    /// 
    /// 如果应用了该变换返回true，否则返回false
    pub fn has_transform(&self, flag: u16) -> bool {
        self.transform_flags & flag != 0
    }

    /// 估算序列化后的大小
    /// 
    /// # 返回
    /// 
    /// 估算的字节数
    pub fn estimated_serialized_size(&self) -> usize {
        CHUNK_FRAME_HEADER_SIZE + 1 + self.content_hash.len() + 2 + 
        self.payload.len() + CHUNK_FRAME_CRC_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{hash_algorithms, transform_flags};

    #[test]
    fn test_chunk_kind_conversion() {
        assert_eq!(ChunkKind::from_u8(chunk_kinds::DATA_NODE), ChunkKind::DataNode);
        assert_eq!(ChunkKind::DataNode.to_u8(), chunk_kinds::DATA_NODE);
        
        let unknown = ChunkKind::from_u8(255);
        assert_eq!(unknown, ChunkKind::Unknown(255));
        assert_eq!(unknown.to_u8(), 255);
    }

    #[test]
    fn test_codec_compression() {
        let data = b"Hello, World! This is a test string for compression.";
        
        // 测试无压缩
        let none_codec = Codec::None;
        let compressed = none_codec.compress(data).unwrap();
        let decompressed = none_codec.decompress(&compressed, Some(data.len())).unwrap();
        assert_eq!(data, decompressed.as_slice());

        // 测试Zstd压缩
        let zstd_codec = Codec::Zstd;
        let compressed = zstd_codec.compress(data).unwrap();
        let decompressed = zstd_codec.decompress(&compressed, Some(data.len())).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_chunk_creation() {
        let data = b"Test chunk data";
        let chunk = Chunk::new(
            ChunkKind::DataNode,
            data,
            Codec::None,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();

        assert_eq!(chunk.kind, ChunkKind::DataNode);
        assert_eq!(chunk.codec, Codec::None);
        assert_eq!(chunk.raw_size, data.len() as u32);
        
        let recovered_data = chunk.get_raw_data().unwrap();
        assert_eq!(data, recovered_data.as_slice());
    }

    #[test]
    fn test_chunk_serialization() {
        let data = b"Test serialization data";
        let chunk = Chunk::new(
            ChunkKind::Schema,
            data,
            Codec::Zstd,
            transform_flags::DICT_STRING,
            hash_algorithms::BLAKE3,
        ).unwrap();

        let serialized = chunk.serialize().unwrap();
        let (deserialized, consumed) = Chunk::deserialize(&serialized).unwrap();

        assert_eq!(consumed, serialized.len());
        assert_eq!(deserialized.kind, chunk.kind);
        assert_eq!(deserialized.codec, chunk.codec);
        assert_eq!(deserialized.transform_flags, chunk.transform_flags);
        assert_eq!(deserialized.raw_size, chunk.raw_size);
        assert_eq!(deserialized.content_hash, chunk.content_hash);
        
        let recovered_data = deserialized.get_raw_data().unwrap();
        assert_eq!(data, recovered_data.as_slice());
    }

    #[test]
    fn test_chunk_verification() {
        let data = b"Data for verification test";
        let chunk = Chunk::new(
            ChunkKind::DataNode,
            data,
            Codec::None,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();

        // 正常验证应该成功
        assert!(chunk.verify().is_ok());

        // 修改内容哈希后验证应该失败
        let mut corrupted_chunk = chunk.clone();
        corrupted_chunk.content_hash[0] ^= 0xFF;
        assert!(corrupted_chunk.verify().is_err());
    }

    #[test]
    fn test_compression_ratio() {
        let data = vec![0u8; 1000]; // 大量重复数据，压缩率应该很高
        let chunk = Chunk::new(
            ChunkKind::DataNode,
            &data,
            Codec::Zstd,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();

        let ratio = chunk.compression_ratio();
        assert!(ratio > 1.0); // 压缩率应该大于1
    }

    #[test]
    fn test_transform_flags() {
        let data = b"Transform test data";
        let chunk = Chunk::new(
            ChunkKind::DataNode,
            data,
            Codec::None,
            transform_flags::DICT_STRING | transform_flags::INTEGER_VARINT,
            hash_algorithms::BLAKE3,
        ).unwrap();

        assert!(chunk.has_transform(transform_flags::DICT_STRING));
        assert!(chunk.has_transform(transform_flags::INTEGER_VARINT));
        assert!(!chunk.has_transform(transform_flags::COLUMNARIZE));
    }
}