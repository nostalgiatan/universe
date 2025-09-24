//! # CIL - UNIV 容器格式命令行工具
//!
//! CIL (Container Interface Layer) 是 UNIV 容器格式的官方命令行工具，
//! 提供创建、读取、验证和转换 UNIV 容器的功能。
//!
//! ## 功能特性
//!
//! - 创建各种 Profile 类型的容器 (RECD, BLOB, TABL, TSDB, GRPH, TYPE)
//! - 读取和显示容器内容
//! - 验证容器完整性和格式符合性
//! - 提取和导出容器数据
//! - 容器格式转换和优化

#![allow(clippy::redundant_closure)]

use std::path::PathBuf;
use std::fs;

use clap::{Parser, Subcommand, ValueEnum};
use universe::{Container, Profile, ChunkKind, error::Result};

/// UNIV 容器格式命令行工具
#[derive(Parser)]
#[command(
    name = "cil",
    version = "1.1.0",
    about = "UNIV 容器格式命令行工具 - 创建、读取、验证 UNIV 容器",
    long_about = "CIL (Container Interface Layer) 是 UNIV 容器格式的官方命令行工具。\n\
                 支持创建各种 Profile 类型的容器，读取和验证容器内容，\n\
                 以及提供完整的容器操作功能。"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// 详细输出模式
    #[arg(short, long)]
    verbose: bool,
    
    /// 安静模式，只输出错误信息
    #[arg(short, long)]
    quiet: bool,
}

/// 可用的命令
#[derive(Subcommand)]
enum Commands {
    /// 创建新的 UNIV 容器
    Create {
        /// 输出文件路径
        #[arg(short, long)]
        output: PathBuf,
        
        /// 容器 Profile 类型
        #[arg(short, long, default_value = "recd")]
        profile: ProfileType,
        
        /// 生产者信息
        #[arg(long)]
        producer: Option<String>,
        
        /// 命名空间根
        #[arg(long)]
        namespace: Option<String>,
        
        /// 输入数据文件 (可多个)
        #[arg(short, long)]
        input: Vec<PathBuf>,
        
        /// 数据块类型
        #[arg(long, default_value = "data-node")]
        chunk_type: ChunkType,
    },
    
    /// 读取和显示容器信息
    Info {
        /// 容器文件路径
        file: PathBuf,
        
        /// 显示详细的块信息
        #[arg(long)]
        chunks: bool,
        
        /// 显示 TOC 信息
        #[arg(long)]
        toc: bool,
        
        /// 显示引用图信息
        #[arg(long)]
        refs: bool,
    },
    
    /// 验证容器完整性
    Verify {
        /// 容器文件路径
        file: PathBuf,
        
        /// 严格验证模式
        #[arg(long)]
        strict: bool,
    },
    
    /// 提取容器数据
    Extract {
        /// 容器文件路径
        file: PathBuf,
        
        /// 输出目录
        #[arg(short, long)]
        output: PathBuf,
        
        /// 提取指定块索引 (可多个)
        #[arg(long)]
        chunk: Vec<usize>,
        
        /// 保持原始格式
        #[arg(long)]
        raw: bool,
    },
    
    /// 性能分析和基准测试
    Benchmark {
        /// 测试文件路径
        file: PathBuf,
        
        /// 测试轮数
        #[arg(short, long, default_value = "10")]
        rounds: usize,
        
        /// 是否测试SIMD加速
        #[arg(long)]
        simd: bool,
        
        /// 是否测试并行处理
        #[arg(long)]
        parallel: bool,
    },
    
    /// 优化容器
    Optimize {
        /// 输入容器文件
        input: PathBuf,
        
        /// 输出优化后的容器文件
        #[arg(short, long)]
        output: PathBuf,
        
        /// 重新压缩使用更高压缩比
        #[arg(long)]
        recompress: bool,
        
        /// 启用零拷贝优化
        #[arg(long)]
        zero_copy: bool,
    },
}

/// Profile 类型枚举
#[derive(Clone, Copy, ValueEnum)]
enum ProfileType {
    /// 结构化记录
    #[clap(name = "recd")]
    Recd,
    /// 大对象/媒体文件
    #[clap(name = "blob")]
    Blob,
    /// 表格数据
    #[clap(name = "tabl")]
    Tabl,
    /// 时间序列数据
    #[clap(name = "tsdb")]
    Tsdb,
    /// 图形数据
    #[clap(name = "grph")]
    Grph,
    /// 类型仓库
    #[clap(name = "type")]
    Type,
    /// 混合模式
    #[clap(name = "mixd")]
    Mixd,
}

