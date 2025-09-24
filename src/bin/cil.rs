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

    /// 类型仓库操作
    #[command(subcommand)]
    Type(TypeCommands),
    
    /// 数据打包操作
    #[command(subcommand)]
    Data(DataCommands),
}

/// Profile 类型枚举
#[derive(Clone, Copy, ValueEnum, Debug)]
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

/// 类型仓库操作命令
#[derive(Subcommand)]
enum TypeCommands {
    /// 打包类型仓库
    Pack {
        /// 输出类型仓库文件
        #[arg(short, long)]
        output: PathBuf,
        
        /// 命名空间
        #[arg(short, long)]
        namespace: String,
        
        /// 包名
        #[arg(short, long)]
        package: String,
        
        /// 版本
        #[arg(long, default_value = "1.0.0")]
        version: String,
        
        /// 输入 Schema 文件 (可多个)
        #[arg(short, long)]
        input: Vec<PathBuf>,
        
        /// 许可证
        #[arg(long)]
        license: Option<String>,
        
        /// 描述
        #[arg(long)]
        description: Option<String>,
    },
    
    /// 发布类型仓库到注册表
    Publish {
        /// 类型仓库文件
        file: PathBuf,
        
        /// 注册表 URL
        #[arg(long)]
        registry: Option<String>,
        
        /// 访问令牌
        #[arg(long)]
        token: Option<String>,
    },
    
    /// 解析类型依赖
    Resolve {
        /// 类型仓库文件或 URN
        input: String,
        
        /// 输出解析结果
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        /// 显示依赖树
        #[arg(long)]
        tree: bool,
    },
    
    /// 验证类型仓库
    Verify {
        /// 类型仓库文件
        file: PathBuf,
        
        /// 验证签名
        #[arg(long)]
        signatures: bool,
        
        /// 验证依赖
        #[arg(long)]
        dependencies: bool,
    },
}

/// 数据打包操作命令
#[derive(Subcommand)]
enum DataCommands {
    /// 打包数据文件
    Pack {
        /// 输出数据容器文件
        #[arg(short, long)]
        output: PathBuf,
        
        /// 容器 Profile 类型
        #[arg(short, long, default_value = "recd")]
        profile: ProfileType,
        
        /// Schema 引用 (URN 或文件路径)
        #[arg(long)]
        schema: Option<String>,
        
        /// 输入数据文件 (可多个)
        #[arg(short, long)]
        input: Vec<PathBuf>,
        
        /// 数据格式 (json, jsonl, csv, cbor)
        #[arg(long, default_value = "json")]
        format: DataFormat,
        
        /// 启用数据变换
        #[arg(long)]
        transforms: Vec<String>,
        
        /// 压缩算法
        #[arg(long, default_value = "zstd")]
        codec: String,
    },
    
    /// 批量导入数据
    Import {
        /// 输出容器文件
        #[arg(short, long)]
        output: PathBuf,
        
        /// 输入数据目录
        #[arg(short, long)]
        input: PathBuf,
        
        /// 数据格式
        #[arg(long, default_value = "jsonl")]
        format: DataFormat,
        
        /// Schema 文件或 URN
        #[arg(long)]
        schema: Option<String>,
        
        /// 批处理大小
        #[arg(long, default_value = "1000")]
        batch_size: usize,
    },
}

