//! # TOC (Table of Contents) 索引系统
//!
//! 处理 UNIV 容器的目录索引，提供快速访问和查找功能。

use crate::error::{UnivError, Result};
use crate::constants::TOC_MAGIC;
use crate::util::varint::VarInt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use bytes::{Buf, BufMut, BytesMut};

/// TOC 目录结构
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Toc {
    /// 分片数量
    pub shard_count: u32,
    /// 分片偏移量列表
    pub shard_offsets: Vec<u64>,
    /// 块索引表
    pub chunks: HashMap<String, ChunkIndex>,
    /// 节点索引表
    pub nodes: HashMap<String, NodeIndex>,
    /// 引用索引表
    pub refs: HashMap<String, RefIndex>,
    /// 根节点集合
    pub roots: HashMap<String, String>, // name -> node_id
}

/// 块索引条目
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkIndex {
    /// 块标识符
    pub chunk_id: String,
    /// 文件偏移量
    pub offset: u64,
    /// 块大小
    pub size: u32,
    /// 块类型
    pub kind: u8,
    /// 压缩算法
    pub codec: u8,
    /// 原始大小
    pub raw_size: u32,
}

/// 节点索引条目
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeIndex {
    /// 节点标识符
    pub node_id: String,
    /// 对应的块标识符
    pub chunk_id: String,
    /// 节点类型
    pub node_type: String,
    /// 哈希策略
    pub hash_policy: u8,
}

/// 引用索引条目
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefIndex {
    /// 源节点标识符
    pub source_node: String,
    /// 目标节点标识符
    pub target_node: String,
    /// 是否为外部引用
    pub external: bool,
    /// 引用类型
    pub ref_type: Option<String>,
}

impl Toc {
    /// 创建新的 TOC
    pub fn new() -> Self {
        Self {
            shard_count: 0,
            shard_offsets: Vec::new(),
            chunks: HashMap::new(),
            nodes: HashMap::new(),
            refs: HashMap::new(),
            roots: HashMap::new(),
        }
    }

    /// 添加块索引
    /// 
    /// # 参数
    /// 
    /// * `chunk_index` - 块索引条目
    pub fn add_chunk(&mut self, chunk_index: ChunkIndex) {
        self.chunks.insert(chunk_index.chunk_id.clone(), chunk_index);
    }

    /// 添加节点索引
    /// 
    /// # 参数
    /// 
    /// * `node_index` - 节点索引条目
    pub fn add_node(&mut self, node_index: NodeIndex) {
        self.nodes.insert(node_index.node_id.clone(), node_index);
    }

    /// 添加引用索引
    /// 
    /// # 参数
    /// 
    /// * `ref_index` - 引用索引条目
    pub fn add_ref(&mut self, ref_index: RefIndex) {
        let key = format!("{}:{}", ref_index.source_node, ref_index.target_node);
        self.refs.insert(key, ref_index);
    }

    /// 添加根节点
    /// 
    /// # 参数
    /// 
    /// * `name` - 根节点名称
    /// * `node_id` - 节点标识符
    pub fn add_root(&mut self, name: String, node_id: String) -> Result<()> {
        if self.roots.contains_key(&name) {
            return Err(UnivError::index_error(format!("根节点名称重复: {}", name)));
        }
        
        self.roots.insert(name, node_id);
        Ok(())
    }

    /// 获取块索引
    /// 
    /// # 参数
    /// 
    /// * `chunk_id` - 块标识符
    /// 
    /// # 返回
    /// 
    /// 块索引条目的引用
    pub fn get_chunk(&self, chunk_id: &str) -> Option<&ChunkIndex> {
        self.chunks.get(chunk_id)
    }

    /// 获取节点索引
    /// 
    /// # 参数
    /// 
    /// * `node_id` - 节点标识符
    /// 
    /// # 返回
    /// 
    /// 节点索引条目的引用
    pub fn get_node(&self, node_id: &str) -> Option<&NodeIndex> {
        self.nodes.get(node_id)
    }

    /// 获取根节点
    /// 
    /// # 参数
    /// 
    /// * `name` - 根节点名称
    /// 
    /// # 返回
    /// 
    /// 节点标识符的引用
    pub fn get_root(&self, name: &str) -> Option<&String> {
        self.roots.get(name)
    }

    /// 获取主根节点
    /// 
    /// # 返回
    /// 
    /// 主根节点的标识符
    pub fn get_main_root(&self) -> Option<&String> {
        self.get_root("main")
            .or_else(|| self.get_root("_default"))
            .or_else(|| self.roots.values().next())
    }

