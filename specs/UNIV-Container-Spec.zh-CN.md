# UNIV 容器规范 v1.0-draft

状态：Draft-1.0  
最后更新：2025-09-23  
许可：MIT

（本文件整合 v0.3 并采纳全部开放问题决议）

## 目录
1. 概述
2. 分型魔数与 Profile
3. 二进制通用约定
4. 文件头
5. Chunk 帧结构
6. TOC 与索引
7. 变换与压缩流水线
8. 类型系统钩子（与 TYPE Profile 协作）
9. 值与规范化编码概述
10. Profile 约束（终版）
11. DataNode 与 Blob 细则
12. 内容寻址与引用
13. 分块策略
14. 安全与限幅
15. 版本与兼容策略
16. 命名根 RootSet
17. Range Map（Blob）
18. GRPH 外部引用
19. 决议要点摘要
20. 实现建议
附录 A：枚举  
附录 B：字段 ID 冲突算法伪代码  
附录 C：规范化排序规则  
附录 D：错误分类

---

## 1. 概述
UNIV 提供一个统一二进制容器，用分型 Profile 控制特性集合与优化路径，避免“通用=臃肿”。支持：块级压缩、内容寻址、随机访问、Schema 引用、外部类型仓库、可演进与安全限幅。

## 2. 分型魔数与 Profile
首 8 字节：
- Magic4 = "UNV1"
- ProfileCode4：标准或自定义（标准：BLOB/RECD/TABL/TSDB/GRPH/MIXD/TYPE；自定义以 'X' 起）

未知 Profile：
- 若首字符 'X'：可按 MIXD 降级只读（不执行特定优化）
- 否则：视为不可识别错误

## 3. 二进制通用约定
Little-Endian；Varint=LEB128；ZigZag 编码有符号；哈希=默认 BLAKE3-256；校验=CRC32C；压缩=zstd(推荐), lz4, deflate, none；NFC 字符串标准化（可在 Schema 中关闭）。

## 4. 文件头
字段：
- VersionMajor(uint16)=0, VersionMinor(uint16)=1(容器 v1.0-draft)
- Flags(uint32)
- TOCOffset(uint64)
- HeaderExtLen(varint)
- HeaderExt(TLV)

扩展 TLV（新增含解析安全项）详见附录 A.2。

## 5. Chunk 帧结构
"CK01" + ChunkKind(uint8)+Codec(uint8)+TransformFlags(uint16)+RawSize(uint32)+CompSize(uint32)+HashAlg(uint8)+ContentHash(varbytes)+Reserved(uint16)+Payload+CRC32C(uint32)

ContentHash = 哈希(RawPayloadCanonical)；RawPayloadCanonical 不含压缩/头。

ChunkKind 见附录 A.3。

## 6. TOC 与索引
尾部：IndexShard 块 1..N；TOC Footer：
- "TOC1"+ShardCount+ShardOffsets[]+CRC32C

标准通用表：
- chunks
- nodes
- refs（含 external 标志）
- roots（{name,node_id}，name 唯一）
Profile 特定扩展：列索引、时间窗、range-map、reachability、统计。

## 7. 变换与压缩流水线
顺序：语义变换 → 布局/列式 → 压缩  
TransformFlags 详见附录 A.4。禁止最大熵化预处理。  
列式：TABL 强制；RECD 大批量可选；TSDB 不使用列式而使用时间片块化。

## 8. 类型系统钩子
DataNode SG 模式携带 SchemaRef（指纹 / URN+版本范围 / 内嵌）。解析按 决议顺序。  
UTI/TYPE 详见独立文档。

## 9. 值与规范化编码概述
参见 Canonical-Encoding 文档（单独文件）：
- Map/Record 字段排序规则固定（字段 ID 或键排序）
- 字符串 NFC
- Float NaN 统一 quiet 模式
- Decimal128 = BID + scale
- 外部引用 canonical multihash

## 10. Profile 约束（终版表）

| Profile | 允许 Kind | 必/建议 Transform | 禁止 Transform | Hash 策略默认 | 关键索引 |
|---------|-----------|------------------|---------------|--------------|----------|
| BLOB | Blob, IndexShard, Attachment | CDC(建议) | Columnarize,Gorilla | data-only | range-map |
| RECD | DataNode,Schema,StringTable,IndexShard,Attachment | Dict-String,Integer-Varint（建议）,Columnarize(可选) | Gorilla（除时间字段不可） | data-only | (可空) |
| TABL | DataNode,Schema,StringTable,IndexShard | Columnarize,BitPack,RLE,Delta,Dict-String | Gorilla | data-only | 列索引/统计 |
| TSDB | DataNode,Schema,IndexShard,StringTable | Delta(时间戳 必),Gorilla-FP(建议),Varint | Columnarize | data-only | time_windows,series_index |
| GRPH | DataNode,Schema,StringTable,IndexShard,Attachment | Dict-String,Integer-Varint | Columnarize,Gorilla | data-only | reachability(可选) |
| MIXD | 全部 | 无强制 | 无 | data-only | 可选 |
| TYPE | Schema,StringTable,IndexShard,Attachment | Dict-String(建议) | Columnarize,CDC,Gorilla | payload-inclusive（对 Schema 块无意义） | manifest 相关 |

