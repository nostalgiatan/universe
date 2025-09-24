//! # UNIV 基础使用示例
//!
//! 演示如何使用 UNIV 库的核心功能

use universe::{
    Container, Profile, Header, Chunk, ChunkKind,
    constants::hash_algorithms,
    util::hash::ContentHash,
    reference::{DataNode, ReferenceGraph},
    transform::StringDictionary,
    security::SecurityContext,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== UNIV 库基础使用示例 ===\n");

    // 1. 创建和操作 Profile
    demo_profiles()?;
    
    // 2. 创建和操作文件头
    demo_header()?;
    
    // 3. 创建和操作数据块
    demo_chunks()?;
    
    // 4. 演示哈希功能
    demo_hashing()?;
    
    // 5. 演示引用系统
    demo_references()?;
    
    // 6. 演示变换功能
    demo_transforms()?;
    
    // 7. 演示安全功能
    demo_security()?;
    
    // 8. 演示容器序列化功能
    demo_container_serialization()?;

    println!("\n=== 示例执行完成 ===");
    Ok(())
}

/// 演示 Profile 功能
fn demo_profiles() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Profile 功能演示");
    
    // 创建不同类型的 Profile
    let recd_profile = Profile::Recd;
    let blob_profile = Profile::Blob;
    let type_profile = Profile::Type;
    
    println!("   RECD Profile: {} ({})", recd_profile, recd_profile.description());
    println!("   BLOB Profile: {} ({})", blob_profile, blob_profile.description());
    println!("   TYPE Profile: {} ({})", type_profile, type_profile.description());
    
    // 检查 Profile 属性
    println!("   RECD 是否稳定: {}", recd_profile.is_stable());
    println!("   BLOB 支持的块类型数量: {}", blob_profile.supported_chunk_kinds().len());
    
    // 验证变换
    if let Err(e) = type_profile.validate_transforms(0x04) {
        println!("   TYPE Profile 变换验证失败（预期）: {}", e);
    }
    
    println!();
    Ok(())
}

/// 演示文件头功能
fn demo_header() -> Result<(), Box<dyn std::error::Error>> {
    println!("2. 文件头功能演示");
    
    // 创建文件头
    let mut header = Header::new(Profile::Recd);
    
    // 添加扩展信息
    header.set_producer("universe-rust-example");
    header.set_creation_timestamp_now();
    header.set_namespace_root("org.example.demo");
    
    println!("   生产者: {:?}", header.get_producer());
    println!("   命名空间: {:?}", header.get_namespace_root());
    println!("   创建时间: {:?}", header.get_creation_timestamp());
    
    // 序列化和反序列化
    let serialized = header.serialize()?;
    println!("   序列化后大小: {} 字节", serialized.len());
    
    let (deserialized, consumed) = Header::deserialize(&serialized)?;
    println!("   反序列化消费: {} 字节", consumed);
    println!("   Profile 匹配: {}", deserialized.profile == header.profile);
    
    println!();
    Ok(())
}

/// 演示数据块功能
fn demo_chunks() -> Result<(), Box<dyn std::error::Error>> {
    println!("3. 数据块功能演示");
    
    // 创建测试数据
    let test_data = "这是一个测试数据块，包含中文内容，用于演示压缩效果。".repeat(10).into_bytes();
    println!("   原始数据大小: {} 字节", test_data.len());
    
    // 使用不同压缩算法创建块
    let chunk_none = Chunk::new(
        ChunkKind::DataNode,
        &test_data,
        universe::chunk::Codec::None,
        0,
        hash_algorithms::BLAKE3,
    )?;
    
    let chunk_zstd = Chunk::new(
        ChunkKind::DataNode,
        &test_data,
        universe::chunk::Codec::Zstd,
        0,
        hash_algorithms::BLAKE3,
    )?;
    
    println!("   无压缩块大小: {} 字节，压缩比: {:.2}", 
             chunk_none.compressed_size, chunk_none.compression_ratio());
    println!("   Zstd压缩块大小: {} 字节，压缩比: {:.2}", 
             chunk_zstd.compressed_size, chunk_zstd.compression_ratio());
    
    // 验证完整性
    chunk_none.verify()?;
    chunk_zstd.verify()?;
    println!("   数据完整性验证通过");
    
    // 序列化和反序列化
    let serialized = chunk_zstd.serialize()?;
    let (deserialized, _) = Chunk::deserialize(&serialized)?;
    let recovered_data = deserialized.get_raw_data()?;
    println!("   序列化后恢复数据匹配: {}", recovered_data == test_data);
    
    println!();
    Ok(())
}

