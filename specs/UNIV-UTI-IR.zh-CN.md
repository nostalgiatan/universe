# UTI（UNIV Type IR）规范
版本：1.0.0 Release  
状态：Stable

## 1. 目标
机器友好中间表示，支持：结构/泛型/约束/默认值/注解；用于指纹计算与代码生成。

## 2. 根对象字段
必填：ns, pkg, name, version, kind  
可选：type_params, fields, variants, enum_members, element, key, value, scale, constraints, annotations, deprecated

## 3. kind 枚举
`null|bool|int|uint|bigint|float32|float64|decimal128|string|bytes|timestamp|date|time|duration|uuid|record|enum|union|list|map|set|ref|any`

## 4. record_field
```
{
 "id": int32,
 "name": string,
 "type": <type_ref>,
 "optional": bool?,
 "default": <value>?,      // 编码遵循 Canonical Encoding 子集
 "annotations": {...}?,
 "deprecated": bool?
}
```

## 5. type_ref
- 简名（内建标量）
- 结构对象：
  - {"list": {"type": <type_ref>}}
  - {"map": {"key":<type_ref>, "value":<type_ref>}}
  - {"set": {"type":<type_ref>}}
  - {"record": {...inline record UTI...}}
  - {"union":[<type_ref>...]}
  - {"enum":["A","B",...]}
  - {"ref":{"urn": "..."}}
  - {"ref_fp":{"fingerprint": "hex32"}}

## 6. 约束
对象数组，每元素：
- range: {"type":"range","field":"amount","min":0,"max":100?}
- pattern: {"type":"pattern","field":"code","regex":"^[A-Z]+$"}
- unique: {"type":"unique","fields":["$.id","$.country"]}
- foreign_key: {"type":"foreign_key","src":["$.user_id"],"target_urn":"urn:...:User:1.0.0","tgt":["$.id"],"on":"restrict"}
- custom: {"type":"custom","name":"urn:vendor:rule:xyz","params":{...}}

## 7. JSONPath 子集（外键索引）
EBNF（见 UTL / JSONPath 文件，也在本处引用）：
```
Path       = "$" ( Segment )*
Segment    = "." Name | "[" Index "]"
Name       = /[A-Za-z_][A-Za-z0-9_]*/
Index      = DIGIT+
```
不允许通配符 / 过滤 / 多选。

## 8. 泛型
`type_params`: [{"name":"T","constraints":["record","ref","scalar"]}]  
实例指纹 = blake3(base_fp || param1_fp || ...)

约束含义：
- scalar: 原子基元（不含 record/union 等）
- record: kind=record
- ref: 允许引用类型
实现保留“扩展约束空间”。

## 9. 注解
键：UTF-8，不含空白；保留前缀：`univ.`  
用途：文档(`doc`), codegen(`codegen.java.class`), 验证策略(`validate.mode`)

## 10. 规范化规则
- JSON → CBOR → Canonical：对象键按 ASCII 排序
- 删除空值（null/空数组/空对象）除非语义字段（enum_members 必需保留）
- 字段 ID 升序
- 枚举顺序保留定义序（影响指纹）

## 11. 指纹
Fingerprint = blake3_256(cbor_canonical)  
InterfaceFingerprint = 同上但剔除 annotations 文档类键（`doc`, `codegen.*`）保留默认值  

## 12. 两阶段解析
1) 预注册：ns/pkg/name/version -> 占位 (无字段)
2) 加载 body：验证引用存在
3) 检测循环（除合法 ref 自回） → 违规报错

## 13. BigInt / Decimal 语义
- BigInt: 任意精度（上限实现限制）
- Decimal128: 固定精度 + scale；不支持 NaN/Inf

## 14. 默认值限制
- 不得引用循环结构
- 不得包含外部 ref
- 枚举默认值必须为有效成员

## 15. 枚举演进
- 新增成员置末尾；原成员顺序不可改变
- 删除或重排 → MAJOR 版本

## 16. 废弃字段
`deprecated: true` + 注解说明；工具提示但允许读取。

## 17. 错误分类（UTI）
U_KIND_INVALID / U_FIELD_ID_DUP / U_ENUM_DUP / U_CONSTRAINT_INVALID / U_GENERIC_CONSTRAINT_FAIL / U_JSONPATH_INVALID / U_FINGERPRINT_MISMATCH

UTI 规范完成。