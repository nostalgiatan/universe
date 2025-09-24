# UNIV 容器规范（Container Spec）
版本：1.0.0 Release  
发布日期：2025-09-24  
许可：MIT  
状态：Stable

本文件为 UNIV 容器第一版正式发布规范（相对 draft 系列进行了全面整合与增强）。  
本版本落地“分型魔数 + 类型生态 + 追加写入 + 安全与信任”核心设计，并吸收审查阶段全部 P0 改进项。

## 目录
1. 设计概览
2. Profile 分型与魔数
3. 文件结构总览
4. 二进制基础与通用编码约定
5. 文件头（Header）
6. 块（Chunk）帧格式
7. 数据节点（DataNode）与 Blob
8. 追加与流式（Streaming / Append）模式
9. TOC / 索引与分片
10. 内容寻址与哈希策略
11. 引用（Ref）与外部引用（External Ref）
12. 变换（Transforms）与压缩流水线
13. 时间序列 Gorilla-FP 比特级规范
14. 列式与行组（TABL Profile 复用）
15. 规范化与哈希稳定性挂钩点
16. 安全与资源限幅
17. 加密封装（Encryption Envelope）初版
18. Bundle Manifest（可选环境冻结）
19. Profile 约束矩阵（正式版）
20. 错误分类与故障模式
21. 性能与实现建议
22. 测试向量与互操作基线
附录 A：枚举  
附录 B：字段 ID 分配与退化防护  
附录 C：Map/Set 排序理由（Rationale）  
附录 D：Streaming / Provisional TOC 详细  
附录 E：Gorilla-FP 参考伪代码  
附录 F：加密封装结构  
术语表

---

## 1. 设计概览
UNIV = 通用二进制容器 + 分型 Profile 限定特性集合，统一：  
- 块级压缩 / 校验  
- 内容寻址（确定性哈希）  
- 类型引用（与 TYPE Profile 协作）  
- 图 / 列式 / 时序 / Blob 高密度表达  
- 追加写入与尾部索引  
- 安全限幅与供应链信任（Schema 签名、外部引用允许清单）  

避免：“全功能 -> 性能差 / 矩阵爆炸”。

---

## 2. Profile 分型与魔数
前 8 字节魔数：
- Magic4: ASCII "UNV1"
- ProfileCode4: ASCII（A-Z0-9，标准或自定义）

标准 Profile：BLOB / RECD / TABL / TSDB / GRPH / MIXD / TYPE  
自定义：首字母必须 `X`（如 `XAI1`）。  
未知 Profile：
- 若首字母 `X` → 允许降级为“无优化中立解析”（类似 MIXD 的只读模式）
- 否则 → 拒绝解析（E_PROFILE_UNKNOWN）

Profile 稳定级别在“Profile Registry”文档中管理。

---

## 3. 文件结构总览
```
+-----------+--------------+--------------+-------------+-----------+
| Header    | Chunk Frames | (可选追加段) | TOC Shards  | TOC Footer|
+-----------+--------------+--------------+-------------+-----------+
```
追加写入：多段顺序追加，每段可产生新的 Chunk + 临时索引（Provisional TOC），最终以 Final TOC Footer 收尾。

---

## 4. 二进制基础与通用编码约定
- 字节序：Little-Endian（除非单独声明）
- Varint：LEB128（无符号），有符号整数 ZigZag 后编码
- 哈希：默认 BLAKE3-256；外部引用固定使用 multihash(sha256)
- 校验：块级 CRC32C（必需）；可选全局签名
- 字符串：UTF-8 + NFC（可在 Schema 禁用）
- 时间：Timestamp = UTC 纳秒；Date = proleptic Gregorian（天）；Time = 一天内纳秒
- Decimal128：IEEE 754 BID；不支持 NaN / Infinity（发现即错误）
- BigInt：补码编码（见 Canonical Encoding 规范）

---

## 5. 文件头（Header）
结构：
```
Magic4("UNV1") + ProfileCode4
VersionMajor(uint16)=1
VersionMinor(uint16)=0
Flags(uint32)
TOCOffset(uint64)   // 0 表示未知（流式/进行中）
HeaderExtLen(varint)
HeaderExt[HeaderExtLen] (TLV)
```
关键 Flags：
- 0x0001 HasHeaderExt
- 0x0002 StreamedWithoutTOC（进行中）
- 0x0004 ContainsEncryptedChunks
- 0x0008 ContainsSignatures
- 0x0010 ProfileMinorInHeaderExt
- 0x0020 HasProvisionalIndex（存在临时索引段）

HeaderExt（TLV）典型：
- NamespaceRoot (type=10)
- ResolverHints (type=11)
- WindowSizeSeconds (TSDB)
- EncryptionInfo (保留)
- BundleManifestHash (type=20)