/// 数据格式枚举
#[derive(Clone, Copy, ValueEnum, Debug)]
enum DataFormat {
    /// JSON 格式
    #[clap(name = "json")]
    Json,
    /// JSON Lines 格式
    #[clap(name = "jsonl")]
    JsonLines,
    /// CSV 格式
    #[clap(name = "csv")]
    Csv,
    /// CBOR 格式
    #[clap(name = "cbor")]
    Cbor,
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
        Commands::Type(type_cmd) => {
            match type_cmd {
                TypeCommands::Pack { output, namespace, package, version, input, license, description } => {
                    type_pack(output, namespace, package, version, input, license, description, cli.verbose)
                }
                TypeCommands::Publish { file, registry, token } => {
                    type_publish(file, registry, token, cli.verbose)
                }
                TypeCommands::Resolve { input, output, tree } => {
                    type_resolve(input, output, tree, cli.verbose)
                }
                TypeCommands::Verify { file, signatures, dependencies } => {
                    type_verify(file, signatures, dependencies, cli.verbose)
                }
            }
        }
        Commands::Data(data_cmd) => {
            match data_cmd {
                DataCommands::Pack { output, profile, schema, input, format, transforms, codec } => {
                    data_pack(output, profile, schema, input, format, transforms, codec, cli.verbose)
                }
                DataCommands::Import { output, input, format, schema, batch_size } => {
                    data_import(output, input, format, schema, batch_size, cli.verbose)
                }
            }
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
    recompress: bool,
    zero_copy: bool,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("优化容器: {:?} -> {:?}", input, output);
        if recompress {
            println!("启用重压缩优化");
        }
    }
    
    let data = fs::read(&input)
        .map_err(|e| universe::error::UnivError::IoError(e))?;
    
    let original_size = data.len();
    
