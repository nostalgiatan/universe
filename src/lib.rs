//! # UNIV (Universe) 容器格式库
//!
//! UNIV 是一个统一的二进制容器格式，支持多种数据模式和优化策略。
//! 
//! ## 特性
//! 
//! - 支持分型 Profile（BLOB、RECD、TABL、TSDB、GRPH、TYPE 等）
//! - 块级压缩和变换流水线
//! - 内容寻址和引用系统
//! - 随机访问和 Schema 引用
//! - 可演进的版本兼容策略
//! - 安全限制和验证机制
//!
//! ## 基本用法
//!
//! ```rust
//! use universe::{Container, Profile};
//! 
//! // 创建一个新的 RECD 类型容器
//! let mut container = Container::new(Profile::Recd);
//! 
//! // 添加数据到容器
//! container.add_data(
//!     universe::ChunkKind::DataNode,
//!     b"test data",
//!     universe::chunk::Codec::None,
//!     0, // transform flags
//!     universe::constants::hash_algorithms::BLAKE3
//! ).unwrap();
//! 
//! // 序列化到字节流
//! let bytes = container.serialize().unwrap();
//! 
//! // 从字节流反序列化
//! let deserialized_container = Container::deserialize(&bytes).unwrap();
//! assert_eq!(deserialized_container.chunk_count(), 1);
//! ```

// 核心模块
pub mod constants;
pub mod error;
pub mod header;
pub mod chunk;
pub mod toc;
pub mod transform;
pub mod profile;
pub mod reference;
pub mod security;
pub mod util;

// 重新导出主要类型
pub use error::{UnivError, Result};
pub use header::Header;
pub use chunk::{Chunk, ChunkKind};
pub use profile::Profile;

/// UNIV 容器的主要入口点
/// 
/// 代表一个完整的 UNIV 容器，包含头部信息、数据块和索引。
#[derive(Debug, Clone)]
pub struct Container {
    /// 容器头部信息
    pub header: Header,
    /// 数据块列表
    pub chunks: Vec<Chunk>,
    /// 目录索引
    pub toc: Option<toc::Toc>,
}

impl Container {
    /// 创建一个新的容器
    /// 
    /// # 参数
    /// 
    /// * `profile` - 容器的数据模式
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use universe::{Container, Profile};
    /// 
    /// let container = Container::new(Profile::Recd);
    /// ```
    pub fn new(profile: Profile) -> Self {
        Self {
            header: Header::new(profile),
            chunks: Vec::new(),
            toc: None,
        }
    }

    /// 从字节流反序列化容器
    /// 
    /// # 参数
    /// 
    /// * `data` - 要解析的字节数据
    /// 
    /// # 返回
    /// 
    /// 成功时返回解析后的容器，失败时返回错误
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.is_empty() {
            return Err(UnivError::IncompleteData { 
                expected: 1, 
                actual: 0 
            });
        }
        
        let mut offset = 0;
        
        // 1. 解析头部
        let (header, header_size) = Header::deserialize(&data[offset..])?;
        offset += header_size;
        
        // 2. 解析数据块
        let mut chunks = Vec::new();
        
        // 解析chunks直到遇到文件结束
        while offset < data.len() {
            let remaining = &data[offset..];
            
            // 检查是否是chunk标识符
            if remaining.len() < 4 || &remaining[..4] != b"CK01" {
                // 如果没有更多chunk，剩余数据可能是TOC
                break;
            }
            
            let (chunk, chunk_size) = Chunk::deserialize(remaining)?;
            chunks.push(chunk);
            offset += chunk_size;
        }
        
        // 3. 解析TOC（如果存在）
        let toc = if offset < data.len() {
            let remaining = &data[offset..];
            // TOC 现在是直接的 CBOR 数据，不是以 "TOC1" 开头
            // 尝试解析剩余数据为 TOC
            if !remaining.is_empty() {
                match toc::Toc::deserialize(remaining) {
                    Ok(toc) => Some(toc),
                    Err(_) => None, // 如果解析失败，表示没有TOC
                }
            } else {
                None
            }
        } else {
            None
        };
        
