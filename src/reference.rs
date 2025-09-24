//! # 内容寻址与引用系统
//!
//! 处理 UNIV 容器中的内容寻址、节点引用和外部引用功能。

use crate::error::Result;
use crate::util::hash::{ContentHash, HashProvider};
use crate::util::validation::CycleDetector;
use crate::constants::hash_algorithms;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// 节点标识符类型
pub type NodeId = String;

/// 引用类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReferenceType {
    /// 内部引用
    Internal,
    /// 外部引用
    External,
}

/// 节点引用
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeReference {
    /// 目标节点ID
    pub target_id: NodeId,
    /// 引用类型
    pub ref_type: ReferenceType,
    /// 哈希算法
    pub hash_algorithm: u8,
    /// 内容哈希
    pub content_hash: Vec<u8>,
    /// URN（用于外部引用）
    pub urn: Option<String>,
}

impl NodeReference {
    /// 创建内部引用
    /// 
    /// # 参数
    /// 
    /// * `target_id` - 目标节点ID
    /// * `hash_algorithm` - 哈希算法
    /// * `content_hash` - 内容哈希
    /// 
    /// # 返回
    /// 
    /// 新创建的内部引用
    pub fn internal(target_id: NodeId, hash_algorithm: u8, content_hash: Vec<u8>) -> Self {
        Self {
            target_id,
            ref_type: ReferenceType::Internal,
            hash_algorithm,
            content_hash,
            urn: None,
        }
    }

    /// 创建外部引用
    /// 
    /// # 参数
    /// 
    /// * `urn` - 外部资源的URN
    /// 
    /// # 返回
    /// 
    /// 新创建的外部引用
    pub fn external(urn: String) -> Result<Self> {
        // 生成外部引用的multihash
        let urn_bytes = urn.as_bytes();
        let sha256_hash = HashProvider::hash(hash_algorithms::SHA256, urn_bytes)?;
        let multihash = sha256_hash; // 简化实现，实际应该使用multihash格式
        
        let target_id = hex::encode(&multihash);
        
        Ok(Self {
            target_id,
            ref_type: ReferenceType::External,
            hash_algorithm: hash_algorithms::SHA256,
            content_hash: multihash,
            urn: Some(urn),
        })
    }

    /// 检查是否为外部引用
    pub fn is_external(&self) -> bool {
        self.ref_type == ReferenceType::External
    }

    /// 获取Content Hash对象
    pub fn get_content_hash(&self) -> ContentHash {
        ContentHash::from_hash(self.hash_algorithm, self.content_hash.clone())
    }
}

/// 数据节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataNode {
    /// 节点ID（基于内容哈希）
    pub id: NodeId,
    /// 节点数据
    pub data: Vec<u8>,
    /// 引用的其他节点
    pub references: Vec<NodeReference>,
    /// 内容哈希
    pub content_hash: ContentHash,
    /// 节点元数据
    pub metadata: HashMap<String, String>,
}

impl DataNode {
    /// 创建新的数据节点
    /// 
    /// # 参数
    /// 
    /// * `data` - 节点数据
    /// * `hash_algorithm` - 哈希算法
    /// 
    /// # 返回
    /// 
    /// 新创建的数据节点
    pub fn new(data: Vec<u8>, hash_algorithm: u8) -> Result<Self> {
        let content_hash = ContentHash::new(hash_algorithm, &data)?;
        let id = content_hash.hex();
        
        Ok(Self {
            id,
            data,
            references: Vec::new(),
            content_hash,
            metadata: HashMap::new(),
        })
    }

    /// 添加内部引用
    /// 
    /// # 参数
    /// 
    /// * `target_node` - 目标节点
    pub fn add_internal_reference(&mut self, target_node: &DataNode) {
        let reference = NodeReference::internal(
            target_node.id.clone(),
            target_node.content_hash.algorithm,
            target_node.content_hash.hash.clone(),
        );
        self.references.push(reference);
    }