/// 演示哈希功能
fn demo_hashing() -> Result<(), Box<dyn std::error::Error>> {
    println!("4. 哈希功能演示");
    
    let test_data = b"Hello, UNIV World!";
    
    // 使用不同哈希算法
    let blake3_hash = ContentHash::new(hash_algorithms::BLAKE3, test_data)?;
    let crc32c_hash = ContentHash::new(hash_algorithms::CRC32C, test_data)?;
    
    println!("   BLAKE3: {}", blake3_hash);
    println!("   CRC32C: {}", crc32c_hash);
    
    // 验证哈希
    blake3_hash.verify(test_data)?;
    crc32c_hash.verify(test_data)?;
    println!("   哈希验证通过");
    
    // 解析哈希字符串
    let parsed_hash: ContentHash = blake3_hash.to_string().parse()?;
    println!("   解析后的哈希匹配: {}", parsed_hash == blake3_hash);
    
    println!();
    Ok(())
}

/// 演示引用系统
fn demo_references() -> Result<(), Box<dyn std::error::Error>> {
    println!("5. 引用系统演示");
    
    // 创建数据节点
    let data1 = "第一个数据节点".as_bytes().to_vec();
    let data2 = "第二个数据节点".as_bytes().to_vec();
    let data3 = "第三个数据节点".as_bytes().to_vec();
    
    let node1 = DataNode::new(data1, hash_algorithms::BLAKE3)?;
    let mut node2 = DataNode::new(data2, hash_algorithms::BLAKE3)?;
    let mut node3 = DataNode::new(data3, hash_algorithms::BLAKE3)?;
    
    // 创建引用关系
    node2.add_internal_reference(&node1);
    node3.add_internal_reference(&node1);
    node3.add_internal_reference(&node2);
    node3.add_external_reference("urn:univ:org.example:ExternalType:1.0.0".to_string())?;
    
    println!("   节点1 ID: {}", node1.id);
    println!("   节点2 引用数: {}", node2.references.len());
    println!("   节点3 内部引用数: {}", node3.get_internal_references().len());
    println!("   节点3 外部引用数: {}", node3.get_external_references().len());
    
    // 创建引用图
    let mut graph = ReferenceGraph::new();
    let _node1_id = node1.id.clone();
    let _node2_id = node2.id.clone();
    let node3_id = node3.id.clone();
    
    graph.add_node(node1)?;
    graph.add_node(node2)?;
    graph.add_node(node3)?;
    
    // 分析引用关系
    let reachable = graph.get_reachable_nodes(&node3_id);
    println!("   从节点3可达的节点数: {}", reachable.len());
    
    let stats = graph.statistics();
    println!("   图统计 - 节点: {}, 内部引用: {}, 外部引用: {}", 
             stats.node_count, stats.internal_reference_count, stats.external_reference_count);
    
    // 检查循环
    graph.check_cycles()?;
    println!("   循环检测通过");
    
    println!();
    Ok(())
}

/// 演示变换功能
fn demo_transforms() -> Result<(), Box<dyn std::error::Error>> {
    println!("6. 变换功能演示");
    
    // 字符串字典
    let mut dict = StringDictionary::new();
    
    let strings = vec!["hello", "world", "universe", "hello", "world"];
    let mut indices = Vec::new();
    
    for s in &strings {
        let index = dict.add_string(s.to_string());
        indices.push(index);
    }
    
    println!("   原始字符串: {:?}", strings);
    println!("   字典索引: {:?}", indices);
    println!("   字典大小: {} 个唯一字符串", dict.len());
    
    // 序列化字典
    let serialized = dict.serialize()?;
    let deserialized = StringDictionary::deserialize(&serialized)?;
    println!("   字典序列化/反序列化成功，大小: {} 字节", serialized.len());
    
    // 验证字典内容
    for (i, &index) in indices.iter().enumerate() {
        let original = strings[i];
        let recovered = deserialized.get_string(index).unwrap();
        assert_eq!(original, recovered);
    }
    println!("   字典内容验证通过");
    
    println!();
    Ok(())
}

