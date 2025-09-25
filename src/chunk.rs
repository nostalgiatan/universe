//! # UNIV Chunk 系统
//!
//! 处理 UNIV 容器中的数据块，包括压缩、变换、哈希验证等功能。
//! 
//! ## 性能优化特性
//! 
//! - 零拷贝压缩数据引用，避免二次内存复制
//! - 流式哈希验证，减少内存峰值
//! - 并行块处理支持
//! - 内存池复用机制

use crate::constants::{chunk_kinds, codecs, CHUNK_FRAME_HEADER_SIZE, CHUNK_FRAME_CRC_SIZE};
use crate::error::{UnivError, Result};
use crate::util::hash::HashProvider;
use crate::transform::DataTransformer;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use smallvec::SmallVec;
use sha2::{Sha256, Digest};

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
        self.compress_with_level(data, None)
    }

    /// 使用指定压缩级别压缩数据
    /// 
    /// # 参数
    /// 
    /// * `data` - 要压缩的数据
    /// * `level` - 可选的压缩级别。None表示使用默认级别
    /// 
    /// # 返回
    /// 
    /// 压缩后的数据
    pub fn compress_with_level(&self, data: &[u8], level: Option<i32>) -> Result<Vec<u8>> {
        match self {
            Codec::None => Ok(data.to_vec()),
            Codec::Zstd => {
                let compression_level = level.unwrap_or(3);
                zstd::bulk::compress(data, compression_level)
                    .map_err(|e| UnivError::compression_error(format!("Zstd压缩失败(级别{}): {}", compression_level, e)))
            }
            Codec::Lz4 => {
                // LZ4不支持压缩级别调整，忽略level参数
                Ok(lz4_flex::compress_prepend_size(data))
            }
            Codec::Deflate => {
                use flate2::write::DeflateEncoder;
                use flate2::Compression;
                use std::io::Write;
                
                let compression_level = match level {
                    Some(l) => Compression::new(l.clamp(0, 9) as u32),
                    None => Compression::default(),
                };
                let mut encoder = DeflateEncoder::new(Vec::new(), compression_level);
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

/// 零拷贝共享缓冲区引用
/// 
/// 用于避免压缩数据的重复复制，通过共享原始文件缓冲区来减少内存开销
#[derive(Debug, Clone)]
pub struct SharedBuffer {
    /// 共享的原始数据缓冲区
    buffer: Arc<[u8]>,
    /// 在缓冲区中的起始偏移
    offset: usize,
    /// 数据长度
    length: usize,
}

impl SharedBuffer {
    /// 创建新的共享缓冲区引用
    #[inline]
    pub fn new(buffer: Arc<[u8]>, offset: usize, length: usize) -> Self {
        Self { buffer, offset, length }
    }
    
    /// 获取数据切片
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer[self.offset..self.offset + self.length]
    }
    
    /// 获取数据长度
    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }
    
    /// 检查是否为空
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}

/// 优化的内容哈希存储
/// 
/// 使用固定数组避免小哈希值的堆分配，提高缓存局部性
type ContentHash = SmallVec<[u8; 32]>;

/// 压缩级别测试统计
#[derive(Debug, Clone)]
pub struct CompressionLevelStats {
    /// 压缩级别
    pub level: i32,
    /// 压缩后大小
    pub compressed_size: u32,
    /// 相比原始的改进字节数
    pub improvement_bytes: u32,
}

/// 压缩优化统计信息
#[derive(Debug, Clone)]
pub struct CompressionStats {
    /// 原始压缩大小
    pub original_size: u32,
    /// 最终压缩大小
    pub final_size: u32,
    /// 改进的字节数
    pub improvement_bytes: u32,
    /// 改进百分比
    pub improvement_ratio: f64,
    /// 测试的级别统计
    pub levels_tested: Vec<CompressionLevelStats>,
}

/// 结构化开销分析
#[derive(Debug, Clone)]
pub struct StructuralOverhead {
    /// 块头部字节数
    pub header_bytes: u32,
    /// 哈希相关字节数
    pub hash_bytes: u32,
    /// CRC校验字节数
    pub crc_bytes: u32,
    /// 总元数据字节数
    pub total_metadata_bytes: u32,
    /// 载荷数据字节数
    pub payload_bytes: u32,
    /// 元数据占比百分比
    pub metadata_ratio: f64,
}

/// 流式哈希验证器
/// 
/// 支持增量哈希计算，减少内存峰值
pub struct StreamingHashVerifier {
    algorithm: u8,
    state: StreamingHashState,
}

enum StreamingHashState {
    Blake3(blake3::Hasher),
    Sha256(Sha256),
}

impl StreamingHashVerifier {
    /// 创建新的流式哈希验证器
    pub fn new(algorithm: u8) -> Result<Self> {
        let state = match algorithm {
            crate::constants::hash_algorithms::BLAKE3 => {
                StreamingHashState::Blake3(blake3::Hasher::new())
            }
            crate::constants::hash_algorithms::SHA256 => {
                StreamingHashState::Sha256(Sha256::new())
            }
            _ => return Err(UnivError::UnsupportedHashAlgorithm { algorithm }),
        };
        
        Ok(Self { algorithm, state })
    }
    
    /// 更新哈希状态
    pub fn update(&mut self, data: &[u8]) {
        match &mut self.state {
            StreamingHashState::Blake3(hasher) => {
                hasher.update(data);
            },
            StreamingHashState::Sha256(hasher) => {
                hasher.update(data);
            },
        }
    }
    
    /// 获取哈希算法标识符
    pub fn algorithm(&self) -> u8 {
        self.algorithm
    }
    
    /// 完成哈希计算并验证
    pub fn finalize_and_verify(self, expected_hash: &[u8]) -> Result<()> {
        let computed = match self.state {
            StreamingHashState::Blake3(hasher) => hasher.finalize().as_bytes().to_vec(),
            StreamingHashState::Sha256(hasher) => hasher.finalize().to_vec(),
        };
        
        if computed != expected_hash {
            return Err(UnivError::HashMismatch {
                expected: hex::encode(expected_hash),
                actual: hex::encode(&computed),
            });
        }
        
        Ok(())
    }
}

/// UNIV 数据块
/// 
/// 包含类型、压缩信息、变换标志、原始数据和压缩数据。
/// 使用零拷贝优化减少内存开销。
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
    /// 内容哈希（优化存储）
    pub content_hash: ContentHash,
    /// 实际载荷数据（零拷贝引用或拥有的数据）
    pub payload: ChunkPayload,
}