---

## 6. 块（Chunk）帧格式
```
"CK01" (4B)
ChunkKind (uint8)
Codec (uint8)         // 0 none, 1 zstd, 2 lz4, 3 deflate
TransformFlags (uint16)
RawSize (uint32)      // 变换前原始负载大小
CompSize (uint32)     // 压缩后大小
HashAlg (uint8)       // 0=blake3-256, 0xFF=multihash
ContentHash (varbytes)// 通常 32B; 若 multihash: code+len+digest
Reserved (uint16)=0
Payload[CompSize]
CRC32C(uint32)        // 覆盖 "CK01" 起至 Payload 末
```
ContentHash = hash(RawPayloadCanonical)。  
RawPayloadCanonical = 子头 + 规范化编码主体（不含压缩与外层头部）。  

ChunkKind（标准集合）：DataNode(0x00), Blob(0x01), Schema(0x02), StringTable(0x03), IndexShard(0x04), Attachment(0x05)。

---

## 7. 数据节点（DataNode）与 Blob
DataNode Raw Payload 子头：
```
Mode(uint8)              // 0=SD 自描述; 1=SG 基于 Schema
SchemaRef 或 TypeTag     // SG: Fingerprint(32B) | URN+版本范围+fingerprint?; SD: Varint(TypeTag)
LayoutFlags(uint16)
LocalStringTableRef? (可选 ContentHash)
Body (规范化值编码)
```
Blob Raw Payload：
```
HashPolicy(uint8)  // 0=data-only,1=payload-inclusive
MIME? (varbytes)
Name? (varbytes)
Length(varint)
Data[Length]
```
HashPolicy=0 → ContentHash 只基于 Data；=1 → 基于整个 RawPayloadCanonical。  
BLOB Profile 默认 0；混用须在 TOC 里记录 hash_policy，工具发告警。

---

## 8. 追加与流式（Streaming / Append）
支持多段写入：
- Provisional Index（临时 TOC Shard）在段尾（标记 HasProvisionalIndex）
- 最终合并写入 Final TOC；Header.TOCOffset 指向最后 TOC Footer
- 中途消费策略：
  1. 扫描自末尾回溯最近 Provisional Footer
  2. 校验相关块 CRC
  3. 标记状态 “PARTIAL”

附录 D 给出 Provisional Footer 结构与状态机。

---

## 9. TOC / 索引与分片
Footer:
```
"TOC1"
ShardCount(varint)
ShardOffsets[ShardCount] (uint64)
CRC32C(uint32)
```
常规索引表（一个或多个 IndexShard 中）：
- chunks: content_hash, file_offset, comp_size, raw_size, kind, codec, hash_alg, transform_flags, hash_policy?
- nodes: node_id, kind, schema_or_type, is_root
- refs: from_node, to_hash, external(bool)
- roots: {name, node_id}

Profile 扩展：
- TABL: 列统计, 行组映射
- TSDB: 时间窗 + series_index
- GRPH: reachability
- BLOB: range-map（也可 Attachment 形式）
- TYPE: manifest, exports, deps（在其专属规范内）

---

## 10. 内容寻址与哈希策略
- NodeID=ContentHash
- 默认哈希：BLAKE3-256；外部引用 multihash(sha256)
- 保证：同一语义值 → 唯一规范化编码 → 稳定哈希
- Bundle Manifest（可选）可列出被视为“版本上下文”的 NodeID 集合，文件可在 HeaderExt 中存其哈希

---

## 11. 引用（Ref）与外部引用（External Ref）
标准 Ref 编码：Tag + HashAlg(1B) + ContentHash(varbytes)  
External Ref：
- hash = multihash(code=0x12, digest=sha256(URN UTF-8 bytes))
- refs 表 external=true
- 不计入循环检测（内部图必须无环）
Allowlist：
- 解析器可通过 ResolverHints.trust.namespaceAllowlist 限制可接受的外部命名空间

---

## 12. 变换与压缩流水线
顺序：
1. 语义变换（Delta/FOR、Gorilla、字典等）
2. 布局变换（Columnarize, BitShuffle/ByteShuffle）
3. 压缩（zstd/lz4/…）

禁止“造最大熵”；压缩器内部熵编码（ANS/Huffman）处理概率均化。  
TransformFlags（位）详见附录 A.4。

---