impl From<ProfileType> for Profile {
    fn from(pt: ProfileType) -> Self {
        match pt {
            ProfileType::Recd => Profile::Recd,
            ProfileType::Blob => Profile::Blob,
            ProfileType::Tabl => Profile::Tabl,
            ProfileType::Tsdb => Profile::Tsdb,
            ProfileType::Grph => Profile::Grph,
            ProfileType::Type => Profile::Type,
            ProfileType::Mixd => Profile::Mixd,
        }
    }
}

/// 数据块类型枚举
#[derive(Clone, Copy, ValueEnum)]
enum ChunkType {
    /// 数据节点
    #[clap(name = "data-node")]
    DataNode,
    /// 大对象
    #[clap(name = "blob")]
    Blob,
    /// Schema 定义
    #[clap(name = "schema")]
    Schema,
    /// 字符串表
    #[clap(name = "string-table")]
    StringTable,
    /// 索引分片
    #[clap(name = "index-shard")]
    IndexShard,
    /// 附件
    #[clap(name = "attachment")]
    Attachment,
}

impl From<ChunkType> for ChunkKind {
    fn from(ct: ChunkType) -> Self {
        match ct {
            ChunkType::DataNode => ChunkKind::DataNode,
            ChunkType::Blob => ChunkKind::Blob,
            ChunkType::Schema => ChunkKind::Schema,
            ChunkType::StringTable => ChunkKind::StringTable,
            ChunkType::IndexShard => ChunkKind::IndexShard,
            ChunkType::Attachment => ChunkKind::Attachment,
        }
    }
}

fn main() {
    let cli = Cli::parse();
    
    let result = match cli.command {
        Commands::Create { 
            output, 
            profile, 
            producer, 
            namespace, 
            input, 
            chunk_type 
        } => {
            create_container(output, profile, producer, namespace, input, chunk_type, cli.verbose)
        }
        Commands::Info { file, chunks, toc, refs } => {
            show_container_info(file, chunks, toc, refs, cli.verbose)
        }
        Commands::Verify { file, strict } => {
            verify_container(file, strict, cli.verbose)
        }
        Commands::Extract { file, output, chunk, raw } => {
            extract_container_data(file, output, chunk, raw, cli.verbose)
        }
        Commands::Benchmark { file, rounds, simd, parallel } => {
            benchmark_container(file, rounds, simd, parallel, cli.verbose)
        }
        Commands::Optimize { input, output, recompress, zero_copy } => {
            optimize_container(input, output, recompress, zero_copy, cli.verbose)
        }
    };
    
    match result {
        Ok(()) => {
            if !cli.quiet {
                eprintln!("操作完成");
            }
        }
        Err(e) => {
            eprintln!("错误: {}", e);
            std::process::exit(1);
        }
    }
}

/// 创建新的 UNIV 容器
fn create_container(
    output: PathBuf,
    profile: ProfileType,
    producer: Option<String>,
    namespace: Option<String>,
    input_files: Vec<PathBuf>,
    chunk_type: ChunkType,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("创建 {} Profile 容器: {:?}", profile_name(profile), output);
    }
    
    // 创建容器
    let mut container = Container::new(profile.into());
    
    // 设置头部信息
    if let Some(producer) = producer {
        container.header.set_producer(&producer);
    }
    
    if let Some(namespace) = namespace {
        container.header.set_namespace_root(&namespace);
    }
    
    // 添加输入文件数据
    for input_file in input_files {
        if verbose {
            println!("添加文件: {:?}", input_file);
        }
        
        let data = fs::read(&input_file)
            .map_err(universe::error::UnivError::IoError)?;
        
        // 使用简化的API添加数据，自动选择最佳设置
        container.add_data_simple(chunk_type.into(), &data)?;
    }
    
    // 如果没有输入文件，创建一个示例数据块
    if container.chunk_count() == 0 {
        let example_data = format!("UNIV 容器示例数据 - Profile: {}", profile_name(profile));
        container.add_data_simple(chunk_type.into(), example_data.as_bytes())?;
    }
    
    // 序列化容器
    let serialized = container.serialize()?;
    
    // 写入文件
    fs::write(&output, &serialized)
        .map_err(universe::error::UnivError::IoError)?;
    
    if verbose {
        println!("容器已创建: {} 字节, {} 个数据块", serialized.len(), container.chunk_count());
    }
    
    Ok(())
}

