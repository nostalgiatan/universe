# UNIV (Universe) 容器格式库 v1.1.0

[![Version](https://img.shields.io/badge/version-1.1.0-brightgreen.svg)](https://github.com/nostalgiatan/universe)
[![Status](https://img.shields.io/badge/status-stable-success.svg)](https://github.com/nostalgiatan/universe)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

UNIV 是一个统一的二进制容器格式的 Rust 实现，支持多种数据模式和优化策略。  
本项目基于 **UNIV v1.1.0 规范**，提供完整且稳定的容器格式实现，具备跨平台SIMD加速和统一API设计。

## 🎯 版本 1.1.0 新特性

- ✅ **统一API入口** - 简化的API设计，减少重复代码和复杂性
- ✅ **跨平台SIMD加速** - 自动利用向量指令优化数据处理和帧扫描
- ✅ **增强的命令行工具** - 新增 `benchmark` 和 `optimize` 命令
- ✅ **零编译警告** - 完全清理的代码库，生产环境就绪
- ✅ **智能默认设置** - `add_data_simple()` 方法自动选择最佳参数
- ✅ **统一验证接口** - `verify(parallel)` 统一并行/串行验证入口

## 特性

- 🚀 **多Profile支持**: 支持 BLOB、RECD、TABL、TSDB、GRPH、TYPE 等标准Profile
- 🗜️ **灵活压缩**: 支持 zstd、lz4、deflate 等多种压缩算法
- 🔗 **内容寻址**: 基于哈希的内容寻址和引用系统
- 🛡️ **安全保护**: 内置安全限制和攻击检测机制
- 📊 **数据变换**: 字典压缩、Delta编码、列式化等数据优化
- 🔍 **快速访问**: TOC索引支持随机访问和快速查找
- 📖 **完整文档**: 中文注释和完整的API文档

## 快速开始

添加到您的 `Cargo.toml`:

```toml
[dependencies]
universe = "1.1.0"
```

### 基本使用（1.1.0 简化API）

```rust
use universe::{Container, Profile, ChunkKind};

// 创建容器并使用简化API添加数据
let mut container = Container::new(Profile::Recd);

// 智能添加数据（自动选择最佳压缩和哈希算法）
container.add_data_simple(ChunkKind::DataNode, b"Hello, UNIV!")?;

// 统一验证接口（支持并行/串行）
container.verify(true)?; // 并行验证

// 序列化
let bytes = container.serialize()?;
println!("容器大小: {} 字节", bytes.len());
```

```rust
use universe::{Container, Profile, Header, Chunk, ChunkKind};
use universe::constants::hash_algorithms;

// 创建容器
let container = Container::new(Profile::Recd);

// 创建文件头
let mut header = Header::new(Profile::Recd);
header.set_producer("my-app");
header.set_namespace_root("org.example");

// 创建数据块
let data = b"Hello, UNIV World!";
let chunk = Chunk::new(
    ChunkKind::DataNode,
    data,
    universe::chunk::Codec::Zstd,
    0,
    hash_algorithms::BLAKE3,
)?;

// 验证数据完整性
chunk.verify()?;
```

### 高级功能

```rust
use universe::{
    reference::{DataNode, ReferenceGraph},
    transform::StringDictionary,
    security::SecurityContext,
    util::hash::ContentHash,
};

// 内容寻址和引用
let node = DataNode::new(data.to_vec(), hash_algorithms::BLAKE3)?;
let mut graph = ReferenceGraph::new();
graph.add_node(node)?;
graph.check_cycles()?; // 循环检测

// 字符串字典压缩
let mut dict = StringDictionary::new();
let index = dict.add_string("repeated_string".to_string());

// 安全验证
let mut security_context = SecurityContext::new();
security_context.validate_container()?;

// SIMD加速的帧扫描（v1.1.0新功能）
#[cfg(feature = "simd")]
{
    use universe::chunk::simd;
    let positions = simd::scan_frame_headers_simd(&data);
}
```

## 支持的Profile

| Profile | 描述 | 状态 | 特点 |
|---------|------|------|------|
| BLOB | 大对象/媒体文件 | 稳定 | 范围映射、CDC分块 |
| RECD | 结构化记录 | 稳定 | Schema引用、可选列式化 |
| TABL | 列式表 | 稳定 | 强制列式化、分析优化 |
| TSDB | 时间序列 | 稳定 | 时间窗口、Gorilla压缩 |
| GRPH | 图/DAG | Beta | 外部引用、可达性索引 |
| TYPE | 类型仓库 | 稳定 | Schema分发、依赖解析 |
| MIXD | 混合 | 遗留 | 最大兼容性 |

## 架构

```
universe/
├── src/
│   ├── constants.rs      # 格式常量和枚举
│   ├── error.rs          # 错误处理系统
│   ├── profile.rs        # Profile系统
│   ├── header.rs         # 文件头处理
│   ├── chunk.rs          # 数据块系统
│   ├── toc.rs            # 目录索引
│   ├── transform.rs      # 数据变换
│   ├── reference.rs      # 引用系统
│   ├── security.rs       # 安全验证
│   └── util/
│       ├── hash.rs       # 哈希算法
│       ├── varint.rs     # 可变长编码
│       └── validation.rs # 验证工具
├── examples/
│   └── basic_usage.rs    # 使用示例
└── specs/                # 规范文档
```

## 运行示例

```bash
# 运行基础使用示例
cargo run --example basic_usage

# 使用CLI工具创建容器
cargo run --bin cil -- create -o example.univ --producer "MyApp" --namespace "com.example"

# 查看容器信息
cargo run --bin cil -- info example.univ --chunks --toc

# 验证容器完整性
cargo run --bin cil -- verify example.univ --strict

# 提取容器数据
cargo run --bin cil -- extract example.univ -o extracted/

# 运行所有测试
cargo test

# 生成文档
cargo doc --open
```

## CLI 工具 (CIL) v1.1.0

本项目包含了官方的命令行工具 `cil`，提供完整的 UNIV 容器操作功能：

### 基础命令
- **create**: 创建各种 Profile 类型的容器
- **info**: 查看容器详细信息
- **verify**: 验证容器完整性和规范符合性  
- **extract**: 提取容器中的数据块

### 新增命令 (v1.1.0)
- **benchmark**: 性能基准测试，支持SIMD和并行测试
- **optimize**: 容器优化，支持零拷贝和重压缩

### 使用示例

```bash
# 创建容器
cil create -o my-container.univ --producer "我的程序" --namespace "org.example"

# 性能基准测试
cil benchmark my-container.univ --rounds 10 --simd --parallel

# 优化容器
cil optimize input.univ -o optimized.univ --zero-copy --recompress

# 查看帮助
cil --help
```

## 测试覆盖

- ✅ **75个单元测试**全部通过（包含CLI测试）
- ✅ **7个文档测试**全部通过
- ✅ 覆盖所有核心功能模块
- ✅ 包含错误处理和边界条件测试
- ✅ CLI工具集成测试覆盖

## 依赖项

本项目使用以下主要依赖：

- `blake3` - BLAKE3哈希算法
- `zstd`, `lz4_flex`, `flate2` - 压缩算法
- `serde`, `ciborium` - 序列化支持
- `crc32c` - CRC校验
- `chrono` - 时间处理
- `bytes` - 字节操作
- `leb128` - 可变长整数编码

## 规范符合性

本实现严格遵循 UNIV 容器规范 v1.0.0 正式版本，包括：

- 📋 魔数和Profile系统
- 🔧 Chunk帧结构和压缩流水线
- 📚 TOC索引和快速访问
- 🔐 安全限制和验证机制
- 🔗 内容寻址和引用系统
- 📊 数据变换和优化策略

## 许可证

MIT License - 查看 [LICENSE](LICENSE) 文件了解详情。

## 贡献

欢迎提交 Issue 和 Pull Request！在提交之前请确保：

1. 运行 `cargo test` 确保所有测试通过
2. 运行 `cargo fmt` 格式化代码
3. 运行 `cargo clippy` 检查代码质量
4. 添加适当的测试和文档

## 更多信息

- 📖 [API文档](target/doc/universe/index.html) (运行 `cargo doc --open` 生成)
- 📋 [UNIV规范](specs/) - 详细的格式规范文档
- 🎯 [使用示例](examples/) - 更多使用示例