        Ok(Self {
            header,
            chunks,
            toc,
        })
    }

    /// 将容器序列化为字节流
    /// 
    /// # 返回
    /// 
    /// 成功时返回序列化后的字节数据，失败时返回错误
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut result = Vec::new();
        
        // 1. 序列化头部
        let header_data = self.header.serialize()?;
        result.extend_from_slice(&header_data);
        
        // 2. 序列化所有数据块
        for chunk in &self.chunks {
            let chunk_data = chunk.serialize()?;
            result.extend_from_slice(&chunk_data);
        }
        
        // 3. 序列化TOC（如果存在）
        if let Some(ref toc) = self.toc {
            let toc_data = toc.serialize()?;
            result.extend_from_slice(&toc_data);
        }
        
        Ok(result)
    }

    /// 添加数据块到容器
    /// 
    /// # 参数
    /// 
    /// * `chunk` - 要添加的数据块
    pub fn add_chunk(&mut self, chunk: Chunk) {
        self.chunks.push(chunk);
    }

    /// 创建并添加数据块
    /// 
    /// # 参数
    /// 
    /// * `kind` - 块类型
    /// * `data` - 原始数据
    /// * `codec` - 压缩算法
    /// * `transform_flags` - 变换标志
    /// * `hash_algorithm` - 哈希算法
    /// 
    /// # 返回
    /// 
    /// 成功时返回块的索引，失败时返回错误
    pub fn add_data(&mut self, 
                   kind: ChunkKind, 
                   data: &[u8], 
                   codec: chunk::Codec, 
                   transform_flags: u16, 
                   hash_algorithm: u8) -> Result<usize> {
        let chunk = Chunk::new(kind, data, codec, transform_flags, hash_algorithm)?;
        let index = self.chunks.len();
        self.chunks.push(chunk);
        Ok(index)
    }

    /// 获取数据块数量
    /// 
    /// # 返回
    /// 
    /// 容器中的数据块数量
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// 获取指定索引的数据块
    /// 
    /// # 参数
    /// 
    /// * `index` - 数据块索引
    /// 
    /// # 返回
    /// 
    /// 数据块的引用，如果索引无效返回None
    pub fn get_chunk(&self, index: usize) -> Option<&Chunk> {
        self.chunks.get(index)
    }

    /// 设置TOC索引
    /// 
    /// # 参数
    /// 
    /// * `toc` - TOC索引结构
    pub fn set_toc(&mut self, toc: toc::Toc) {
        self.toc = Some(toc);
    }

    /// 估算序列化后的大小
    /// 
    /// # 返回
    /// 
    /// 估算的字节数
    pub fn estimated_size(&self) -> usize {
        let header_size = self.header.estimated_size();
        let chunks_size: usize = self.chunks.iter()
            .map(|chunk| chunk.estimated_serialized_size())
            .sum();
        let toc_size = self.toc.as_ref()
            .map(|_| 1024) // 简单估算TOC大小
            .unwrap_or(0);
        
        header_size + chunks_size + toc_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::hash_algorithms;

    #[test]
    fn test_container_creation() {
        let container = Container::new(Profile::Recd);
        
        assert_eq!(container.header.profile, Profile::Recd);
        assert_eq!(container.chunks.len(), 0);
        assert!(container.toc.is_none());
    }

    #[test]
    fn test_container_add_chunk() {
        let mut container = Container::new(Profile::Blob);
        let test_data = "测试数据".as_bytes();
        
        let chunk = Chunk::new(
            ChunkKind::DataNode,
            test_data,
            chunk::Codec::None,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        container.add_chunk(chunk);
        assert_eq!(container.chunk_count(), 1);
        
        let retrieved_chunk = container.get_chunk(0).unwrap();
        assert_eq!(retrieved_chunk.kind, ChunkKind::DataNode);
        assert_eq!(retrieved_chunk.raw_size, test_data.len() as u32);
    }

    #[test]
    fn test_container_add_data() {
        let mut container = Container::new(Profile::Recd);
        let test_data = "这是一个测试数据块".as_bytes();
        
        let index = container.add_data(
            ChunkKind::DataNode,
            test_data,
            chunk::Codec::None,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        assert_eq!(index, 0);
        assert_eq!(container.chunk_count(), 1);
        
        let chunk = container.get_chunk(0).unwrap();
        let recovered_data = chunk.get_raw_data().unwrap();
        assert_eq!(recovered_data.as_slice(), test_data);
    }

    #[test]
    fn test_container_serialization_roundtrip() {
        let mut container = Container::new(Profile::Recd);
        
        // 设置头部信息
        container.header.set_producer("universe测试");
        container.header.set_creation_timestamp_now();
        
        // 添加测试数据
        let test_data1 = "第一个数据块 - 包含中文内容".as_bytes();
        let test_data2 = "Second data block with mixed content 混合内容".as_bytes();
        
        container.add_data(
            ChunkKind::DataNode,
            test_data1,
            chunk::Codec::None,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        container.add_data(
            ChunkKind::Blob,
            test_data2,
            chunk::Codec::Zstd,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        // 序列化
        let serialized = container.serialize().unwrap();
        assert!(!serialized.is_empty());
        
        // 反序列化
        let deserialized = Container::deserialize(&serialized).unwrap();
        
        // 验证头部信息
        assert_eq!(deserialized.header.profile, Profile::Recd);
        assert_eq!(deserialized.header.get_producer(), container.header.get_producer());
        
        // 验证数据块
        assert_eq!(deserialized.chunk_count(), 2);
        
        let chunk1 = deserialized.get_chunk(0).unwrap();
        assert_eq!(chunk1.kind, ChunkKind::DataNode);
        let recovered_data1 = chunk1.get_raw_data().unwrap();
        assert_eq!(recovered_data1.as_slice(), test_data1);
        
        let chunk2 = deserialized.get_chunk(1).unwrap();
        assert_eq!(chunk2.kind, ChunkKind::Blob);
        let recovered_data2 = chunk2.get_raw_data().unwrap();
        assert_eq!(recovered_data2.as_slice(), test_data2);
    }

    #[test]
    fn test_container_with_toc() {
        let mut container = Container::new(Profile::Grph);
        
        // 添加数据块
        container.add_data(
            ChunkKind::DataNode,
            b"node data",
            chunk::Codec::None,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        // 创建并设置TOC
        let mut toc = toc::Toc::new();
        toc.add_root("main".to_string(), "node1".to_string()).unwrap();
        container.set_toc(toc);
        
        // 序列化和反序列化
        let serialized = container.serialize().unwrap();
        let deserialized = Container::deserialize(&serialized).unwrap();
        
        assert!(deserialized.toc.is_some());
        let toc = deserialized.toc.as_ref().unwrap();
        assert_eq!(toc.get_root("main"), Some(&"node1".to_string()));
    }

    #[test]
    fn test_container_empty_serialization() {
        let container = Container::new(Profile::Type);
        
        let serialized = container.serialize().unwrap();
        let deserialized = Container::deserialize(&serialized).unwrap();
        
        assert_eq!(deserialized.header.profile, Profile::Type);
        assert_eq!(deserialized.chunk_count(), 0);
        assert!(deserialized.toc.is_none());
    }

    #[test]
    fn test_container_invalid_deserialization() {
        // 测试空数据
        let result = Container::deserialize(&[]);
        assert!(result.is_err());
        
        // 测试无效魔数
        let invalid_data = vec![0u8; 32];
        let result = Container::deserialize(&invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_container_size_estimation() {
        let mut container = Container::new(Profile::Tabl);
        
        let initial_size = container.estimated_size();
        assert!(initial_size > 0);
        
        container.add_data(
            ChunkKind::DataNode,
            b"test data for size estimation",
            chunk::Codec::None,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        let size_with_chunk = container.estimated_size();
        assert!(size_with_chunk > initial_size);
    }

    #[test]
    fn test_different_profiles() {
        let profiles = [
            Profile::Blob,
            Profile::Recd,
            Profile::Tabl,
            Profile::Tsdb,
            Profile::Grph,
            Profile::Type,
        ];
        
        for profile in profiles {
            let container = Container::new(profile);
            let serialized = container.serialize().unwrap();
            let deserialized = Container::deserialize(&serialized).unwrap();
            
            assert_eq!(deserialized.header.profile, profile);
        }
    }

    #[test]
    fn test_multiple_chunk_types() {
        let mut container = Container::new(Profile::Mixd);
        
        // 添加不同类型的数据块
        container.add_data(ChunkKind::DataNode, b"data", chunk::Codec::None, 0, hash_algorithms::BLAKE3).unwrap();
        container.add_data(ChunkKind::Blob, b"blob", chunk::Codec::Lz4, 0, hash_algorithms::BLAKE3).unwrap();
        container.add_data(ChunkKind::Schema, b"schema", chunk::Codec::Zstd, 0, hash_algorithms::BLAKE3).unwrap();
        
        let serialized = container.serialize().unwrap();
        let deserialized = Container::deserialize(&serialized).unwrap();
        
        assert_eq!(deserialized.chunk_count(), 3);
        assert_eq!(deserialized.get_chunk(0).unwrap().kind, ChunkKind::DataNode);
        assert_eq!(deserialized.get_chunk(1).unwrap().kind, ChunkKind::Blob);
        assert_eq!(deserialized.get_chunk(2).unwrap().kind, ChunkKind::Schema);
    }

    #[test]
    fn test_comprehensive_integration() {
        // 这是一个综合集成测试，验证所有功能协同工作
        let mut container = Container::new(Profile::Grph);
        
        // 设置完整的头部信息
        container.header.set_producer("综合测试程序 v1.0");
        container.header.set_creation_timestamp_now();
        container.header.set_namespace_root("test.universe.integration");
        
        // 添加不同类型和压缩算法的数据
        let json_schema = r#"{"type": "object", "properties": {"name": {"type": "string"}}}"#;
        let binary_data: Vec<u8> = (0..=255).cycle().take(4000).collect(); // 4KB 的二进制数据
        let text_data = "这是一个包含中文的长文本数据，用于测试UTF-8编码和压缩效果。".repeat(50);
        
        // 添加 Schema 块（无压缩）
        container.add_data(
            ChunkKind::Schema,
            json_schema.as_bytes(),
            chunk::Codec::None,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        // 添加 Blob 块（ZSTD压缩）
        container.add_data(
            ChunkKind::Blob,
            &binary_data,
            chunk::Codec::Zstd,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        // 添加 DataNode 块（LZ4压缩）
        container.add_data(
            ChunkKind::DataNode,
            text_data.as_bytes(),
            chunk::Codec::Lz4,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
        
        // 创建并设置TOC
        let mut toc = toc::Toc::new();
        toc.add_root("schema".to_string(), "schema-node-1".to_string()).unwrap();
        toc.add_root("data".to_string(), "data-node-1".to_string()).unwrap();
        container.set_toc(toc);
        
        // 验证容器状态
        assert_eq!(container.chunk_count(), 3);
        assert!(container.toc.is_some());
        
        // 序列化
        let serialized = container.serialize().unwrap();
        assert!(serialized.len() > 100); // 确保有实际内容
        
        // 反序列化
        let deserialized = Container::deserialize(&serialized).unwrap();
        
        // 验证基本属性
        assert_eq!(deserialized.header.profile, Profile::Grph);
        assert_eq!(deserialized.chunk_count(), 3);
        assert!(deserialized.toc.is_some());
        
        // 验证头部信息
        assert_eq!(deserialized.header.get_producer(), Some("综合测试程序 v1.0"));
        assert_eq!(deserialized.header.get_namespace_root(), Some("test.universe.integration"));
        
        // 验证每个数据块
        let schema_chunk = deserialized.get_chunk(0).unwrap();
        assert_eq!(schema_chunk.kind, ChunkKind::Schema);
        assert_eq!(schema_chunk.codec, chunk::Codec::None);
        let recovered_schema = schema_chunk.get_raw_data().unwrap();
        assert_eq!(String::from_utf8(recovered_schema.to_vec()).unwrap(), json_schema);
        
        let blob_chunk = deserialized.get_chunk(1).unwrap();
        assert_eq!(blob_chunk.kind, ChunkKind::Blob);
        assert_eq!(blob_chunk.codec, chunk::Codec::Zstd);
        let recovered_blob = blob_chunk.get_raw_data().unwrap();
        assert_eq!(recovered_blob.to_vec(), binary_data);
        
        let data_chunk = deserialized.get_chunk(2).unwrap();
        assert_eq!(data_chunk.kind, ChunkKind::DataNode);
        assert_eq!(data_chunk.codec, chunk::Codec::Lz4);
        let recovered_text = data_chunk.get_raw_data().unwrap();
        assert_eq!(String::from_utf8(recovered_text.to_vec()).unwrap(), text_data);
        
        // 验证TOC
        let toc = deserialized.toc.as_ref().unwrap();
        assert_eq!(toc.get_root("schema"), Some(&"schema-node-1".to_string()));
        assert_eq!(toc.get_root("data"), Some(&"data-node-1".to_string()));
        
        // 验证压缩效果
        assert!(blob_chunk.compression_ratio() > 1.0, "二进制数据应该有压缩效果");
        assert!(data_chunk.compression_ratio() > 2.0, "重复文本应该有很好的压缩效果");
    }
}