## 13. 时间序列 Gorilla-FP 比特级规范（TSDB）
块结构：
```
BlockHeader:
  BaseTimestamp(int64 ns)
  BaseValue(float64)      // 原始首值 IEEE 754
  Count(uint32)
  (可选) ValueBitWidthHint(uint8) // 未来兼容
  EncodedDeltas (bitstream)
```
时间戳编码：
- 存储 Δt = t_i - t_{i-1}
- ΔΔt = Δt_i - Δt_{i-1} 以低比特可变长前缀：
  - Pattern:
    1bit '0' → ΔΔt=0
    2bits '10' + 7 bits → 有符号 7 位
    4bits '1100' + 9 bits
    5bits '11010' + 12 bits
    5bits '11011' + 32 bits
（与 Gorilla 原版类似，可调整窗口；超出 → fallback 32 bits）

浮点值编码：
- 取 XOR_i = float_to_u64(v_i) XOR float_to_u64(v_{i-1})
- 若 XOR_i == 0 → 输出单比特 ‘0’
- 否则输出 ‘1’ + leading_zero_count(5 bits) + meaningful_bits_len(6 bits) + meaningful_bits
- 可裁剪末尾全零（由 meaningful_bits_len 控制）

Bitstream 衔接顺序：时间序列按时间排序 → 先编码所有时间，再与值差异交织（推荐 interleaving: [ΔΔt_i][value_i]），工具需统一。

详细参考伪代码：见附录 E。

---

## 14. 列式与行组（TABL）
行组默认目标：128K 行（±25%）；列块内支持：
- Integer-Varint / Delta / FOR
- BitPack（对低基数整型）
- RLE（重复值序列）
- Dict-String（高重复字符串）
列统计最小集：row_count, null_count, min, max  
可选：distinct_approx(HLL)、quantiles(t-digest)  
列块块头应指示编码链顺序（变换 pipeline 描述 TLV）。

---

## 15. 规范化与哈希稳定性挂钩点
- 字段顺序 = 字段 ID 升序
- Map 键排序（类型优先 + 内部自然序）
- Set 排序使用元素规范化编码字节序（允许实现缓存防二次编码）
- Decimal128：BID 编码 + scale
- 字符串：NFC；纯 ASCII 快速路径
- 浮点 NaN：统一 quiet NaN bit pattern
- External Ref 哈希固定 sha256(URN UTF-8)，避免多算法歧义

---

## 16. 安全与资源限幅
推荐默认（可配置）：
- max_chunks = 1,000,000
- max_chunk_raw_size = 32 MiB
- max_total_raw_uncompressed = 256 GiB
- max_ref_depth = 1024
- max_string_table_bytes = 256 MiB
- max_series_window_span = 10 × base_window
- max_schema_dependencies_depth = 64
- min_average_raw_per_chunk (告警) = 4 KiB
压缩炸弹监控：
- 单块 comp/raw ≥ 32 → 警告或拒绝
- 滑动窗口（最近 64 块）平均膨胀 ≥ 10 → 早停
错误分类见第 20 章。

---

## 17. 加密封装（Encryption Envelope）初版
可选策略：先压缩后加密（防止熵提升失败）。  
EncryptedChunk（在 Payload 层包一层 Envelope）：
```
Envelope:
  Version(uint8)=1
  Alg(uint8)=1   // 1=XChaCha20-Poly1305
  KDF(uint8)=1   // 1=HKDF-SHA256
  Salt(16B)
  Nonce(24B)
  Ciphertext(...)
  AuthTag(16B)
```
- 原 RawPayloadCanonical → 压缩 → 加密 → 作为 Payload
- ContentHash 仍对解密前的 RawPayloadCanonical 计算（需存储明文哈希在 Envelope 中）：
  Envelope 内追加：PlainHash(32B) 用于校验
- HashAlg 不改变；若 Envelope 验签失败 → E_DECRYPT_FAIL

详见附录 F。

---

## 18. Bundle Manifest（可选）
用于冻结一组 Schema/配置/字典集合（类似 BOM）：
Attachment: name="bundle-manifest.cbor"
结构：
```
{
 "schemas":[fingerprint...],
 "dictionaries":[hash...],
 "timestamp": int64_ns,
 "purpose":"analytics|serving|...?",
 "annotations":{...}
}
```
HeaderExt 可存其 BLAKE3 指纹。

---