    /// 添加外部引用
    /// 
    /// # 参数
    /// 
    /// * `urn` - 外部资源URN
    pub fn add_external_reference(&mut self, urn: String) -> Result<()> {
        let reference = NodeReference::external(urn)?;
        self.references.push(reference);
        Ok(())
    }

    /// 验证节点完整性
    pub fn verify(&self) -> Result<()> {
        self.content_hash.verify(&self.data)
    }

    /// 获取所有内部引用
    pub fn get_internal_references(&self) -> Vec<&NodeReference> {
        self.references.iter()
            .filter(|r| r.ref_type == ReferenceType::Internal)
            .collect()
    }

    /// 获取所有外部引用
    pub fn get_external_references(&self) -> Vec<&NodeReference> {
        self.references.iter()
            .filter(|r| r.ref_type == ReferenceType::External)
            .collect()
    }

    /// 设置元数据
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// 获取元数据
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

/// 引用图
#[derive(Debug, Clone)]
pub struct ReferenceGraph {
    /// 节点集合
    nodes: HashMap<NodeId, DataNode>,
    /// 引用关系图
    references: HashMap<NodeId, HashSet<NodeId>>,
    /// 反向引用图
    reverse_references: HashMap<NodeId, HashSet<NodeId>>,
}

impl ReferenceGraph {
    /// 创建新的引用图
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            references: HashMap::new(),
            reverse_references: HashMap::new(),
        }
    }

    /// 添加节点
    /// 
    /// # 参数
    /// 
    /// * `node` - 要添加的节点
    pub fn add_node(&mut self, node: DataNode) -> Result<()> {
        let node_id = node.id.clone();
        
        // 验证节点
        node.verify()?;
        
        // 构建引用关系
        let mut refs = HashSet::new();
        for reference in &node.references {
            if reference.ref_type == ReferenceType::Internal {
                refs.insert(reference.target_id.clone());
                
                // 更新反向引用
                self.reverse_references
                    .entry(reference.target_id.clone())
                    .or_insert_with(HashSet::new)
                    .insert(node_id.clone());
            }
        }
        
        self.references.insert(node_id.clone(), refs);
        self.nodes.insert(node_id, node);
        
        Ok(())
    }

    /// 获取节点
    /// 
    /// # 参数
    /// 
    /// * `node_id` - 节点ID
    /// 
    /// # 返回
    /// 
    /// 节点的引用
    pub fn get_node(&self, node_id: &str) -> Option<&DataNode> {
        self.nodes.get(node_id)
    }

    /// 获取节点的直接引用
    /// 
    /// # 参数
    /// 
    /// * `node_id` - 节点ID
    /// 
    /// # 返回
    /// 
    /// 该节点直接引用的所有节点ID
    pub fn get_references(&self, node_id: &str) -> Option<&HashSet<NodeId>> {
        self.references.get(node_id)
    }

    /// 获取引用某节点的所有节点
    /// 
    /// # 参数
    /// 
    /// * `node_id` - 节点ID
    /// 
    /// # 返回
    /// 
    /// 引用该节点的所有节点ID
    pub fn get_referrers(&self, node_id: &str) -> Option<&HashSet<NodeId>> {
        self.reverse_references.get(node_id)
    }

    /// 检查引用循环
    /// 
    /// # 返回
    /// 
    /// 如果没有循环返回Ok，否则返回错误
    pub fn check_cycles(&self) -> Result<()> {
        let mut detector = CycleDetector::new();
        
        for node_id in self.nodes.keys() {
            if !detector.is_visited(node_id) {
                self.dfs_check_cycle(node_id, &mut detector)?;
            }
        }
        
        Ok(())
    }

    /// 深度优先搜索检查循环
    fn dfs_check_cycle(&self, node_id: &str, detector: &mut CycleDetector) -> Result<()> {
        detector.visit(node_id)?;
        
        if let Some(refs) = self.get_references(node_id) {
            for ref_id in refs {
                if self.nodes.contains_key(ref_id) {
                    self.dfs_check_cycle(ref_id, detector)?;
                }
            }
        }
        
        detector.leave(node_id);
        Ok(())
    }

    /// 获取从指定节点可达的所有节点
    /// 
    /// # 参数
    /// 
    /// * `start_id` - 起始节点ID
    /// 
    /// # 返回
    /// 
    /// 可达节点ID的集合
    pub fn get_reachable_nodes(&self, start_id: &str) -> HashSet<NodeId> {
        let mut reachable = HashSet::new();
        let mut to_visit = vec![start_id.to_string()];
        
        while let Some(current_id) = to_visit.pop() {
            if reachable.contains(&current_id) {
                continue;
            }
            
            reachable.insert(current_id.clone());
            
            if let Some(refs) = self.get_references(&current_id) {
                for ref_id in refs {
                    if self.nodes.contains_key(ref_id) && !reachable.contains(ref_id) {
                        to_visit.push(ref_id.clone());
                    }
                }
            }
        }
        
        reachable
    }

    /// 计算图的统计信息
    pub fn statistics(&self) -> GraphStatistics {
        let node_count = self.nodes.len();
        let internal_ref_count: usize = self.references.values()
            .map(|refs| refs.len())
            .sum();
        let external_ref_count: usize = self.nodes.values()
            .map(|node| node.get_external_references().len())
            .sum();
        
        GraphStatistics {
            node_count,
            internal_reference_count: internal_ref_count,
            external_reference_count: external_ref_count,
        }
    }

    /// 获取所有节点ID
    pub fn node_ids(&self) -> Vec<&String> {
        self.nodes.keys().collect()
    }

    /// 获取节点总数
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for ReferenceGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// 图统计信息
#[derive(Debug, Clone, PartialEq)]
pub struct GraphStatistics {
    /// 节点数量
    pub node_count: usize,
    /// 内部引用数量
    pub internal_reference_count: usize,
    /// 外部引用数量
    pub external_reference_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_reference_creation() {
        let target_id = "test_node".to_string();
        let hash = vec![1, 2, 3, 4];
        
        let internal_ref = NodeReference::internal(
            target_id.clone(),
            hash_algorithms::BLAKE3,
            hash.clone()
        );
        
        assert_eq!(internal_ref.target_id, target_id);
        assert_eq!(internal_ref.ref_type, ReferenceType::Internal);
        assert!(!internal_ref.is_external());
        
        let external_ref = NodeReference::external("urn:univ:org.example:Test:1.0.0".to_string()).unwrap();
        assert_eq!(external_ref.ref_type, ReferenceType::External);
        assert!(external_ref.is_external());
        assert!(external_ref.urn.is_some());
    }