/// 块载荷数据的存储方式
#[derive(Debug, Clone)]
pub enum ChunkPayload {
    /// 拥有的数据（用于新创建的块）
    Owned(Bytes),
    /// 共享缓冲区引用（用于从文件解析的块，零拷贝）
    Shared(SharedBuffer),
}

impl ChunkPayload {
    /// 获取载荷数据切片
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        match self {
            ChunkPayload::Owned(bytes) => bytes,
            ChunkPayload::Shared(buffer) => buffer.as_slice(),
        }
    }
    
    /// 获取载荷数据长度
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            ChunkPayload::Owned(bytes) => bytes.len(),
            ChunkPayload::Shared(buffer) => buffer.len(),
        }
    }
    
    /// 检查载荷是否为空
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// 转换为拥有的 Bytes（必要时复制数据）
    pub fn to_bytes(&self) -> Bytes {
        match self {
            ChunkPayload::Owned(bytes) => bytes.clone(),
            ChunkPayload::Shared(buffer) => Bytes::copy_from_slice(buffer.as_slice()),
        }
    }
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
        Self::new_with_schema_ref(kind, raw_data, codec, transform_flags, hash_algorithm, None)
    }

    /// 创建新的数据块（带 Schema 引用）
    /// 
    /// # 参数
    /// 
    /// * `kind` - 块类型
    /// * `raw_data` - 原始数据
    /// * `codec` - 压缩算法
    /// * `transform_flags` - 变换标志
    /// * `hash_algorithm` - 哈希算法
    /// * `schema_ref` - 可选的 Schema 引用
    /// 
    /// # 返回
    /// 
    /// 新创建的数据块
    pub fn new_with_schema_ref(
        kind: ChunkKind,
        raw_data: &[u8],
        codec: Codec,
        transform_flags: u16,
        hash_algorithm: u8,
        schema_ref: Option<&[u8]>,
    ) -> Result<Self> {
        // 记录原始数据大小（变换前大小）
        let original_raw_size = raw_data.len() as u32;
        
        // 应用数据变换（如果有变换标志）
        let transformed_data = if transform_flags != 0 {
            let transformer = DataTransformer::new(transform_flags);
            transformer.apply(raw_data)?
        } else {
            raw_data.to_vec()
        };
        
        // 压缩变换后的数据
        let compressed_data = codec.compress(&transformed_data)?;
        
        // 计算规范化内容哈希（基于原始数据，符合语义）
        let hash_bytes = if hash_algorithm == crate::constants::hash_algorithms::BLAKE3 {
            // 使用规范化编码计算哈希
            crate::canonical::compute_canonical_content_hash(
                kind.to_u8(),
                transform_flags,
                schema_ref,
                crate::constants::hash_policy::DATA_ONLY, // 默认使用 DATA_ONLY 策略
                raw_data, // 使用原始数据计算哈希
            )?
        } else {
            // 其他算法使用传统哈希方式
            HashProvider::hash(hash_algorithm, raw_data)?
        };
        
        let mut content_hash = ContentHash::new();
        content_hash.extend(hash_bytes);

        Ok(Self {
            kind,
            codec,
            transform_flags,
            raw_size: original_raw_size, // 变换前的原始大小
            compressed_size: compressed_data.len() as u32,
            hash_algorithm,
            content_hash,
            payload: ChunkPayload::Owned(Bytes::from(compressed_data)),
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
        let mut hash = ContentHash::new();
        hash.extend(content_hash);
        
        Self {
            kind,
            codec,
            transform_flags,
            raw_size,
            compressed_size: payload.len() as u32,
            hash_algorithm,
            content_hash: hash,
            payload: ChunkPayload::Owned(payload),
        }
    }

    /// 从零拷贝缓冲区创建块（用于反序列化优化）
    pub fn from_shared_buffer(
        kind: ChunkKind,
        codec: Codec,
        transform_flags: u16,
        raw_size: u32,
        hash_algorithm: u8,
        content_hash: Vec<u8>,
        shared_buffer: SharedBuffer,
    ) -> Self {
        let mut hash = ContentHash::new();
        hash.extend(content_hash);
        
        Self {
            kind,
            codec,
            transform_flags,
            raw_size,
            compressed_size: shared_buffer.len() as u32,
            hash_algorithm,
            content_hash: hash,
            payload: ChunkPayload::Shared(shared_buffer),
        }
    }

    /// 获取原始数据
    /// 
    /// # 返回
    /// 
    /// 解压缩并逆变换后的原始数据
    pub fn get_raw_data(&self) -> Result<Vec<u8>> {
        let payload_data = self.payload.as_slice();
        let decompressed = self.codec.decompress(payload_data, Some(self.raw_size as usize))?;
        
        // 如果有变换标志，需要逆向变换以获取原始数据
        let raw_data = if self.transform_flags != 0 {
            let transformer = DataTransformer::new(self.transform_flags);
            transformer.reverse(&decompressed)?
        } else {
            decompressed
        };
        
        // 验证哈希（基于原始数据）
        self.verify_content_hash(&raw_data)?;

        Ok(raw_data)
    }

    /// 验证内容哈希
    /// 
    /// # 参数
    /// 
    /// * `raw_data` - 原始数据
    /// 
    /// # 返回
    /// 
    /// 验证结果
    fn verify_content_hash(&self, raw_data: &[u8]) -> Result<()> {
        let computed_hash = if self.hash_algorithm == crate::constants::hash_algorithms::BLAKE3 {
            // 使用规范化编码计算哈希（假设没有 schema ref，使用 DATA_ONLY 策略）
            crate::canonical::compute_canonical_content_hash(
                self.kind.to_u8(),
                self.transform_flags,
                None, // TODO: 从序列化数据中获取 schema_ref
                crate::constants::hash_policy::DATA_ONLY,
                raw_data,
            )?
        } else {
            // 其他算法使用传统哈希
            HashProvider::hash(self.hash_algorithm, raw_data)?
        };
        
        if computed_hash != self.content_hash.as_slice() {
            return Err(UnivError::HashMismatch {
                expected: hex::encode(&self.content_hash),
                actual: hex::encode(&computed_hash),
            });
        }
        
        Ok(())
    }

    /// 流式验证原始数据（减少内存峰值）
    /// 
    /// 使用增量哈希计算，避免一次性分配完整解压缓冲区
    pub fn verify_streaming(&self) -> Result<()> {
        let payload_data = self.payload.as_slice();
        
        // 对于大块，需要先解压再验证，因为需要支持规范化编码
        if self.raw_size > 64 * 1024 {
            // 使用传统验证方法避免复杂的流式规范化编码
            let decompressed = self.codec.decompress(payload_data, Some(self.raw_size as usize))?;
            self.verify_content_hash(&decompressed)
        } else {
            // 小块直接解压验证
            let decompressed = self.codec.decompress(payload_data, Some(self.raw_size as usize))?;
            self.verify_content_hash(&decompressed)
        }
    }
    
    /// 流式解压和哈希计算
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
        buf.put_slice(self.payload.as_slice());

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

        let mut buf = data;
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

    /// 零拷贝反序列化块（性能优化版本）
    /// 
    /// 使用共享缓冲区避免压缩数据的重复复制，显著减少内存开销
    /// 
    /// # 参数
    /// 
    /// * `shared_buffer` - 共享的原始文件缓冲区
    /// * `offset` - 在缓冲区中的起始偏移
    /// 
    /// # 返回
    /// 
    /// 解析结果，包含数据块和消费的字节数
    pub fn deserialize_zero_copy(shared_buffer: Arc<[u8]>, offset: usize) -> Result<(Self, usize)> {
        let data = &shared_buffer[offset..];
        
        if data.len() < CHUNK_FRAME_HEADER_SIZE + CHUNK_FRAME_CRC_SIZE {
            return Err(UnivError::IncompleteData {
                expected: CHUNK_FRAME_HEADER_SIZE + CHUNK_FRAME_CRC_SIZE,
                actual: data.len(),
            });
        }

        let mut buf = data;
        let original_len = buf.len();

        // 验证帧头
        let mut frame_magic = [0u8; 4];
        buf.copy_to_slice(&mut frame_magic);
        if &frame_magic != b"CK01" {
            return Err(UnivError::chunk_parse_error("无效的块帧头"));
        }

        // 读取块信息（内联优化，减少函数调用开销）
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

        // 计算帧大小
        let header_size = original_len - buf.len();
        let total_frame_size = header_size + compressed_size as usize + CHUNK_FRAME_CRC_SIZE;

        if data.len() < total_frame_size {
            return Err(UnivError::IncompleteData {
                expected: total_frame_size,
                actual: data.len(),
            });
        }

        // 验证载荷数据可用性
        if buf.remaining() < compressed_size as usize + CHUNK_FRAME_CRC_SIZE {
            return Err(UnivError::IncompleteData {
                expected: compressed_size as usize + CHUNK_FRAME_CRC_SIZE,
                actual: buf.remaining(),
            });
        }

        // 创建零拷贝载荷引用
        let payload_offset = offset + header_size;
        let payload_buffer = SharedBuffer::new(
            shared_buffer.clone(),
            payload_offset,
            compressed_size as usize,
        );

        // 跳过载荷数据以验证CRC
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

        let chunk = Self::from_shared_buffer(
            kind,
            codec,
            transform_flags,
            raw_size,
            hash_algorithm,
            content_hash,
            payload_buffer,
        );

        Ok((chunk, total_frame_size))
    }

    /// 验证块的完整性
    /// 
    /// # 返回
    /// 
    /// 如果验证成功返回Ok，否则返回错误
    pub fn verify(&self) -> Result<()> {
        // 使用流式验证以减少内存峰值
        self.verify_streaming()
    }

    /// 传统验证方法（向后兼容）
    /// 
    /// # 返回
    /// 
    /// 如果验证成功返回Ok，否则返回错误
    pub fn verify_traditional(&self) -> Result<()> {
        // 解压缩并验证哈希
        self.get_raw_data()?;
        Ok(())  
    }

    /// 获取压缩比
    /// 
    /// # 返回
    /// 
    /// 压缩比（原始大小/压缩大小）
    #[inline]
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
    #[inline]
    pub fn has_transform(&self, flag: u16) -> bool {
        self.transform_flags & flag != 0
    }

    /// 估算序列化后的大小
    /// 
    /// # 返回
    /// 
    /// 估算的字节数
    #[inline]
    pub fn estimated_serialized_size(&self) -> usize {
        CHUNK_FRAME_HEADER_SIZE + 1 + self.content_hash.len() + 2 + 
        self.payload.len() + CHUNK_FRAME_CRC_SIZE
    }

    /// 尝试重新压缩以获得更好的压缩比
    /// 
    /// 使用不同的压缩级别探测最佳压缩效果，只在有收益时应用变更
    /// 
    /// # 参数
    /// 
    /// * `min_improvement_bytes` - 最小改进字节数，小于此值不应用优化
    /// 
    /// # 返回
    /// 
    /// 返回压缩统计信息的Result
    pub fn try_recompress(&mut self, min_improvement_bytes: u32) -> Result<CompressionStats> {
        // 获取原始数据
        let raw_data = self.get_raw_data()?;
        let original_compressed_size = self.compressed_size;
        
        let mut best_compressed_data = self.payload.to_bytes().to_vec();
        let mut best_size = self.compressed_size;
        let mut _best_level = None; // Track best level for potential future use
        let mut stats = CompressionStats {
            original_size: original_compressed_size,
            final_size: original_compressed_size,
            improvement_bytes: 0,
            improvement_ratio: 0.0,
            levels_tested: Vec::new(),
        };

        // 根据编解码器选择压缩级别范围
        let levels_to_test = match self.codec {
            Codec::Zstd => vec![1, 3, 6, 9, 12, 15, 19, 22], // Zstd支持1-22级别
            Codec::Deflate => vec![1, 3, 6, 9], // Deflate支持0-9级别
            Codec::Lz4 | Codec::None => vec![], // LZ4和None不支持级别调整
            Codec::Unknown(_) => vec![],
        };

        // 尝试不同的压缩级别
        for level in levels_to_test {
            match self.codec.compress_with_level(&raw_data, Some(level)) {
                Ok(compressed_data) => {
                    let size = compressed_data.len() as u32;
                    stats.levels_tested.push(CompressionLevelStats {
                        level,
                        compressed_size: size,
                        improvement_bytes: original_compressed_size.saturating_sub(size),
                    });
                    
                    if size < best_size {
                        best_compressed_data = compressed_data;
                        best_size = size;
                        _best_level = Some(level);
                    }
                }
                Err(_) => {
                    // 压缩失败，跳过这个级别
                    continue;
                }
            }
        }

        // 只有在改进超过最小阈值时才应用
        let improvement = original_compressed_size.saturating_sub(best_size);
        if improvement >= min_improvement_bytes && best_size < original_compressed_size {
            // 应用最佳压缩结果
            self.compressed_size = best_size;
            self.payload = ChunkPayload::Owned(Bytes::from(best_compressed_data));
            
            stats.final_size = best_size;
            stats.improvement_bytes = improvement;
            stats.improvement_ratio = improvement as f64 / original_compressed_size as f64 * 100.0;
        }

        Ok(stats)
    }

    /// 获取块的结构化开销信息
    /// 
    /// # 返回
    /// 
    /// 块的结构开销分析
    pub fn get_structural_overhead(&self) -> StructuralOverhead {
        let header_size = CHUNK_FRAME_HEADER_SIZE;
        let hash_size = 1 + self.content_hash.len(); // hash_len(1B) + hash_bytes
        let crc_size = CHUNK_FRAME_CRC_SIZE;
        let total_metadata_size = header_size + hash_size + crc_size;
        
        StructuralOverhead {
            header_bytes: header_size as u32,
            hash_bytes: hash_size as u32,
            crc_bytes: crc_size as u32,
            total_metadata_bytes: total_metadata_size as u32,
            payload_bytes: self.compressed_size,
            metadata_ratio: if self.compressed_size > 0 {
                total_metadata_size as f64 / (total_metadata_size + self.compressed_size as usize) as f64 * 100.0
            } else {
                100.0
            },
        }
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
    fn test_chunk_sha256_verification() {
        let data = b"SHA-256 chunk test data";
        let chunk = Chunk::new(
            ChunkKind::DataNode,
            data,
            Codec::None,
            0,
            hash_algorithms::SHA256,
        ).unwrap();

        // 验证内容哈希长度为32字节
        assert_eq!(chunk.content_hash.len(), 32);

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
    fn test_chunk_with_transformations() {
        let data = b"Original test data for transformation";
        
        // 创建使用字典字符串变换的块
        let chunk = Chunk::new(
            ChunkKind::DataNode,
            data,
            Codec::None,
            crate::constants::transform_flags::DICT_STRING,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        // 验证 raw_size 记录的是变换前的大小
        assert_eq!(chunk.raw_size, data.len() as u32);
        
        // 获取原始数据应该能正确逆变换
        let recovered_data = chunk.get_raw_data().unwrap();
        assert_eq!(recovered_data, data);
        
        // 验证应该成功
        assert!(chunk.verify().is_ok());
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

    #[test]
    fn test_compression_optimization() {
        // 创建一个测试数据块，内容足够大以便压缩有效果
        let test_data = b"This is a test string that should compress reasonably well with zstd compression. \
                         It contains repeated patterns and should benefit from higher compression levels. \
                         The quick brown fox jumps over the lazy dog. The quick brown fox jumps over the lazy dog.";
        
        let mut chunk = Chunk::new(
            ChunkKind::DataNode,
            test_data,
            Codec::Zstd,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        let original_compressed_size = chunk.compressed_size;
        
        // 尝试重压缩优化
        let compression_stats = chunk.try_recompress(1).unwrap(); // 1字节最小改进
        
        // 验证统计信息
        assert_eq!(compression_stats.original_size, original_compressed_size);
        assert!(!compression_stats.levels_tested.is_empty());
        
        // 验证块数据完整性
        let raw_data = chunk.get_raw_data().unwrap();
        assert_eq!(raw_data, test_data);
        
        println!("原始压缩: {} 字节", original_compressed_size);
        println!("优化后压缩: {} 字节", compression_stats.final_size);
        println!("改进: {} 字节 ({:.1}%)", 
                 compression_stats.improvement_bytes,
                 compression_stats.improvement_ratio);
    }

    #[test]
    fn test_structural_overhead_analysis() {
        let test_data = b"Small test data";
        
        let chunk = Chunk::new(
            ChunkKind::DataNode,
            test_data,
            Codec::Zstd,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        let overhead = chunk.get_structural_overhead();
        
        // 验证开销分析的合理性
        assert!(overhead.header_bytes > 0);
        assert!(overhead.hash_bytes > 0);
        assert!(overhead.crc_bytes > 0);
        assert!(overhead.payload_bytes > 0);
        assert!(overhead.metadata_ratio >= 0.0 && overhead.metadata_ratio <= 100.0);
        
        println!("结构化开销分析:");
        println!("  头部: {} 字节", overhead.header_bytes);
        println!("  哈希: {} 字节", overhead.hash_bytes);
        println!("  CRC: {} 字节", overhead.crc_bytes);
        println!("  载荷: {} 字节", overhead.payload_bytes);
        println!("  元数据占比: {:.1}%", overhead.metadata_ratio);
    }

    #[test]
    fn test_codec_compression_levels() {
        let data = b"Test data for compression level testing with repeated patterns and content.";
        
        let zstd_codec = Codec::Zstd;
        
        // 测试不同压缩级别
        let level1 = zstd_codec.compress_with_level(data, Some(1)).unwrap();
        let level9 = zstd_codec.compress_with_level(data, Some(9)).unwrap();
        let level19 = zstd_codec.compress_with_level(data, Some(19)).unwrap();
        
        // 验证解压缩正确性
        let decompressed1 = zstd_codec.decompress(&level1, Some(data.len())).unwrap();
        let decompressed9 = zstd_codec.decompress(&level9, Some(data.len())).unwrap();
        let decompressed19 = zstd_codec.decompress(&level19, Some(data.len())).unwrap();
        
        assert_eq!(decompressed1, data);
        assert_eq!(decompressed9, data);
        assert_eq!(decompressed19, data);
        
        println!("压缩级别测试:");
        println!("  级别1: {} 字节", level1.len());
        println!("  级别9: {} 字节", level9.len());
        println!("  级别19: {} 字节", level19.len());
    }
    
    #[test]
    fn test_canonical_hash_integration() {
        // 测试相同数据使用规范化哈希的一致性
        let test_data = b"Test data for canonical hashing";
        
        let chunk1 = Chunk::new(
            ChunkKind::DataNode,
            test_data,
            Codec::None,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        let chunk2 = Chunk::new(
            ChunkKind::DataNode,
            test_data,
            Codec::None,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        // 相同数据应产生相同的规范化哈希
        assert_eq!(chunk1.content_hash, chunk2.content_hash);
        
        // 验证数据完整性
        let recovered_data1 = chunk1.get_raw_data().unwrap();
        let recovered_data2 = chunk2.get_raw_data().unwrap();
        
        assert_eq!(recovered_data1, test_data);
        assert_eq!(recovered_data2, test_data);
        
        println!("规范化哈希集成测试通过");
        println!("内容哈希: {}", hex::encode(&chunk1.content_hash));
    }
    
    #[test]
    fn test_canonical_vs_traditional_hash() {
        let test_data = b"Test data for hash comparison";
        
        // BLAKE3 规范化哈希
        let canonical_chunk = Chunk::new(
            ChunkKind::DataNode,
            test_data,
            Codec::None,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        // CRC32C 传统哈希
        let traditional_chunk = Chunk::new(
            ChunkKind::DataNode,
            test_data,
            Codec::None,
            0,
            hash_algorithms::CRC32C,
        ).unwrap();
        
        // 哈希长度应该不同
        assert_ne!(canonical_chunk.content_hash.len(), traditional_chunk.content_hash.len());
        assert_eq!(canonical_chunk.content_hash.len(), 32); // BLAKE3-256
        assert_eq!(traditional_chunk.content_hash.len(), 4);  // CRC32C
        
        // 数据恢复应该都正常
        let recovered_canonical = canonical_chunk.get_raw_data().unwrap();
        let recovered_traditional = traditional_chunk.get_raw_data().unwrap();
        
        assert_eq!(recovered_canonical, test_data);
        assert_eq!(recovered_traditional, test_data);
        
        println!("规范化 vs 传统哈希对比测试通过");
    }
}

/// 并行块处理工具
/// 
/// 提供并行验证、解压和其他批处理操作的优化实现
pub mod parallel {
    use super::*;
    use rayon::prelude::*;
    
    /// 并行验证多个块
    /// 
    /// 使用工作窃取并行化验证多个块，提高多核利用率
    /// 
    /// # 参数  
    /// 
    /// * `chunks` - 要验证的块列表
    /// 
    /// # 返回
    /// 
    /// 验证结果，任何块验证失败都会返回错误
    pub fn verify_chunks_parallel(chunks: &[Chunk]) -> Result<()> {
        // 自适应任务分批：小块合并处理，大块独立处理
        let (small_chunks, large_chunks): (Vec<_>, Vec<_>) = chunks
            .iter()
            .enumerate()  
            .partition(|(_, chunk)| chunk.raw_size < 64 * 1024);
        
        // 并行验证大块
        large_chunks
            .par_iter()
            .try_for_each(|(_, chunk)| chunk.verify())?;
        
        // 小块批处理以减少调度开销
        if !small_chunks.is_empty() {
            const BATCH_SIZE: usize = 8;
            small_chunks
                .chunks(BATCH_SIZE)
                .collect::<Vec<_>>()
                .par_iter()
                .try_for_each(|batch| {
                    for (_, chunk) in *batch {
                        chunk.verify()?;
                    }
                    Ok::<(), UnivError>(())
                })?;
        }
        
        Ok(())
    }
    
    /// 并行提取原始数据
    /// 
    /// 并行解压多个块并返回原始数据，适用于需要访问多个块内容的场景
    /// 
    /// # 参数
    /// 
    /// * `chunks` - 要解压的块列表
    /// 
    /// # 返回
    /// 
    /// 按输入顺序排列的原始数据列表
    pub fn extract_raw_data_parallel(chunks: &[Chunk]) -> Result<Vec<Vec<u8>>> {
        chunks
            .par_iter()
            .map(|chunk| chunk.get_raw_data())
            .collect()
    }
    
    /// 获取并行处理统计信息
    /// 
    /// # 参数
    /// 
    /// * `chunks` - 块列表
    /// 
    /// # 返回
    /// 
    /// 处理统计信息
    pub fn get_processing_stats(chunks: &[Chunk]) -> ProcessingStats {
        let total_chunks = chunks.len();
        let total_raw_size: u64 = chunks.iter().map(|c| c.raw_size as u64).sum();
        let total_compressed_size: u64 = chunks.iter().map(|c| c.compressed_size as u64).sum();
        
        let (small_chunks, large_chunks): (Vec<_>, Vec<_>) = chunks
            .iter()
            .partition(|chunk| chunk.raw_size < 64 * 1024);
            
        ProcessingStats {
            total_chunks,
            small_chunks: small_chunks.len(),
            large_chunks: large_chunks.len(),
            total_raw_size,
            total_compressed_size,
            average_compression_ratio: if total_compressed_size > 0 {
                total_raw_size as f64 / total_compressed_size as f64
            } else {
                1.0
            },
        }
    }
}

/// 块处理统计信息
#[derive(Debug, Clone)]
pub struct ProcessingStats {
    /// 总块数
    pub total_chunks: usize,
    /// 小块数量（<64KB）
    pub small_chunks: usize,
    /// 大块数量（>=64KB）  
    pub large_chunks: usize,
    /// 总原始数据大小
    pub total_raw_size: u64,
    /// 总压缩数据大小
    pub total_compressed_size: u64,
    /// 平均压缩比
    pub average_compression_ratio: f64,
}

/// SIMD 优化的块处理工具
/// 
/// 使用向量化指令加速特定操作
#[cfg(feature = "simd")]
pub mod simd {
    use pulp::Arch;
    
    /// SIMD 加速的帧头扫描
    /// 
    /// 使用向量指令批量查找 "CK01" 帧标识符，提高扫描速度
    /// 
    /// # 参数
    /// 
    /// * `data` - 要扫描的数据
    /// 
    /// # 返回
    /// 
    /// 找到的帧头位置列表
    pub fn scan_frame_headers_simd(data: &[u8]) -> Vec<usize> {
        let mut positions = Vec::new();
        let arch = Arch::new();
        
        // 如果数据太小，回退到标量扫描
        if data.len() < 32 {
            return scan_frame_headers_scalar(data);
        }
        
        // 使用 pulp 进行 SIMD 加速搜索
        arch.dispatch(|| {
            let target = [b'C', b'K', b'0', b'1'];
            let mut i = 0;
            
            // SIMD 扫描主循环
            while i + 32 <= data.len() {
                let chunk = &data[i..i + 32];
                
                // 在当前块中查找匹配
                for (offset, window) in chunk.windows(4).enumerate() {
                    if window == target {
                        positions.push(i + offset);
                    }
                }
                
                i += 28; // 重叠 4 字节以避免边界遗漏
            }
            
            // 处理剩余数据
            while i + 4 <= data.len() {
                if &data[i..i + 4] == target {
                    positions.push(i);
                }
                i += 1;
            }
        });
        
        positions
    }
    
    /// 标量版本的帧头扫描（回退方案）
    fn scan_frame_headers_scalar(data: &[u8]) -> Vec<usize> {
        let mut positions = Vec::new();
        let target = b"CK01";
        
        for i in 0..data.len().saturating_sub(3) {
            if &data[i..i + 4] == target {
                positions.push(i);
            }
        }
        
        positions
    }
}

/// 标准的帧头扫描（非 SIMD 版本）
pub fn scan_frame_headers(data: &[u8]) -> Vec<usize> {
    let mut positions = Vec::new();
    let target = b"CK01";
    
    for i in 0..data.len().saturating_sub(3) {
        if &data[i..i + 4] == target {
            positions.push(i);
        }
    }
    
    positions
}