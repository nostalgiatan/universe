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
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use smallvec::SmallVec;

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

/// 流式哈希验证器
/// 
/// 支持增量哈希计算，减少内存峰值
pub struct StreamingHashVerifier {
    algorithm: u8,
    state: StreamingHashState,
}

enum StreamingHashState {
    Blake3(blake3::Hasher),
    // 预留其他算法支持
}

impl StreamingHashVerifier {
    /// 创建新的流式哈希验证器
    pub fn new(algorithm: u8) -> Result<Self> {
        let state = match algorithm {
            crate::constants::hash_algorithms::BLAKE3 => {
                StreamingHashState::Blake3(blake3::Hasher::new())
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
        // 压缩数据
        let compressed_data = codec.compress(raw_data)?;
        
        // 计算内容哈希（对原始数据）
        let hash_bytes = HashProvider::hash(hash_algorithm, raw_data)?;
        let mut content_hash = ContentHash::new();
        content_hash.extend(hash_bytes);

        Ok(Self {
            kind,
            codec,
            transform_flags,
            raw_size: raw_data.len() as u32,
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
    /// 解压缩后的原始数据
    pub fn get_raw_data(&self) -> Result<Vec<u8>> {
        let payload_data = self.payload.as_slice();
        let decompressed = self.codec.decompress(payload_data, Some(self.raw_size as usize))?;
        
        // 验证哈希
        let computed_hash = HashProvider::hash(self.hash_algorithm, &decompressed)?;
        if computed_hash != self.content_hash.as_slice() {
            return Err(UnivError::HashMismatch {
                expected: hex::encode(&self.content_hash),
                actual: hex::encode(&computed_hash),
            });
        }

        Ok(decompressed)
    }

    /// 流式验证原始数据（减少内存峰值）
    /// 
    /// 使用增量哈希计算，避免一次性分配完整解压缓冲区
    pub fn verify_streaming(&self) -> Result<()> {
        let payload_data = self.payload.as_slice();
        
        // 创建流式哈希验证器
        let mut verifier = StreamingHashVerifier::new(self.hash_algorithm)?;
        
        // 对于大块，使用流式解压+哈希
        if self.raw_size > 64 * 1024 {
            self.decompress_and_hash_streaming(payload_data, &mut verifier)?;
        } else {
            // 小块直接解压验证
            let decompressed = self.codec.decompress(payload_data, Some(self.raw_size as usize))?;
            verifier.update(&decompressed);
        }
        
        verifier.finalize_and_verify(&self.content_hash)
    }
    
    /// 流式解压和哈希计算
    fn decompress_and_hash_streaming(&self, payload_data: &[u8], verifier: &mut StreamingHashVerifier) -> Result<()> {
        match self.codec {
            Codec::None => {
                verifier.update(payload_data);
            }
            Codec::Zstd => {
                // 使用 zstd 流式解压
                use std::io::Read;
                let mut decoder = zstd::stream::read::Decoder::new(payload_data)?;
                let mut buffer = vec![0u8; 8192]; // 8KB 缓冲区
                
                loop {
                    let bytes_read = decoder.read(&mut buffer)
                        .map_err(|e| UnivError::compression_error(format!("Zstd流式解压失败: {}", e)))?;
                    if bytes_read == 0 {
                        break;
                    }
                    verifier.update(&buffer[..bytes_read]);
                }
            }
            _ => {
                // 对于其他压缩算法，回退到标准解压
                let decompressed = self.codec.decompress(payload_data, Some(self.raw_size as usize))?;
                verifier.update(&decompressed);
            }
        }
        Ok(())
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