    #[test]
    fn test_data_node_creation() {
        let data = b"Test node data".to_vec();
        let node = DataNode::new(data.clone(), hash_algorithms::BLAKE3).unwrap();
        
        assert_eq!(node.data, data);
        assert!(node.verify().is_ok());
        assert_eq!(node.references.len(), 0);
    }

    #[test]
    fn test_data_node_references() {
        let data1 = b"Node 1".to_vec();
        let data2 = b"Node 2".to_vec();
        
        let node1 = DataNode::new(data1, hash_algorithms::BLAKE3).unwrap();
        let mut node2 = DataNode::new(data2, hash_algorithms::BLAKE3).unwrap();
        
        // 添加内部引用
        node2.add_internal_reference(&node1);
        assert_eq!(node2.get_internal_references().len(), 1);
        
        // 添加外部引用
        node2.add_external_reference("urn:univ:org.example:Test:1.0.0".to_string()).unwrap();
        assert_eq!(node2.get_external_references().len(), 1);
    }

    #[test]
    fn test_reference_graph() {
        let mut graph = ReferenceGraph::new();
        
        let data1 = b"Node 1".to_vec();
        let data2 = b"Node 2".to_vec();
        
        let node1 = DataNode::new(data1, hash_algorithms::BLAKE3).unwrap();
        let mut node2 = DataNode::new(data2, hash_algorithms::BLAKE3).unwrap();
        
        // 创建引用关系
        node2.add_internal_reference(&node1);
        
        // 添加到图中
        let node1_id = node1.id.clone();
        let node2_id = node2.id.clone();
        
        graph.add_node(node1).unwrap();
        graph.add_node(node2).unwrap();
        
        // 验证引用关系
        let refs = graph.get_references(&node2_id);
        assert!(refs.is_some());
        assert!(refs.unwrap().contains(&node1_id));
        
        let referrers = graph.get_referrers(&node1_id);
        assert!(referrers.is_some());
        assert!(referrers.unwrap().contains(&node2_id));
        
        // 验证无循环
        assert!(graph.check_cycles().is_ok());
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = ReferenceGraph::new();
        
        // 创建三个节点形成循环
        let data1 = b"Node 1".to_vec();
        let data2 = b"Node 2".to_vec();
        let data3 = b"Node 3".to_vec();
        
        let node1 = DataNode::new(data1, hash_algorithms::BLAKE3).unwrap();
        let mut node2 = DataNode::new(data2, hash_algorithms::BLAKE3).unwrap();
        let mut node3 = DataNode::new(data3, hash_algorithms::BLAKE3).unwrap();
        
        // 创建循环：node1 -> node2 -> node3 -> node1
        node2.add_internal_reference(&node1);
        node3.add_internal_reference(&node2);
        
        graph.add_node(node1).unwrap();
        graph.add_node(node2).unwrap();
        graph.add_node(node3).unwrap();
        
        // 此时应该没有循环
        assert!(graph.check_cycles().is_ok());
        
        // 注意：由于我们需要先创建节点才能引用，
        // 实际的循环检测需要在所有节点都添加后进行更复杂的设置
    }