/// 演示安全功能
fn demo_security() -> Result<(), Box<dyn std::error::Error>> {
    println!("7. 安全功能演示");
    
    // 创建安全上下文
    let mut security_context = SecurityContext::new();
    
    // 模拟添加一些块
    use universe::security::SecurityStatsUpdate;
    
    security_context.update_stats(SecurityStatsUpdate::AddChunk {
        chunk_type: 1,
        raw_size: 1024,
        compressed_size: 512,
    });
    
    security_context.update_stats(SecurityStatsUpdate::AddChunk {
        chunk_type: 2,
        raw_size: 2048,
        compressed_size: 1024,
    });
    
    security_context.update_stats(SecurityStatsUpdate::SetReferenceDepth(5));
    
    println!("   块数量: {}", security_context.stats.chunk_count);
    println!("   总原始大小: {} 字节", security_context.stats.total_raw_size);
    println!("   平均压缩比: {:.2}", security_context.stats.average_compression_ratio());
    println!("   最大引用深度: {:?}", security_context.stats.max_reference_depth);
    
    // 验证容器安全性
    security_context.validate_container()?;
    println!("   容器安全验证通过");
    
    // 验证单个块
    security_context.validate_chunk(1024, 512)?;
    println!("   块安全验证通过");
    
    // 检查限制违规
    let violations = security_context.stats.check_limits(&security_context.validator);
    println!("   安全限制违规: {} 个", violations.len());
    
    println!();
    Ok(())
}

/// 演示容器序列化和反序列化功能
fn demo_container_serialization() -> Result<(), Box<dyn std::error::Error>> {
    println!("8. 容器序列化功能演示");
    
    // 创建一个新容器
    let mut container = Container::new(Profile::Recd);
    
    // 设置头部信息
    container.header.set_producer("UNIV示例程序");
    container.header.set_creation_timestamp_now();
    
    println!("   创建了 {:?} 类型的容器", container.header.profile);
    
    // 添加多种类型的数据
    let data1 = "第一个数据块：包含中文字符的测试数据";
    let data2 = b"Second data block: Binary data with mixed content";
    let data3 = r#"{"type": "json", "content": "第三个数据块", "encoding": "UTF-8"}"#;
    
    container.add_data(
        ChunkKind::DataNode,
        data1.as_bytes(),
        universe::chunk::Codec::None,
        0,
        hash_algorithms::BLAKE3,
    )?;
    
    container.add_data(
        ChunkKind::Blob,
        data2,
        universe::chunk::Codec::Zstd,
        0,
        hash_algorithms::BLAKE3,
    )?;
    
    container.add_data(
        ChunkKind::Schema,
        data3.as_bytes(),
        universe::chunk::Codec::Lz4,
        0,
        hash_algorithms::BLAKE3,
    )?;
    
    println!("   添加了 {} 个数据块", container.chunk_count());
    
    // 序列化容器
    let serialized_data = container.serialize()?;
    println!("   序列化后大小: {} 字节", serialized_data.len());
    println!("   估算大小: {} 字节", container.estimated_size());
    
    // 反序列化容器
    let deserialized_container = Container::deserialize(&serialized_data)?;
    
    // 验证反序列化结果
    println!("   反序列化成功！");
    println!("   Profile: {:?}", deserialized_container.header.profile);
    println!("   数据块数量: {}", deserialized_container.chunk_count());
    println!("   生产者: {:?}", deserialized_container.header.get_producer());
    
    // 验证数据完整性
    for i in 0..deserialized_container.chunk_count() {
        let chunk = deserialized_container.get_chunk(i).unwrap();
        let recovered_data = chunk.get_raw_data()?;
        println!("   块 {} - 类型: {:?}, 大小: {} 字节, 压缩比: {:.2}", 
                 i, chunk.kind, recovered_data.len(), chunk.compression_ratio());
    }
    
    // 验证第一个块的内容
    let first_chunk = deserialized_container.get_chunk(0).unwrap();
    let recovered_data = first_chunk.get_raw_data()?;
    let recovered_text = String::from_utf8(recovered_data.to_vec())?;
    
    if recovered_text == data1 {
        println!("   ✓ 第一个数据块内容验证成功");
    } else {
        println!("   ✗ 第一个数据块内容验证失败");
    }
    
    println!();
    Ok(())
}