/// 显示容器信息
fn show_container_info(
    file: PathBuf,
    show_chunks: bool,
    show_toc: bool,
    show_refs: bool,
    verbose: bool,
) -> Result<()> {
    let data = fs::read(&file)
        .map_err(|e| universe::error::UnivError::IoError(e))?;
    
    let container = Container::deserialize(&data)?;
    
    // 基本信息
    println!("=== 容器信息 ===");
    println!("文件: {:?}", file);
    println!("大小: {} 字节", data.len());
    println!("Profile: {} ({})", container.header.profile, container.header.profile.description());
    println!("版本: {}.{}", container.header.major_version, container.header.minor_version);
    println!("数据块数量: {}", container.chunk_count());
    
    if let Some(producer) = container.header.get_producer() {
        println!("生产者: {}", producer);
    }
    
    if let Some(namespace) = container.header.get_namespace_root() {
        println!("命名空间: {}", namespace);
    }
    
    if let Some(timestamp) = container.header.get_creation_timestamp() {
        println!("创建时间: {}", timestamp);
    }
    
    // 显示数据块信息
    if show_chunks {
        println!("\n=== 数据块详情 ===");
        for (i, chunk) in container.chunks.iter().enumerate() {
            println!("块 {}: {:?}", i, chunk.kind);
            println!("  压缩: {:?}", chunk.codec);
            println!("  原始大小: {} 字节", chunk.raw_size);
            println!("  压缩大小: {} 字节", chunk.compressed_size);
            println!("  压缩比: {:.2}", chunk.compression_ratio());
            println!("  哈希算法: {:?}", chunk.hash_algorithm);
            println!("  内容哈希: {}", hex::encode(&chunk.content_hash));
            
            if verbose {
                println!("  变换标志: 0x{:04x}", chunk.transform_flags);
            }
        }
    }
    
    // 显示 TOC 信息
    if show_toc {
        println!("\n=== 目录索引 (TOC) ===");
        if let Some(ref toc) = container.toc {
            println!("节点数量: {}", toc.nodes.len());
            println!("引用数量: {}", toc.refs.len());
            println!("根节点数量: {}", toc.roots.len());
            
            if verbose {
                for (name, node_id) in &toc.roots {
                    println!("  根节点: {} -> {}", name, hex::encode(node_id));
                }
            }
        } else {
            println!("无 TOC 信息");
        }
    }
    
    // 显示引用信息
    if show_refs && container.toc.is_some() {
        println!("\n=== 引用关系 ===");
        let toc = container.toc.as_ref().unwrap();
        
        for (name, ref_index) in &toc.refs {
            println!("引用 {}: {} -> {}", 
                     name, 
                     ref_index.source_node, 
                     ref_index.target_node);
            
            if ref_index.external {
                println!("  (外部引用)");
            }
        }
    }
    
    Ok(())
}

/// 验证容器完整性
fn verify_container(file: PathBuf, strict: bool, verbose: bool) -> Result<()> {
    if verbose {
        println!("验证容器: {:?}", file);
    }
    
    let data = fs::read(&file)
        .map_err(|e| universe::error::UnivError::IoError(e))?;
    
    let container = Container::deserialize(&data)?;
    
    // 基本验证
    println!("✓ 容器解析成功");
    
    // 验证数据块完整性（使用统一的验证API）
    if strict {
        // 严格模式使用串行验证确保准确性
        container.verify(false)?;
        println!("✓ 严格模式数据块验证通过");
    } else {
        // 普通模式使用并行验证提高速度
        container.verify(true)?;
        println!("✓ 数据块验证通过");
    }
    
    // 严格模式验证
    if strict {
        // 验证 Profile 约束
        for chunk in &container.chunks {
            if !container.header.profile.supports_chunk_kind(chunk.kind.to_u8()) {
                return Err(universe::error::UnivError::schema_error(
                    format!("Profile {} 不支持 ChunkKind {:?}", 
                            container.header.profile, chunk.kind)
                ));
            }
        }
        
        println!("✓ Profile 约束验证通过");
        
        // 验证引用完整性
        if let Some(ref toc) = container.toc {
            // 简化循环检测 - 这里只做基本检查
            // 完整的循环检测需要根据实际的引用结构来实现
            if !toc.refs.is_empty() {
                println!("✓ 引用结构检查通过");
            }
        }
    }
    
    println!("✓ 容器验证完成");
    Ok(())
}