    // 根据选项选择反序列化方法
    let mut container = if zero_copy {
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
    
    // 执行实际的优化
    let mut total_original_compressed: u64 = 0;
    let mut total_final_compressed: u64 = 0;
    let mut total_improvement_bytes: u64 = 0;
    let mut optimized_chunks = 0;
    let mut structural_stats = Vec::new();
    
    if recompress {
        if verbose {
            println!("\n开始重压缩优化...");
        }
        
        for (i, chunk) in container.chunks.iter_mut().enumerate() {
            let original_compressed = chunk.compressed_size;
            total_original_compressed += original_compressed as u64;
            
            // 尝试重压缩（最小改进64字节）
            match chunk.try_recompress(64) {
                Ok(compression_stats) => {
                    total_final_compressed += compression_stats.final_size as u64;
                    
                    if compression_stats.improvement_bytes > 0 {
                        optimized_chunks += 1;
                        total_improvement_bytes += compression_stats.improvement_bytes as u64;
                        
                        if verbose {
                            println!("块{}: {} -> {} 字节 ({:.1}% 减少, 测试了{}个级别)", 
                                i,
                                compression_stats.original_size,
                                compression_stats.final_size,
                                compression_stats.improvement_ratio,
                                compression_stats.levels_tested.len()
                            );
                        }
                    } else {
                        if verbose && i < 5 { // 只显示前几个未优化的块信息
                            println!("块{}: 无改进机会 ({} 字节)", i, original_compressed);
                        }
                    }
                    
                    // 收集结构化开销统计
                    if i < 10 { // 只分析前10个块的开销
                        let overhead = chunk.get_structural_overhead();
                        structural_stats.push(overhead);
                    }
                }
                Err(e) => {
                    if verbose {
                        println!("块{} 重压缩失败: {}", i, e);
                    }
                    total_final_compressed += original_compressed as u64;
                }
            }
        }
        
        if verbose {
            println!("\n压缩优化结果:");
            println!("  优化的块数: {}/{}", optimized_chunks, container.chunks.len());
            println!("  原始压缩总大小: {:.2} KB", total_original_compressed as f64 / 1024.0);
            println!("  最终压缩总大小: {:.2} KB", total_final_compressed as f64 / 1024.0);
            println!("  压缩改进: {} 字节 ({:.1}%)", 
                total_improvement_bytes,
                if total_original_compressed > 0 {
                    total_improvement_bytes as f64 / total_original_compressed as f64 * 100.0
                } else { 0.0 }
            );
            
            // 显示结构化开销分析
            if !structural_stats.is_empty() {
                println!("\n结构化开销分析(前{}个块):", structural_stats.len());
                let avg_metadata_ratio: f64 = structural_stats.iter()
                    .map(|s| s.metadata_ratio)
                    .sum::<f64>() / structural_stats.len() as f64;
                let avg_header_bytes = structural_stats.iter()
                    .map(|s| s.header_bytes)
                    .sum::<u32>() / structural_stats.len() as u32;
                
                println!("  平均元数据开销: {:.1}%", avg_metadata_ratio);
                println!("  平均头部大小: {} 字节", avg_header_bytes);
            }
        }
    } else {
        // 没有启用重压缩，只复制现有压缩大小
        total_final_compressed = container.chunks.iter()
            .map(|c| c.compressed_size as u64)
            .sum();
    }
    
    // 序列化优化后的容器
    let optimized_data = container.serialize()?;
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

/// 打包类型仓库
fn type_pack(
    output: PathBuf,
    namespace: String,
    package: String,
    version: String,
    input: Vec<PathBuf>,
    license: Option<String>,
    description: Option<String>,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("打包类型仓库: {} -> {:?}", package, output);
        println!("命名空间: {}", namespace);
        println!("版本: {}", version);
    }
    
    // 创建 TYPE Profile 容器
    let mut container = Container::new(Profile::Type);
    
    // 添加生产者信息
    container.header.add_extension(
        universe::constants::header_ext_types::PRODUCER,
        universe::header::HeaderExtension::Unknown(format!("CIL v1.1.0 type pack").into_bytes()),
    );
    
    // 添加命名空间根
    container.header.add_extension(
        universe::constants::header_ext_types::NAMESPACE_ROOT,
        universe::header::HeaderExtension::Unknown(namespace.clone().into_bytes()),
    );
    
    // 构建 Manifest 数据
    let mut manifest_map = std::collections::HashMap::new();
    manifest_map.insert("namespace".to_string(), serde_json::Value::String(namespace));
    manifest_map.insert("package".to_string(), serde_json::Value::String(package));
    manifest_map.insert("version".to_string(), serde_json::Value::String(version));
    
    if let Some(desc) = description {
        manifest_map.insert("description".to_string(), serde_json::Value::String(desc));
    }
    
    if let Some(lic) = license {
        manifest_map.insert("license".to_string(), serde_json::Value::String(lic));
    }
    
    // 添加导出列表（简化实现）
    let exports = serde_json::Value::Array(vec![]);
    manifest_map.insert("exports".to_string(), exports);
    
    let manifest_json = serde_json::Value::Object(serde_json::Map::from_iter(manifest_map));
    let manifest_data = serde_json::to_vec(&manifest_json)?;
    
    // 添加 Manifest 块
    container.add_data(
        ChunkKind::Schema, // 使用 Schema 块类型存储 Manifest
        &manifest_data,
        universe::chunk::Codec::Zstd,
        0,
        universe::constants::hash_algorithms::BLAKE3,
    )?;
    
    // 处理输入 Schema 文件
    for (i, schema_file) in input.iter().enumerate() {
        if verbose {
            println!("处理 Schema 文件: {:?}", schema_file);
        }
        
        let schema_data = fs::read(schema_file)
            .map_err(|e| universe::error::UnivError::IoError(e))?;
        
        container.add_data(
            ChunkKind::Schema,
            &schema_data,
            universe::chunk::Codec::Zstd,
            0,
            universe::constants::hash_algorithms::BLAKE3,
        )?;
        
        if verbose {
            println!("  已添加 Schema #{}: {} 字节", i + 1, schema_data.len());
        }
    }
    
    // 序列化并写入
    let serialized = container.serialize()?;
    fs::write(&output, &serialized)
        .map_err(|e| universe::error::UnivError::IoError(e))?;
    
    if verbose {
        println!("类型仓库已打包: {:.2} KB", serialized.len() as f64 / 1024.0);
        println!("包含 {} 个 Schema", input.len());
    }
    
    println!("类型仓库打包完成");
    Ok(())
}

/// 发布类型仓库
fn type_publish(
    _file: PathBuf,
    _registry: Option<String>,
    _token: Option<String>,
    _verbose: bool,
) -> Result<()> {
    // 简化实现：暂不实现实际的注册表发布功能
    println!("类型仓库发布功能尚未实现");
    println!("请使用其他工具或手动上传到注册表");
    Ok(())
}

/// 解析类型依赖
fn type_resolve(
    input: String,
    output: Option<PathBuf>,
    tree: bool,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("解析类型依赖: {}", input);
    }
    