## 11. DataNode 与 Blob 细则
DataNode 子头：Mode(0/1)+TypeTag|SchemaRef+LayoutFlags+LocalStringTableRef?+Body  
Blob Payload：MIME?+Name?+Length+Data  
hash_policy 在 Index 中记录（0=data-only,1=payload-inclusive）。BLOB 默认 0；可用 1 保留 MIME/Name 变更敏感度。

## 12. 内容寻址与引用
NodeID=ContentHash；Ref = HashAlg+ContentHash；external 引用：multihash(sha256(URN utf8))，refs 表标记 external=true，不计入循环检测。  
循环：内部节点必须 DAG；检测发现环→解析错误。

## 13. 分块策略
块大小建议 256 KiB 目标（范围 64 KiB–4 MiB），大字段可启用 CDC。TABL 列块对齐行组；TSDB 按时间窗口切块。

## 14. 安全与限幅
默认限幅（可配置）：
- max_chunks=1,000,000
- max_raw_size=256 GiB
- max_chunk_raw=32 MiB
- max_ref_depth=1024
- max_string_table=256 MiB
- max_series_window_span = 10 × base_window
校验：CRC32C 必须成功；压缩膨胀阈值 comp/raw ≥ 32 触发警告或拒绝。

## 15. 版本与兼容策略
Minor 增强向后兼容；Major 不兼容。  
保留前向兼容：未知 ChunkKind (非关键路径) 可跳过；未知 TransformFlags 忽略该标志效果。

## 16. 命名根 RootSet
结构：`[{name(UTF-8 NFC), node_id}]`  
名称唯一；保留 `_` 前缀系统；建议主入口 `main`。  
若没有根→默认将第一 DataNode/Blob 设为 `_default`（可配置）。

## 17. Range Map（Blob）
Attachment: name=`"range-map.cbor"` 内容：数组：
```
[ { "offset":uint64, "length":uint64, "hash":ContentHash, "mediaOffset":uint64? }, ... ]
```

## 18. GRPH 外部引用
Ref 标记 external；只包含 multihash(sha256(URN))；Index refs 增列 `external`=bool。  
解码不展开；客户端可通过 TYPE/Registry 解析 URN。

## 19. 决议要点摘要
详见顶层“决议摘要”列表（不再重复）。

## 20. 实现建议
- 首先实现 RECD + TYPE → 支撑 Schema 化对象
- 后续扩展 TABL（复用列式编码库 / Arrow 互操作）
- 哈希使用 BLAKE3 并行加速
- 针对 TSDB：优先实现 Delta+Gorilla 组合
- 灰度期：工具链输出调试 JSON 视图

---

### 附录 A：枚举

A.1 Header Flags  
- 0x1 HasHeaderExt  
- 0x2 StreamedWithoutTOC  
- 0x4 ContainsEncryptedChunks  
- 0x8 ContainsSignatures  
- 0x10 ProfileMinorInHeaderExt  

A.2 Header 扩展（TLV）  
- 1 Producer (UTF-8)  
- 2 CreationTimestamp (uint64 ns)  
- 3 AppHint (UTF-8)  
- 4 DefaultCodec (uint8+params)  
- 5 DefaultHashAlg  
- 6 ProfileMinor/Options (CBOR)  
- 10 NamespaceRoot (UTF-8)  
- 11 ResolverHints (CBOR) 结构含 endpoints/policy/trust/fallback  
- 12 WindowSizeSeconds (TSDB)  
- 13 ColumnGroupHint (TABL)  
- 14 EncryptionInfo (保留)  

A.3 ChunkKind  
0x00 DataNode  
0x01 Blob  
0x02 Schema  
0x03 StringTable  
0x04 IndexShard  
0x05 Attachment  
0x20-0x3F 私有

A.4 TransformFlags  
与前版本一致（见 v0.3），无新增。

### 附录 B：字段 ID 冲突算法伪代码
```
fn allocate_field_id(ns, pkg, type_name, field_name):
    base = blake3_64(ns+"."+pkg+"."+type_name+"#"+field_name) & 0x7FFFFFFF
    if base in RESERVED_RANGE: base = (base - 0x10000) & 0x7FFFFFFF
    id = base
    while id in used_ids:
        id = (id + 1) & 0x7FFFFFFF
        if id == base: error("exhausted")
    used_ids.add(id)
    return id
```

### 附录 C：规范化排序
Map 键：类型序 String<Int<UInt<UUID；同类型内部按自然序；字段按 ID 升序。  
字符串 NFC；枚举成员按定义顺序；Union Tag 按声明顺序分配 varint。

### 附录 D：错误分类
- E_MAGIC_INVALID
- E_VERSION_UNSUPPORTED
- E_PROFILE_UNKNOWN
- E_TOC_MISSING
- E_CHUNK_CRC_FAIL
- E_HASH_MISMATCH
- E_SCHEMA_RESOLVE_FAIL
- E_REF_CYCLE
- E_REF_DEPTH_EXCEEDED
- E_LIMIT_EXCEEDED
- E_FORMAT_VIOLATION