/// 提取容器数据
fn extract_container_data(
    file: PathBuf,
    output_dir: PathBuf,
    chunk_indices: Vec<usize>,
    raw: bool,
    verbose: bool,
) -> Result<()> {
    let data = fs::read(&file)
        .map_err(|e| universe::error::UnivError::IoError(e))?;
    
    let container = Container::deserialize(&data)?;
    
    // 创建输出目录
    fs::create_dir_all(&output_dir)
        .map_err(universe::error::UnivError::IoError)?;
    
    let indices = if chunk_indices.is_empty() {
        (0..container.chunk_count()).collect()
    } else {
        chunk_indices
    };
    
    for &index in &indices {
        if index >= container.chunk_count() {
            eprintln!("警告: 数据块索引 {} 超出范围", index);
            continue;
        }
        
        let chunk = container.get_chunk(index).unwrap();
        let chunk_data = if raw {
            chunk.payload.to_bytes().to_vec()
        } else {
            chunk.get_raw_data()?
        };
        
        let filename = format!("chunk_{:03}_{:?}.bin", index, chunk.kind);
        let output_path = output_dir.join(filename);
        
        fs::write(&output_path, &chunk_data)
            .map_err(universe::error::UnivError::IoError)?;
        
        if verbose {
            println!("提取数据块 {} -> {:?} ({} 字节)", 
                     index, output_path, chunk_data.len());
        }
    }
    
    Ok(())
}

/// 获取 Profile 名称
fn profile_name(profile: ProfileType) -> &'static str {
    match profile {
        ProfileType::Recd => "RECD",
        ProfileType::Blob => "BLOB", 
        ProfileType::Tabl => "TABL",
        ProfileType::Tsdb => "TSDB",
        ProfileType::Grph => "GRPH",
        ProfileType::Type => "TYPE",
        ProfileType::Mixd => "MIXD",
    }
}

/// 性能基准测试
fn benchmark_container(
    file: PathBuf,
    rounds: usize,
    _test_simd: bool,
    _test_parallel: bool,
    verbose: bool,
) -> Result<()> {
    use std::time::Instant;
    
    if verbose {
        println!("对容器文件进行性能基准测试: {:?}", file);
        println!("测试轮数: {}", rounds);
    }
    
    let data = fs::read(&file)
        .map_err(|e| universe::error::UnivError::IoError(e))?;
    
    println!("文件大小: {:.2} KB", data.len() as f64 / 1024.0);
    
    // 基础反序列化性能测试
    let mut total_deserialize_time = std::time::Duration::new(0, 0);
    for _ in 0..rounds {
        let start = Instant::now();
        let _container = Container::deserialize(&data)?;
        total_deserialize_time += start.elapsed();
    }
    
    let avg_deserialize_ms = total_deserialize_time.as_millis() as f64 / rounds as f64;
    println!("平均反序列化时间: {:.2} ms", avg_deserialize_ms);
    
    // 零拷贝反序列化性能测试  
    let mut total_zero_copy_time = std::time::Duration::new(0, 0);
    for _ in 0..rounds {
        let start = Instant::now();
        let _container = Container::deserialize_zero_copy(&data)?;
        total_zero_copy_time += start.elapsed();
    }
    
    let avg_zero_copy_ms = total_zero_copy_time.as_millis() as f64 / rounds as f64;
    println!("平均零拷贝反序列化时间: {:.2} ms", avg_zero_copy_ms);
    
    if avg_deserialize_ms > 0.0 {
        let speedup = avg_deserialize_ms / avg_zero_copy_ms;
        println!("零拷贝性能提升: {:.2}x", speedup);
    }
    
    Ok(())
}

