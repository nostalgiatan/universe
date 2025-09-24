# UTI（UNIV Type IR）规范 v1.0-draft

## 1. 概述
机器友好的中间表示；CBOR 规范化后用于指纹。

## 2. 根对象字段
```
{
 "ns": string,
 "pkg": string,
 "name": string,
 "version": "MAJOR.MINOR.PATCH",
 "kind": "record"|"enum"|"union"|"list"|"map"|"set"|"ref"|"decimal128"|"bytes"|"string"|"int"|"uint"|"float32"|"float64"|"timestamp"|...,
 "type_params": [ { "name":string, "constraints":[string...] } ],
 "fields": [ record_field... ],
 "variants": [ { "name":string, "type": <type_ref>? } ],
 "enum_members": [string...],
 "element": <type_ref>,
 "key": <type_ref>,
 "value": <type_ref>,
 "scale": int? (decimal),
 "constraints": [ constraint... ],
 "annotations": { key:value... },
 "deprecated": bool?
}
```

## 3. record_field
```
{
 "id": int32,
 "name": string,
 "type": <type_ref>,
 "optional": bool?,
 "default": <value>?,
 "annotations": {...},
 "deprecated": bool?
}
```

## 4. type_ref
- 原子：字符串（简标签）
- 复合：对象：  
  - {"list":{"type":<type_ref>}}  
  - {"map":{"key":<type_ref>,"value":<type_ref>}}  
  - {"record":{...嵌套 record}}  
  - {"ref":{"urn"|"fingerprint"}}  
  - {"union":[<type_ref>...]}  
  - {"enum":["A","B",...]}  

## 5. 约束
形如：
```
{ "type":"range", "field":"amount", "min":0 }
{ "type":"pattern", "field":"code", "regex":"^[A-Z]{3}$" }
{ "type":"unique", "fields":["$.id"] }
{ "type":"foreign_key", "src":["$.user_id"], "target_urn":"urn:...:User:1.0.0", "tgt":["$.id"], "on":"restrict" }
```

## 6. 规范化
排序规则：
- 对象键按 ASCII 升序
- 数组顺序保留
- 去除 null/默认空字段
- 布尔、数值、字符串保持原型
NaN、字段 ID 规则同容器规范。

## 7. 指纹
Fingerprint = BLAKE3-256(cbor_canonical_bytes)
InterfaceFingerprint = 同上，但移除 annotations 与 doc.*（保留 default）

## 8. 泛型实例化
实例指纹 = blake3( base_fingerprint || param1_fpr || param2_fpr ... )

## 9. 递归
允许 ref 回指同命名空间类型。禁止无 ref 的直接结构性无限递归（检测深度）。