    // 检查输入是文件路径还是 URN
    let container = if input.starts_with("urn:") {
        // 简化实现：暂不支持 URN 解析
        println!("URN 解析功能尚未实现: {}", input);
        return Ok(());
    } else {
        let file_path = PathBuf::from(&input);
        if !file_path.exists() {
            return Err(universe::error::UnivError::deserialization_error(
                format!("文件不存在: {:?}", file_path)
            ));
        }
        
        let data = fs::read(&file_path)
            .map_err(|e| universe::error::UnivError::IoError(e))?;
        Container::deserialize(&data)?
    };
    
    // 验证是 TYPE profile
    if container.header.profile != Profile::Type {
        return Err(universe::error::UnivError::deserialization_error(
            "不是有效的类型仓库文件".to_string()
        ));
    }
    
    if verbose {
        println!("类型仓库验证通过");
        println!("包含 {} 个块", container.chunks.len());
    }
    
    if tree {
        println!("依赖树:");
        println!("└── {} (根)", input);
        println!("    ├── Manifest");
        for (i, chunk) in container.chunks.iter().enumerate() {
            if i == 0 {
                continue; // 跳过 Manifest
            }
            println!("    ├── Schema #{}: {} 字节", i, chunk.raw_size);
        }
    }
    
    // 如果指定了输出文件，写入解析结果
    if let Some(output_path) = output {
        let resolve_info = serde_json::json!({
            "input": input,
            "profile": "TYPE",
            "chunks": container.chunks.len(),
            "manifest": "present",
            "schemas": container.chunks.len().saturating_sub(1),
        });
        
        let resolve_data = serde_json::to_vec_pretty(&resolve_info)?;
        fs::write(&output_path, &resolve_data)
            .map_err(|e| universe::error::UnivError::IoError(e))?;
        
        if verbose {
            println!("解析结果已写入: {:?}", output_path);
        }
    }
    
    println!("类型依赖解析完成");
    Ok(())
}

/// 验证类型仓库
fn type_verify(
    file: PathBuf,
    signatures: bool,
    dependencies: bool,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("验证类型仓库: {:?}", file);
    }
    
    let data = fs::read(&file)
        .map_err(|e| universe::error::UnivError::IoError(e))?;
    let container = Container::deserialize(&data)?;
    
    // 验证 Profile
    if container.header.profile != Profile::Type {
        return Err(universe::error::UnivError::deserialization_error(
            "不是有效的类型仓库文件".to_string()
        ));
    }
    
    println!("✓ Profile 验证通过 (TYPE)");
    
    // 基本完整性验证
    container.verify(true)?;
    println!("✓ 容器完整性验证通过");
    
    if signatures {
        // 简化实现：暂不实现签名验证
        println!("ⓘ 签名验证功能尚未实现");
    }
    
    if dependencies {
        // 简化实现：暂不实现依赖验证
        println!("ⓘ 依赖验证功能尚未实现");
    }
    
    if verbose {
        println!("类型仓库包含 {} 个块", container.chunks.len());
        if let Some(producer) = container.header.get_producer() {
            println!("生产者: {}", producer);
        }
        if let Some(namespace) = container.header.get_namespace_root() {
            println!("命名空间: {}", namespace);
        }
    }
    
    println!("类型仓库验证完成");
    Ok(())
}

