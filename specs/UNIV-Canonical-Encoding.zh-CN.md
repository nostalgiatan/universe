# UNIV 规范化编码细则 v1.0-draft

## 1. 目的
定义值到字节序的唯一映射确保哈希稳定。

## 2. 总则
- 所有结构必须完全确定顺序
- 空与缺省区分：缺省→省略；显式 null→编码 Null Tag
- 字段 ID 升序；Map 键按规则排序
- 字符串 NFC；无非法 UTF-8
- 浮点：IEEE 754，NaN 统一为 quiet NaN (0x7FF8000000000000 for f64)
- Decimal128：BID 16 字节 + scale(int32 varint) 连续编码（scale 在前）

## 3. 基元编码
Tag(Varint) + Payload  
- Bool: Tag + 1字节(0/1)  
- Int/UInt: ZigZag(若有符号) → Varint  
- String: Varint(len_utf8) + bytes  
- Bytes: Varint(len) + raw  
- Timestamp/Date/Time/Duration: int64 ns or days→Varint  
- UUID: 固定 16 字节  

## 4. 复合
List: Tag + Varint(len) + elements  
Map: Tag + Varint(len) + 每键值：KeyEnc + ValueEnc  
Record (Schema 模式)：位图(optional) + 按字段 ID 顺序值序列  
Union: Tag + Varint(variant_index) + payload  
Enum: Tag + Varint(index)  
Set: 去重后排序（按元素编码字节序比较）  
Ref: Tag + HashAlg(1B) + ContentHash(varbytes)

## 5. 可选字段位图
按字段 ID 顺序分组（8 字段/位），位图后紧跟存在字段的值串。

## 6. Map 键类型标记
Key = KeyTypeTag + EncodedKeyValue  
KeyTypeTag：0=String,1=Int,2=UInt,3=UUID

## 7. 规范化比较
Set 排序：先生成元素临时编码 → 按字节字典序 → 连接

## 8. 外部 Ref
Ref Tag + HashAlg=0x12 + digest(sha256(URN bytes))

## 9. Canonical Hash 输入
RawPayloadCanonical = 子头（模式/SchemaRef/布局字段） + Body（已按上述规范编码）  
ContentHash = blake3_256(RawPayloadCanonical)