    /// 查找节点的所有引用
    /// 
    /// # 参数
    /// 
    /// * `node_id` - 节点标识符
    /// 
    /// # 返回
    /// 
    /// 引用该节点的所有引用条目
    pub fn find_references_to(&self, node_id: &str) -> Vec<&RefIndex> {
        self.refs.values()
            .filter(|ref_index| ref_index.target_node == node_id)
            .collect()
    }

    /// 查找节点引用的所有节点
    /// 
    /// # 参数
    /// 
    /// * `node_id` - 节点标识符
    /// 
    /// # 返回
    /// 
    /// 该节点引用的所有引用条目
    pub fn find_references_from(&self, node_id: &str) -> Vec<&RefIndex> {
        self.refs.values()
            .filter(|ref_index| ref_index.source_node == node_id)
            .collect()
    }

    /// 序列化 TOC 结构为 CBOR 格式（内部格式）
    /// 
    /// # 返回
    /// 
    /// 序列化后的字节数据
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::ser::into_writer(self, &mut buf)
            .map_err(|e| UnivError::serialization_error(format!("TOC序列化失败: {}", e)))?;
        Ok(buf)
    }

    /// 序列化 TOC Footer（规范格式）
    /// 
    /// 根据 UNIV 规范生成 "TOC1" Footer 格式：
    /// Magic4("TOC1") + ShardCount(varint) + ShardOffsets[ShardCount](uint64) + CRC32C(uint32)
    /// 
    /// # 返回
    /// 
    /// 序列化后的 Footer 字节数据
    pub fn serialize_footer(&self) -> Result<Vec<u8>> {
        let mut buf = BytesMut::new();
        
        // 写入魔数 "TOC1"
        buf.put_slice(TOC_MAGIC);
        
        // 写入分片数量（varint）
        let shard_count_bytes = VarInt::encode_u64(self.shard_count as u64)?;
        buf.put_slice(&shard_count_bytes);
        
        // 写入分片偏移量列表（uint64 数组）
        for offset in &self.shard_offsets {
            buf.put_u64_le(*offset);
        }
        
        // 计算并写入 CRC32C
        let crc = crc32c::crc32c(&buf);
        buf.put_u32_le(crc);
        
        Ok(buf.to_vec())
    }

    /// 反序列化 TOC 结构（CBOR 格式）
    /// 
    /// # 参数
    /// 
    /// * `data` - 要解析的字节数据
    /// 
    /// # 返回
    /// 
    /// 解析后的 TOC 结构
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        ciborium::de::from_reader(data)
            .map_err(|e| UnivError::deserialization_error(format!("TOC反序列化失败: {}", e)))
    }

    /// 反序列化 TOC Footer（规范格式）
    /// 
    /// 解析 "TOC1" Footer 格式：
    /// Magic4("TOC1") + ShardCount(varint) + ShardOffsets[ShardCount](uint64) + CRC32C(uint32)
    /// 
    /// # 参数
    /// 
    /// * `data` - Footer 字节数据
    /// 
    /// # 返回
    /// 
    /// 解析后的分片偏移量信息和消耗的字节数
    pub fn deserialize_footer(data: &[u8]) -> Result<(Vec<u64>, usize)> {
        if data.len() < 8 {
            return Err(UnivError::IncompleteData {
                expected: 8,
                actual: data.len(),
            });
        }

        let mut buf = data;
        let original_len = buf.len();

        // 验证魔数 "TOC1"
        let mut magic = [0u8; 4];
        buf.copy_to_slice(&mut magic);
        if &magic != TOC_MAGIC {
            return Err(UnivError::deserialization_error("无效的TOC Footer魔数".to_string()));
        }

        // 读取分片数量（varint）
        let (shard_count, varint_len) = VarInt::decode_u64(buf)?;
        buf = &buf[varint_len..];

        // 读取分片偏移量列表
        let mut shard_offsets = Vec::with_capacity(shard_count as usize);
        for _ in 0..shard_count {
            if buf.len() < 8 {
                return Err(UnivError::IncompleteData {
                    expected: 8,
                    actual: buf.len(),
                });
            }
            shard_offsets.push(buf.get_u64_le());
        }

        // 验证 CRC32C
        if buf.len() < 4 {
            return Err(UnivError::IncompleteData {
                expected: 4,
                actual: buf.len(),
            });
        }
        let expected_crc = buf.get_u32_le();
        let footer_data_len = original_len - buf.len() - 4; // 不包括CRC本身
        let computed_crc = crc32c::crc32c(&data[..footer_data_len]);
        
        if expected_crc != computed_crc {
            return Err(UnivError::deserialization_error(format!(
                "TOC Footer CRC校验失败: 期望{:08x}, 实际{:08x}",
                expected_crc, computed_crc
            )));
        }

        let consumed = original_len - buf.len();
        Ok((shard_offsets, consumed))
    }
}