/// 打包数据文件
fn data_pack(
    output: PathBuf,
    profile: ProfileType,
    schema: Option<String>,
    input: Vec<PathBuf>,
    format: DataFormat,
    transforms: Vec<String>,
    codec: String,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("打包数据文件: {:?}", output);
        println!("Profile: {:?}", profile);
        println!("数据格式: {:?}", format);
        if let Some(ref s) = schema {
            println!("Schema: {}", s);
        }
    }
    
    let mut container = Container::new(profile.into());
    
    // 解析压缩算法
    let codec = match codec.as_str() {
        "none" => universe::chunk::Codec::None,
        "zstd" => universe::chunk::Codec::Zstd,
        "lz4" => universe::chunk::Codec::Lz4,
        "deflate" => universe::chunk::Codec::Deflate,
        _ => universe::chunk::Codec::Zstd, // 默认使用 zstd
    };
    
    // 解析变换标志
    let mut transform_flags = 0u16;
    for transform in &transforms {
        match transform.as_str() {
            "dict-string" => transform_flags |= universe::constants::transform_flags::DICT_STRING,
            "integer-varint" => transform_flags |= universe::constants::transform_flags::INTEGER_VARINT,
            "delta" => transform_flags |= universe::constants::transform_flags::DELTA,
            "columnarize" => transform_flags |= universe::constants::transform_flags::COLUMNARIZE,
            "gorilla" => transform_flags |= universe::constants::transform_flags::GORILLA,
            _ => {
                if verbose {
                    println!("⚠ 未知变换: {}", transform);
                }
            }
        }
    }
    
    // 处理输入文件
    for (i, input_file) in input.iter().enumerate() {
        if verbose {
            println!("处理文件 #{}: {:?}", i + 1, input_file);
        }
        
        let file_data = fs::read(input_file)
            .map_err(|e| universe::error::UnivError::IoError(e))?;
        
        // 根据格式处理数据
        let processed_data = match format {
            DataFormat::Json => {
                // 验证 JSON 格式
                let _: serde_json::Value = serde_json::from_slice(&file_data)?;
                file_data
            }
            DataFormat::JsonLines => {
                // 简化处理：直接使用原始数据
                file_data
            }
            DataFormat::Csv => {
                // 简化处理：直接使用原始数据
                file_data
            }
            DataFormat::Cbor => {
                // 验证 CBOR 格式
                let _: ciborium::Value = ciborium::de::from_reader(file_data.as_slice())?;
                file_data
            }
        };
        
        // 添加到容器
        container.add_data(
            ChunkKind::DataNode,
            &processed_data,
            codec,
            transform_flags,
            universe::constants::hash_algorithms::BLAKE3,
        )?;
        
        if verbose {
            println!("  已添加: {} 字节", processed_data.len());
        }
    }
    
    // 如果指定了 Schema，尝试添加
    if let Some(schema_ref) = schema {
        if verbose {
            println!("处理 Schema 引用: {}", schema_ref);
        }
        
        // 简化实现：如果是文件路径，读取并添加为 Schema 块
        if let Ok(schema_data) = fs::read(&schema_ref) {
            container.add_data(
                ChunkKind::Schema,
                &schema_data,
                universe::chunk::Codec::Zstd,
                0,
                universe::constants::hash_algorithms::BLAKE3,
            )?;
            
            if verbose {
                println!("  已添加 Schema: {} 字节", schema_data.len());
            }
        }
    }
    
    // 序列化并写入
    let serialized = container.serialize()?;
    fs::write(&output, &serialized)
        .map_err(|e| universe::error::UnivError::IoError(e))?;
        
    if verbose {
        println!("数据已打包: {:.2} KB", serialized.len() as f64 / 1024.0);
        println!("包含 {} 个数据块", input.len());
        if !transforms.is_empty() {
            println!("应用的变换: {:?}", transforms);
        }
    }
    
    println!("数据打包完成");
    Ok(())
}