## 19. Profile 约束矩阵（正式）
| Profile | 允许 Chunk | 必/建议 Transform | 禁止 Transform | Hash 默认 | 索引扩展 |
|---------|-----------|------------------|---------------|----------|---------|
| BLOB | Blob, IndexShard, Attachment | CDC(建), ByteShuffle(可) | Columnarize,Gorilla | data-only | range-map |
| RECD | DataNode,Schema,StringTable,IndexShard,Attachment | Dict-String, Varint; Columnarize(可) | Gorilla(值除外) | data-only | 可选无 |
| TABL | DataNode,Schema,StringTable,IndexShard | Columnarize, BitPack, RLE, Delta, Dict-String | Gorilla | data-only | 列统计 |
| TSDB | DataNode,Schema,IndexShard,StringTable | Delta(必), Gorilla-FP(建), Varint | Columnarize | data-only | time_windows, series_index |
| GRPH | DataNode,Schema,StringTable,IndexShard,Attachment | Dict-String, Varint | Columnarize,Gorilla | data-only | reachability |
| MIXD | 全部 | 无强制 | 无 | data-only | 可选 |
| TYPE | Schema,StringTable,IndexShard,Attachment | Dict-String | Columnarize,CDC,Gorilla | payload-inclusive意义弱 | manifest/exports/deps |

---

## 20. 错误分类与故障模式
严格模式（STRICT）：“第一次致命错误终止”；宽松模式（BEST_EFFORT）：尝试跳过损坏 Chunk，记录复原率。  
错误代码：
- E_MAGIC_INVALID
- E_PROFILE_UNKNOWN
- E_VERSION_UNSUPPORTED
- E_TOC_MISSING
- E_TOC_PARTIAL
- E_CHUNK_CRC_FAIL
- E_HASH_MISMATCH
- E_DECRYPT_FAIL
- E_SCHEMA_RESOLVE_FAIL
- E_REF_CYCLE
- E_REF_DEPTH_EXCEEDED
- E_LIMIT_EXCEEDED
- E_FORMAT_VIOLATION
- E_EXTERNAL_REF_DISALLOWED
- E_SIGNATURE_INVALID

---

## 21. 性能与实现建议
- BLAKE3 并行：分块并哈希；合并树根
- zstd 字典集（DictSet）：可在 HeaderExt 或 TOC 增记“适用列模式”
- Set 排序缓存：元素先编码到内存缓冲；复用字节序列排序
- I/O 策略：预读 chunk header；多线程解压队列（线程数 ~ min(核心数, 并行块数)）
- TSDB：分时间窗口并行；聚合路径利用增量统计

---

## 22. 测试向量与互操作基线
建议官方 `testvectors/` 包含：
- 最小 RECD（单字段）
- TABL 行组 + 列统计
- TSDB 块（含 Gorilla 编码）
- GRPH DAG（含 external ref）
- BLOB CDC 分块 + range-map
- EncryptedChunk 示例
- 损坏块 / CRC 失败案例
每个 test vector 附 YAML 元描述：字段、哈希、指纹、预期错误或成功。

---

## 附录 A：枚举（节选）
A.1 Header Flags（见正文）  
A.2 HashAlg: 0=blake3-256, 0xFF=multihash  
A.3 ChunkKind: 0x00..0x05 标准；0x20-0x3F 私有  
A.4 TransformFlags:
- 0x0001 Integer-Varint
- 0x0002 Delta/FOR
- 0x0004 RLE
- 0x0008 BitPack
- 0x0010 ByteShuffle
- 0x0020 BitShuffle
- 0x0040 Gorilla-FP
- 0x0080 Dict-String
- 0x0100 Columnarize
- 0x0200 CDC-Segmented
- 0x0400-0x8000 保留

---

## 附录 B：字段 ID 分配与退化防护（摘要）
- base = blake3_64(ns+"."+pkg+"."+Type+"#"+field) & 0x7FFFFFFF
- 若落入保留区 [0x7FFF0000,0x7FFFFFFF] → base -= 0x10000
- 冲突线性探测：探测步数 > 32 → 切换二级散列：`id = blake3_64("salt"+base+step) & 0x7FFFFFFF`
- 探测循环 → E_FORMAT_VIOLATION

---

## 附录 C：Map/Set 排序 Rationale
Map 键混合类型跨语言序遍历不一致 → 采用（类型顺序+内部自然序）确保稳定。  
Set 排序基于编码字节：保证结构相等 ⇒ 同序。

---

## 附录 D：Streaming / Provisional TOC
Provisional Footer:
```
"PTC1"
ShardCount(varint)
ShardOffsets[]
CRC32C
```
多次出现 → 消费者可读取“最后一个有效 PTC1”作为临时视图。Final 写入 "TOC1" 并更新 Header.TOCOffset。

---

## 附录 E：Gorilla-FP 伪代码（简）
见单独 Gorilla 章节（此处留实现者参考），校验 test vector。略。

---

## 附录 F：加密封装结构
详见第 17 章，Envelope 内部 PlainHash 使得完整性先于解密验证（需 KDF+AEAD）。

---

## 术语表
（略，同前版本：Chunk, Node, Profile, SchemaRef, Fingerprint, Bundle, External Ref, Provisional TOC 等）

本规范至此完成 v1.0.0 发布基线。