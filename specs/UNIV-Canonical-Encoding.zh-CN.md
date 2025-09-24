# UNIV 规范化编码（Canonical Encoding）
版本：1.0.0 Release  
状态：Stable

## 1. 目的
为内容寻址提供唯一字节序；任何实现给出同一语义值 → 相同哈希。

## 2. 总则
- 无多义：排序 / 正规化规则固定
- 排除非语义元（注解）出值域
- 禁止“惰性随机”编码顺序
- 失败即报告 E_FORMAT_VIOLATION

## 3. 基元
| 类型 | 编码 |
|------|------|
| Null | Tag(null) |
| Bool | Tag(bool)+1B(0/1) |
| Int / UInt | ZigZag(仅 Int)+Varint |
| BigInt | Tag + Varint(len L) + two’s complement big-endian L bytes（值 0 → L=1,0x00） |
| Float32/64 | IEEE 原始字节；NaN 规范化 quiet pattern (f64=0x7FF8000000000000) |
| Decimal128 | Tag + scale(ZigZag->Varint) + 16B BID 格式 |
| Bytes | Varint(len)+raw |
| String | Varint(utf8_len)+NFC bytes |
| Timestamp | Varint(int64_ns ZigZag) |
| Date | Varint(int32 days) |
| Time | Varint(int64 ns_in_day) |
| Duration | Varint(int64 ns ZigZag) |
| UUID | 16B big-endian |

## 4. 复合
List: Tag + Varint(count) + 连续元素  
Map: Tag + Varint(count) + (KeyEntry...)  
KeyEntry = KeyTypeTag(0=String,1=Int,2=UInt,3=UUID)+KeyEncoded+ValueEncoded  
排序：按（类型顺序：String<Int<UInt<UUID）然后类型内自然序  
Record (Schema 模式)：Optional Bitmap（字段 ID 升序） + 存在字段值序列  
Union: Tag + Varint(variant_index) + payload  
Enum: Tag + Varint(member_index)  
Set: 去重（语义）→ 每元素编码一次 → 用编码字节临时数组排序（字典序）→ 连接  
Ref: Tag + HashAlg(1B) + ContentHash(varbytes)  
External Ref: HashAlg=0x12 (sha2-256 multihash code) + digest(sha256(URN UTF-8 bytes))

## 5. 可选字段位图
- 块大小 = 8 字段 / 字节
- 顺序：字段 ID 升序
- 位=1 → 有值；位=0 → 缺省（非显式 Null）

显式 Null：仍需输出 Null Tag（使“缺省”与“显式为 Null”区分）。

## 6. 字符串正规化
流程：
1. 快速检测：若全部字节 <0x80 → 跳过正规化
2. 否则 NFC；非法序列 → E_FORMAT_VIOLATION

## 7. BigInt 规则
- 无前导 0（除值 0）
- two’s complement：最高有效位决定符号
- 最大长度（默认 512B，可配置）；超限 E_LIMIT_EXCEEDED

## 8. Decimal128
- 不允许 NaN/Infinity；出现报错
- scale = int32 ZigZag→Varint
- 16 字节 IEEE 754 BID（低端序列）

## 9. Canonical Hash 输入
DataNode/Blob RawPayloadCanonical = 子头（含 Mode/SchemaRef/hash_policy 等）+ 编码主体  
ContentHash = blake3_256(RawPayloadCanonical)  
加密时：PlainHash=同上；Envelope 外层不改变原哈希语义。

## 10. 性能提示
Set 排序可缓存元素编码避免二次编码  
Map 键通常单类型；实现可分支快路径（全部 String → 直接 UTF-8 排序）  
字符串 ASCII 快路径可显著减少 NFC 开销

## 11. 反例（错误）示意
- Map 键未排序 → hash mismatch
- 重复 Set 元素未去重 → hash 变化
- BigInt 前导 0 → E_FORMAT_VIOLATION

## 12. 测试向量（建议）
- Small map 混合键类型
- Set 含复合结构
- Decimal128 正数/负数/scale 变化
- External Ref URN
- BigInt 边界值（0, -1, 2^4095）

规范化编码说明至此结束。