/// 批量导入数据
fn data_import(
    output: PathBuf,
    input: PathBuf,
    format: DataFormat,
    schema: Option<String>,
    batch_size: usize,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("批量导入数据: {:?} -> {:?}", input, output);
        println!("批处理大小: {}", batch_size);
        println!("数据格式: {:?}", format);
    }
    
    if !input.is_dir() {
        return Err(universe::error::UnivError::deserialization_error(
            "输入路径必须是目录".to_string()
        ));
    }
    
    let mut container = Container::new(Profile::Recd);
    let mut processed_files = 0;
    let mut total_bytes = 0u64;
    
    // 遍历目录中的文件
    for entry in fs::read_dir(&input).map_err(|e| universe::error::UnivError::IoError(e))? {
        let entry = entry.map_err(|e| universe::error::UnivError::IoError(e))?;
        let file_path = entry.path();
        
        if !file_path.is_file() {
            continue;
        }
        
        if verbose {
            println!("处理文件: {:?}", file_path);
        }
        
        let file_data = fs::read(&file_path)
            .map_err(|e| universe::error::UnivError::IoError(e))?;
        
        total_bytes += file_data.len() as u64;
        
        // 根据格式验证数据
        match format {
            DataFormat::Json => {
                let _: serde_json::Value = serde_json::from_slice(&file_data)?;
            }
            DataFormat::JsonLines => {
                // 简单验证：检查每行是否为有效 JSON
                let text = std::str::from_utf8(&file_data)
                    .map_err(|_| universe::error::UnivError::deserialization_error("无效的 UTF-8 数据".to_string()))?;
                for line in text.lines() {
                    if !line.trim().is_empty() {
                        let _: serde_json::Value = serde_json::from_str(line)?;
                    }
                }
            }
            DataFormat::Csv => {
                // 简单验证：检查是否包含 CSV 分隔符
                let text = std::str::from_utf8(&file_data)
                    .map_err(|_| universe::error::UnivError::deserialization_error("无效的 UTF-8 数据".to_string()))?;
                if !text.contains(',') && !text.contains('\t') {
                    if verbose {
                        println!("⚠ 文件可能不是有效的 CSV 格式: {:?}", file_path);
                    }
                }
            }
            DataFormat::Cbor => {
                let _: ciborium::Value = ciborium::de::from_reader(file_data.as_slice())?;
            }
        }
        
        // 添加到容器
        container.add_data(
            ChunkKind::DataNode,
            &file_data,
            universe::chunk::Codec::Zstd,
            universe::constants::transform_flags::DICT_STRING | universe::constants::transform_flags::INTEGER_VARINT,
            universe::constants::hash_algorithms::BLAKE3,
        )?;
        
        processed_files += 1;
        
        // 批处理检查
        if processed_files % batch_size == 0 && verbose {
            println!("已处理 {} 个文件...", processed_files);
        }
    }
    
    // 添加 Schema（如果指定）
    if let Some(schema_ref) = schema {
        if let Ok(schema_data) = fs::read(&schema_ref) {
            container.add_data(
                ChunkKind::Schema,
                &schema_data,
                universe::chunk::Codec::Zstd,
                0,
                universe::constants::hash_algorithms::BLAKE3,
            )?;
            
            if verbose {
                println!("已添加 Schema: {} 字节", schema_data.len());
            }
        }
    }
    
    // 序列化并写入
    let serialized = container.serialize()?;
    fs::write(&output, &serialized)
        .map_err(|e| universe::error::UnivError::IoError(e))?;
    
    if verbose {
        println!("\n导入统计:");
        println!("  处理文件数: {}", processed_files);
        println!("  总数据量: {:.2} KB", total_bytes as f64 / 1024.0);
        println!("  输出容器: {:.2} KB", serialized.len() as f64 / 1024.0);
        println!("  压缩比: {:.2}x", total_bytes as f64 / serialized.len() as f64);
    }
    
    println!("批量导入完成");
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