//! # 性能优化验证基准测试
//!
//! 对比优化前后的性能差异，验证优化效果

use std::time::Instant;
use universe::{Container, Profile, chunk::{ChunkKind, Codec}, constants::hash_algorithms};

/// 创建测试数据
fn create_test_data(size: usize) -> Vec<u8> {
    let pattern = "这是一个测试数据块，包含中文内容，用于演示压缩效果。".as_bytes();
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
    println!("# UNIV 容器性能优化验证\n");
    
    // 测试不同大小的容器
    let test_cases = vec![
        ("1MB", 20, 50_000, Codec::Zstd),
        ("5MB", 50, 100_000, Codec::Zstd),
        ("10MB", 100, 100_000, Codec::Zstd),
    ];
    
    for (size_name, chunk_count, chunk_size, codec) in test_cases {
        println!("## 测试用例: {} ({} 块, 每块 ~{}字节, {:?}压缩)", 
                 size_name, chunk_count, chunk_size, codec);
        
        // 创建并序列化测试容器
        let container = create_test_container(chunk_count, chunk_size, codec);
        let serialized = container.serialize().unwrap();
        let file_size = serialized.len();
        
        println!("   序列化后大小: {} 字节", file_size);
        
        // 测试传统反序列化 vs 零拷贝反序列化
        let iterations = 5;
        
        // 传统反序列化
        let start = Instant::now();
        for _ in 0..iterations {
            let _container = Container::deserialize(&serialized).unwrap();
        }
        let traditional_time = start.elapsed().as_millis();
        
        // 零拷贝反序列化
        let start = Instant::now();
        for _ in 0..iterations {
            let _container = Container::deserialize_zero_copy(&serialized).unwrap();
        }
        let zero_copy_time = start.elapsed().as_millis();
        
        println!("   传统反序列化: {}ms", traditional_time / iterations as u128);
        println!("   零拷贝反序列化: {}ms", zero_copy_time / iterations as u128);
        if traditional_time > 0 {
            let speedup = traditional_time as f64 / zero_copy_time as f64;
            println!("   零拷贝加速比: {:.2}x", speedup);
        }
        
        // 内存使用对比
        let traditional_container = Container::deserialize(&serialized).unwrap();
        let zero_copy_container = Container::deserialize_zero_copy(&serialized).unwrap();
        
        // 估算内存使用（简化计算）
        let traditional_stats = traditional_container.get_processing_stats();
        let zero_copy_stats = zero_copy_container.get_processing_stats();
        
        println!("   传统方式内存放大: {:.2}x", 
                 (file_size + traditional_stats.total_compressed_size as usize) as f64 / file_size as f64);
        println!("   零拷贝内存放大: {:.2}x", 
                 (file_size + zero_copy_stats.total_compressed_size as usize) as f64 / file_size as f64);
        
        // 验证性能对比：串行 vs 并行
        let start = Instant::now();
        for _ in 0..iterations {
            zero_copy_container.verify_serial().unwrap();
        }
        let serial_verify_time = start.elapsed().as_millis();
        
        let start = Instant::now();
        for _ in 0..iterations {
            zero_copy_container.verify_parallel().unwrap();
        }
        let parallel_verify_time = start.elapsed().as_millis();
        
        println!("   串行验证: {}ms", serial_verify_time / iterations as u128);
        println!("   并行验证: {}ms", parallel_verify_time / iterations as u128);
        if serial_verify_time > 0 {
            let speedup = serial_verify_time as f64 / parallel_verify_time as f64;
            println!("   并行验证加速比: {:.2}x", speedup);
        }
        
        // 处理统计信息
        let stats = zero_copy_container.get_processing_stats();
        println!("   处理统计: {} 总块数 ({} 小块, {} 大块), 平均压缩比: {:.2}", 
                 stats.total_chunks, stats.small_chunks, stats.large_chunks, 
                 stats.average_compression_ratio);
        
        println!();
    }
}