impl Default for Toc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toc_creation() {
        let toc = Toc::new();
        assert_eq!(toc.shard_count, 0);
        assert!(toc.chunks.is_empty());
        assert!(toc.nodes.is_empty());
        assert!(toc.refs.is_empty());
        assert!(toc.roots.is_empty());
    }

    #[test]
    fn test_chunk_operations() {
        let mut toc = Toc::new();
        
        let chunk_index = ChunkIndex {
            chunk_id: "chunk1".to_string(),
            offset: 100,
            size: 1024,
            kind: 1,
            codec: 0,
            raw_size: 1024,
        };
        
        toc.add_chunk(chunk_index.clone());
        
        let retrieved = toc.get_chunk("chunk1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().offset, 100);
        assert_eq!(retrieved.unwrap().size, 1024);
    }

    #[test]
    fn test_node_operations() {
        let mut toc = Toc::new();
        
        let node_index = NodeIndex {
            node_id: "node1".to_string(),
            chunk_id: "chunk1".to_string(),
            node_type: "DataNode".to_string(),
            hash_policy: 0,
        };
        
        toc.add_node(node_index.clone());
        
        let retrieved = toc.get_node("node1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().chunk_id, "chunk1");
    }

    #[test]
    fn test_root_operations() {
        let mut toc = Toc::new();
        
        assert!(toc.add_root("main".to_string(), "node1".to_string()).is_ok());
        assert!(toc.add_root("main".to_string(), "node2".to_string()).is_err()); // 重复名称
        
        let main_root = toc.get_root("main");
        assert!(main_root.is_some());
        assert_eq!(main_root.unwrap(), "node1");
        
        let main_root = toc.get_main_root();
        assert!(main_root.is_some());
        assert_eq!(main_root.unwrap(), "node1");
    }

    #[test]
    fn test_reference_operations() {
        let mut toc = Toc::new();
        
        let ref_index = RefIndex {
            source_node: "node1".to_string(),
            target_node: "node2".to_string(),
            external: false,
            ref_type: Some("dependency".to_string()),
        };
        
        toc.add_ref(ref_index);
        
        let refs_from_node1 = toc.find_references_from("node1");
        assert_eq!(refs_from_node1.len(), 1);
        assert_eq!(refs_from_node1[0].target_node, "node2");
        
        let refs_to_node2 = toc.find_references_to("node2");
        assert_eq!(refs_to_node2.len(), 1);
        assert_eq!(refs_to_node2[0].source_node, "node1");
    }

    #[test]
    fn test_toc_serialization() {
        let mut toc = Toc::new();
        toc.add_root("main".to_string(), "node1".to_string()).unwrap();
        
        let serialized = toc.serialize().unwrap();
        let deserialized = Toc::deserialize(&serialized).unwrap();
        
        assert_eq!(deserialized.roots.len(), 1);
        assert_eq!(deserialized.get_root("main"), Some(&"node1".to_string()));
    }

    #[test]
    fn test_toc_footer_serialization() {
        let mut toc = Toc::new();
        toc.shard_count = 2;
        toc.shard_offsets = vec![0x1000, 0x2000];
        
        // 序列化 Footer
        let footer_data = toc.serialize_footer().unwrap();
        
        // 验证魔数
        assert_eq!(&footer_data[0..4], b"TOC1");
        
        // 反序列化 Footer
        let (shard_offsets, consumed) = Toc::deserialize_footer(&footer_data).unwrap();
        assert_eq!(consumed, footer_data.len());
        assert_eq!(shard_offsets.len(), 2);
        assert_eq!(shard_offsets[0], 0x1000);
        assert_eq!(shard_offsets[1], 0x2000);
    }

    #[test]
    fn test_toc_footer_crc_validation() {
        let mut toc = Toc::new();
        toc.shard_count = 1;
        toc.shard_offsets = vec![0x1000];
        
        let mut footer_data = toc.serialize_footer().unwrap();
        
        // 损坏 CRC 数据
        let last_idx = footer_data.len() - 1;
        footer_data[last_idx] ^= 0xFF;
        
        // 反序列化应该失败
        let result = Toc::deserialize_footer(&footer_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CRC校验失败"));
    }
}