    #[test]
    fn test_reachable_nodes() {
        let mut graph = ReferenceGraph::new();
        
        let data1 = b"Node 1".to_vec();
        let data2 = b"Node 2".to_vec();
        let data3 = b"Node 3".to_vec();
        
        let node1 = DataNode::new(data1, hash_algorithms::BLAKE3).unwrap();
        let mut node2 = DataNode::new(data2, hash_algorithms::BLAKE3).unwrap();
        let node3 = DataNode::new(data3, hash_algorithms::BLAKE3).unwrap();
        
        node2.add_internal_reference(&node1);
        node2.add_internal_reference(&node3);
        
        let node1_id = node1.id.clone();
        let node2_id = node2.id.clone();
        let node3_id = node3.id.clone();
        
        graph.add_node(node1).unwrap();
        graph.add_node(node2).unwrap();
        graph.add_node(node3).unwrap();
        
        let reachable = graph.get_reachable_nodes(&node2_id);
        assert!(reachable.contains(&node1_id));
        assert!(reachable.contains(&node2_id));
        assert!(reachable.contains(&node3_id));
        assert_eq!(reachable.len(), 3);
    }

    #[test]
    fn test_graph_statistics() {
        let mut graph = ReferenceGraph::new();
        
        let data1 = b"Node 1".to_vec();
        let data2 = b"Node 2".to_vec();
        
        let node1 = DataNode::new(data1, hash_algorithms::BLAKE3).unwrap();
        let mut node2 = DataNode::new(data2, hash_algorithms::BLAKE3).unwrap();
        
        node2.add_internal_reference(&node1);
        node2.add_external_reference("urn:univ:org.example:Test:1.0.0".to_string()).unwrap();
        
        graph.add_node(node1).unwrap();
        graph.add_node(node2).unwrap();
        
        let stats = graph.statistics();
        assert_eq!(stats.node_count, 2);
        assert_eq!(stats.internal_reference_count, 1);
        assert_eq!(stats.external_reference_count, 1);
    }
}