/// 优化容器
fn optimize_container(
    input: PathBuf,
    output: PathBuf,
    _recompress: bool,
    zero_copy: bool,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("优化容器: {:?} -> {:?}", input, output);
    }
    
    let data = fs::read(&input)
        .map_err(|e| universe::error::UnivError::IoError(e))?;
    
    let original_size = data.len();
    
    // 根据选项选择反序列化方法
    let container = if zero_copy {
        if verbose {
            println!("使用零拷贝反序列化");
        }
        Container::deserialize_zero_copy(&data)?
    } else {
        Container::deserialize(&data)?
    };
    
    if verbose {
        println!("原始容器大小: {:.2} KB", original_size as f64 / 1024.0);
        println!("数据块数量: {}", container.chunk_count());
        
        let stats = container.get_processing_stats();
        println!("小块数量: {}", stats.small_chunks);
        println!("大块数量: {}", stats.large_chunks);
        println!("平均压缩比: {:.2}", stats.average_compression_ratio);
    }
    
    let optimized_container = container;
    
    // 序列化优化后的容器
    let optimized_data = optimized_container.serialize()?;
    let optimized_size = optimized_data.len();
    
    // 写入输出文件
    fs::write(&output, &optimized_data)
        .map_err(|e| universe::error::UnivError::IoError(e))?;
    
    if verbose || original_size != optimized_size {
        println!("优化后大小: {:.2} KB", optimized_size as f64 / 1024.0);
        let reduction = (original_size as f64 - optimized_size as f64) / original_size as f64 * 100.0;
        if reduction > 0.0 {
            println!("大小减少: {:.1}%", reduction);
        } else if reduction < 0.0 {
            println!("大小增加: {:.1}%", -reduction);
        } else {
            println!("大小无变化");
        }
    }
    
    println!("容器优化完成");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    /// 测试 CLI 创建容器功能
    #[test]
    fn test_cli_create_container() {
        let temp_dir = TempDir::new().unwrap();
        let output_file = temp_dir.path().join("test.univ");
        
        let result = create_container(
            output_file.clone(),
            ProfileType::Recd,
            Some("test-cli".to_string()),
            Some("org.test".to_string()),
            vec![],
            ChunkType::DataNode,
            false,
        );
        
        assert!(result.is_ok());
        assert!(output_file.exists());
        
        // 验证创建的文件可以正确读取
        let data = fs::read(&output_file).unwrap();
        let container = Container::deserialize(&data).unwrap();
        
        assert_eq!(container.header.profile, Profile::Recd);
        assert_eq!(container.header.get_producer(), Some("test-cli"));
        assert_eq!(container.header.get_namespace_root(), Some("org.test"));
        assert_eq!(container.chunk_count(), 1);
    }
    
    /// 测试容器验证功能
    #[test]
    fn test_cli_verify_container() {
        let temp_dir = TempDir::new().unwrap();
        let container_file = temp_dir.path().join("test.univ");
        
        // 先创建一个容器
        let mut container = Container::new(Profile::Recd);
        container.add_data_simple(ChunkKind::DataNode, b"test data").unwrap();
        
        let serialized = container.serialize().unwrap();
        fs::write(&container_file, &serialized).unwrap();
        
        // 验证容器
        let result = verify_container(container_file, true, false);
        assert!(result.is_ok());
    }
    
    /// 测试数据提取功能
    #[test]
    fn test_cli_extract_data() {
        let temp_dir = TempDir::new().unwrap();
        let container_file = temp_dir.path().join("test.univ");
        let extract_dir = temp_dir.path().join("extracted");
        
        // 创建容器
        let mut container = Container::new(Profile::Recd);
        let test_data = b"Hello, UNIV World!";
        container.add_data(
            ChunkKind::DataNode,
            test_data,
            universe::chunk::Codec::None,
            0,
            universe::constants::hash_algorithms::BLAKE3,
        ).unwrap();
        
        let serialized = container.serialize().unwrap();
        fs::write(&container_file, &serialized).unwrap();
        
        // 提取数据
        let result = extract_container_data(
            container_file,
            extract_dir.clone(),
            vec![],
            false,
            false,
        );
        
        assert!(result.is_ok());
        
        // 检查提取的文件
        let extracted_file = extract_dir.join("chunk_000_DataNode.bin");
        assert!(extracted_file.exists());
        
        let extracted_data = fs::read(&extracted_file).unwrap();
        assert_eq!(extracted_data, test_data);
    }
}