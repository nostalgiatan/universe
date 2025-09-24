//! # 性能基准测试
//!
//! 用于验证性能优化效果的基准测试套件

use std::time::Instant;
use universe::{Container, Profile, chunk::{ChunkKind, Codec}, constants::hash_algorithms};

/// 创建测试数据
fn create_test_data(size: usize) -> Vec<u8> {
    // 生成具有一定模式的测试数据，利于压缩测试
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

/// 基准测试：反序列化性能
fn benchmark_deserialize(data: &[u8], iterations: u32) -> (u128, usize) {
    let start = Instant::now();
    let mut total_chunks = 0;
    
    for _ in 0..iterations {
        let container = Container::deserialize(data).unwrap();
        total_chunks += container.chunk_count();
    }
    
    let elapsed = start.elapsed().as_millis();
    (elapsed, total_chunks / iterations as usize)
}

/// 基准测试：串行验证性能 
fn benchmark_verify_serial(data: &[u8], iterations: u32) -> (u128, usize) {
    let container = Container::deserialize(data).unwrap();
    let start = Instant::now();
    let mut total_verified = 0;
    
    for _ in 0..iterations {
        for chunk in &container.chunks {
            chunk.verify().unwrap();
            total_verified += 1;
        }
    }
    
    let elapsed = start.elapsed().as_millis();
    (elapsed, total_verified / iterations as usize)
}

/// 基准测试：完整处理（反序列化 + 验证）
fn benchmark_full_processing(data: &[u8], iterations: u32) -> (u128, usize, usize) {
    let start = Instant::now();
    let mut total_chunks = 0;
    let mut total_raw_bytes = 0;
    
    for _ in 0..iterations {
        let container = Container::deserialize(data).unwrap();
        
        for chunk in &container.chunks {
            chunk.verify().unwrap();
            total_raw_bytes += chunk.raw_size as usize;
            total_chunks += 1;
        }
    }
    
    let elapsed = start.elapsed().as_millis();
    (elapsed, total_chunks / iterations as usize, total_raw_bytes / iterations as usize)
}

fn main() {
    println!("# UNIV 容器性能基准测试\n");
    
    // 测试不同大小的容器
    let test_cases = vec![
        ("300KB", 10, 30_000, Codec::Zstd),
        ("1MB", 20, 50_000, Codec::Zstd),
        ("5MB", 50, 100_000, Codec::Zstd),
    ];
    
    for (size_name, chunk_count, chunk_size, codec) in test_cases {
        println!("## 测试用例: {} ({} 块, 每块 ~{}字节, {:?}压缩)", 
                 size_name, chunk_count, chunk_size, codec);
        
        // 创建并序列化测试容器
        let container = create_test_container(chunk_count, chunk_size, codec);
        let serialized = container.serialize().unwrap();
        let file_size = serialized.len();
        
        println!("   序列化后大小: {} 字节", file_size);
        
        // 基准测试：反序列化
        let iterations = if file_size < 1_000_000 { 10 } else { 5 };
        let (deserialize_time, chunks) = benchmark_deserialize(&serialized, iterations);
        println!("   反序列化性能: {}ms ({} 块, {:.2}ms/块)", 
                 deserialize_time / iterations as u128, chunks, 
                 deserialize_time as f64 / (iterations * chunks as u32) as f64);
        
        // 基准测试：串行验证
        let (verify_time, verified_chunks) = benchmark_verify_serial(&serialized, iterations);
        println!("   串行验证性能: {}ms ({} 块, {:.2}ms/块)", 
                 verify_time / iterations as u128, verified_chunks, 
                 verify_time as f64 / (iterations * verified_chunks as u32) as f64);
        
        // 基准测试：完整处理
        let (full_time, processed_chunks, raw_bytes) = benchmark_full_processing(&serialized, iterations);
        let throughput = (raw_bytes as f64 * iterations as f64 * 1000.0) / (full_time as f64 * 1024.0 * 1024.0);
        println!("   完整处理性能: {}ms ({} 块, {} 原始字节, {:.2} MB/s)", 
                 full_time / iterations as u128, processed_chunks, raw_bytes, throughput);
        
        // 内存使用分析
        let container = Container::deserialize(&serialized).unwrap();
        let mut compressed_total = 0;
        let mut raw_total = 0;
        
        for chunk in &container.chunks {
            compressed_total += chunk.compressed_size as usize;
            raw_total += chunk.raw_size as usize;
        }
        
        let memory_overhead = (file_size + compressed_total) as f64 / file_size as f64;
        println!("   内存放大倍数: {:.2}x (文件: {}, 压缩数据复制: {}, 原始: {})", 
                 memory_overhead, file_size, compressed_total, raw_total);
        
        println!();
    }
}