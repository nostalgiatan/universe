# UNIV TYPE Profile 规范 v1.0-draft

状态：Draft-1.0  
最后更新：2025-09-23  
许可：MIT

## 1. 目的
集中封装命名空间、包、类型（UTI IR）、版本、依赖、签名与分发元数据，为数据 Profile 引用提供稳定与安全基础。

## 2. 允许 Chunk / 约束
ChunkKind：Schema, StringTable, IndexShard, Attachment  
Transform：Dict-String(建议)；禁止 Columnarize/CDC/Gorilla  
Codec：zstd  
必须包含清单（manifest）与 exports、deps 索引。

## 3. 命名模式
namespace（反向域）+ package（短名）+ type（帕斯卡）  
URN：`urn:univ:<ns>:<pkg>:<type>[:<version>]`  
版本：SemVer

## 4. Manifest
结构（CBOR/JSON 同步）：
```
{
  "namespace": "org.example",
  "package": "payments",
  "version": "1.0.0",
  "description": "...",
  "license": "Apache-2.0",
  "exports": [{ "name":"Invoice", "fingerprint":hex32, "iface_fp":hex32 }],
  "dependencies": [{
     "namespace":"org.example",
     "package":"identity",
     "range":"^2.1.0",
     "iface_fp":hex32
  }],
  "registry_hints": {... 与 ResolverHints 兼容 ...},
  "signatures": [{
     "alg":"ed25519",
     "pub":"base64",
     "sig":"base64",
     "covers":"exports+dependencies"
  }]
}
```

## 5. Schema (UTI IR) 与指纹
- 规范化 CBOR → Fingerprint = BLAKE3-256
- 接口指纹包含默认值（防语义漂移）
- 同类型新版本：MAJOR 不兼容变更；MINOR 可添加可选字段

## 6. 泛型
`type_params`: [{"name":"T","constraints":["record","ref" ...]}]  
实例化：生成复合指纹：hash( base_fingerprint || param_fingerprint... )

## 7. 约束
约束集合：range/pattern/unique/foreign_key/custom。  
foreign_key 使用 JSONPath 子集；on=restrict|cascade|nullify

## 8. 外键表达
UTI foreign_key：
```
{
 "type":"foreign_key",
 "src":["$.user_id"],
 "target_urn":"urn:univ:org.example:identity:User:1.0.0",
 "tgt":["$.id"],
 "on":"restrict"
}
```

## 9. 依赖解析
Resolution 顺序：内嵌→缓存→远程。远程需校验：签名→接口指纹→版本范围。  
签名策略由 ResolverHints.trust 控制。

## 10. 注解空间
`annotations`：键以命名层次，如：
- doc.*
- codegen.<lang>.*
- validate.*
- storage.*
- security.*

保留前缀：`univ.`

## 11. 安全
- Manifest 签名失败 → 拒绝
- 超限：schema 数量、字符串表、依赖深度（默认 64）
- 外部引用 URN 解析需防缓存投毒（指纹二次校验）

## 12. 工具推荐命令
- `univ type pack`
- `univ type publish`
- `univ type resolve`
- `univ type verify`
- `univ codegen <lang>`
- `univ data pack --profile RECD --schema urn:...`
- `univ inspect file.unv1`

## 13. 示例
最小包：Invoice  
数据文件引用：SchemaRef=指纹 或 URN+范围

## 14. 版本策略
MINOR：新增可选字段/注解  
PATCH：文档或非语义注解  
MAJOR：字段删除/类型改变/约束放宽破坏兼容

## 15. 返回码（工具层建议）
- T_OK
- T_SCHEMA_DUP
- T_FINGERPRINT_COLLISION
- T_SIG_INVALID
- T_VERSION_CONFLICT
- T_DEP_MISSING

## 16. 开放点（后续版本）
- 多语言 IDL round-trip（UTL 与 UTI 扩充宏）
- taxonomy/ontology 标记层
- 可组合验证表达式 DSL
