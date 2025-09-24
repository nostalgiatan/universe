//! # 内存使用对比测试
//!
//! 详细对比传统方式和零拷贝优化的内存使用差异

use std::time::Instant;
use universe::{Container, Profile, chunk::{ChunkKind, Codec}, constants::hash_algorithms};

/// 创建测试数据
fn create_test_data(size: usize) -> Vec<u8> {
    let pattern = "这是一个测试数据块，包含中文内容，用于演示压缩效果。This is test data with English content for compression demonstration.".as_bytes();
    let repeat_count = (size + pattern.len() - 1) / pattern.len();
    pattern.repeat(repeat_count)[..size].to_vec()
}

/// 创建包含多个块的测试容器
fn create_test_container(chunk_count: usize, chunk_size: usize, codec: Codec) -> Container {
    let mut container = Container::new(Profile::Blob);
    
    for _i in 0..chunk_count {
        let data = create_test_data(chunk_size);
        container.add_data(
            ChunkKind::Blob,
            &data,
            codec,
            0,
            hash_algorithms::BLAKE3,
        ).unwrap();
    }
    
    container
}

fn main() {
    println!("# UNIV 容器内存使用对比测试\n");
    
    // 测试不同大小的容器
    let test_cases = vec![
        ("500KB", 25, 20_000, Codec::Zstd),
        ("2MB", 40, 50_000, Codec::Zstd),
        ("10MB", 100, 100_000, Codec::Zstd),
        ("50MB", 250, 200_000, Codec::Zstd),
    ];
    
    for (size_name, chunk_count, chunk_size, codec) in test_cases {
        println!("## 测试用例: {} ({} 块, 每块 ~{}字节, {:?}压缩)", 
                 size_name, chunk_count, chunk_size, codec);
        
        // 创建并序列化测试容器
        let container = create_test_container(chunk_count, chunk_size, codec);
        let serialized = container.serialize().unwrap();
        let file_size = serialized.len();
        
        println!("   序列化后大小: {} 字节 ({:.2} MB)", file_size, file_size as f64 / 1024.0 / 1024.0);
        
        // 传统反序列化（复制所有载荷数据）
        let start = Instant::now();
        let traditional_container = Container::deserialize(&serialized).unwrap();
        let traditional_deserialize_time = start.elapsed();
        
        // 零拷贝反序列化
        let start = Instant::now();
        let zero_copy_container = Container::deserialize_zero_copy(&serialized).unwrap();
        let zero_copy_deserialize_time = start.elapsed();
        
        println!("   传统反序列化时间: {:.2}ms", traditional_deserialize_time.as_secs_f64() * 1000.0);
        println!("   零拷贝反序列化时间: {:.2}ms", zero_copy_deserialize_time.as_secs_f64() * 1000.0);
        
        // 内存使用统计
        let traditional_stats = traditional_container.get_memory_stats(file_size);
        let zero_copy_stats = zero_copy_container.get_memory_stats(file_size);
        
        println!("   ### 内存使用对比:");
        println!("   传统方式:");
        println!("     - 拥有载荷: {} 字节", traditional_stats.owned_payload_size);
        println!("     - 共享载荷: {} 字节", traditional_stats.shared_payload_size);
        println!("     - 内存放大: {:.2}x", traditional_stats.traditional_memory_amplification());
        
        println!("   零拷贝方式:");
        println!("     - 拥有载荷: {} 字节", zero_copy_stats.owned_payload_size);
        println!("     - 共享载荷: {} 字节", zero_copy_stats.shared_payload_size);
        println!("     - 内存放大: {:.2}x", zero_copy_stats.memory_amplification());
        println!("     - 节省内存: {} 字节 ({:.1}%)", 
                 zero_copy_stats.zero_copy_savings(),
                 zero_copy_stats.zero_copy_savings() as f64 / file_size as f64 * 100.0);
        
        // 验证性能对比
        let iterations = 3;
        
        // 串行验证
        let start = Instant::now();
        for _ in 0..iterations {
            zero_copy_container.verify_serial().unwrap();
        }
        let serial_time = start.elapsed();
        
        // 并行验证
        let start = Instant::now();
        for _ in 0..iterations {
            zero_copy_container.verify_parallel().unwrap();
        }
        let parallel_time = start.elapsed();
        
        let serial_avg = serial_time.as_secs_f64() * 1000.0 / iterations as f64;
        let parallel_avg = parallel_time.as_secs_f64() * 1000.0 / iterations as f64;
        
        println!("   ### 验证性能对比:");
        println!("   串行验证: {:.2}ms", serial_avg);
        println!("   并行验证: {:.2}ms", parallel_avg);
        if serial_avg > 0.0 {
            println!("   并行加速比: {:.2}x", serial_avg / parallel_avg);
        }
        
        // 处理统计
        let stats = zero_copy_container.get_processing_stats();
        println!("   ### 处理统计:");
        println!("   总块数: {}, 小块: {}, 大块: {}", 
                 stats.total_chunks, stats.small_chunks, stats.large_chunks);
        println!("   平均压缩比: {:.2}", stats.average_compression_ratio);
        println!("   总原始数据: {:.2} MB", stats.total_raw_size as f64 / 1024.0 / 1024.0);
        
        println!();
    }
    
    // 总结优化效果
    println!("## 优化总结");
    println!("1. 零拷贝优化显著减少内存复制开销");
    println!("2. 并行验证利用多核提升处理速度");  
    println!("3. 流式哈希验证减少内存峰值");
    println!("4. 适应性任务批处理优化小块处理效率");
}