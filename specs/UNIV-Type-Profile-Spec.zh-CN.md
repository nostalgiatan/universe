# UNIV TYPE Profile 规范
版本：1.0.0 Release  
状态：Stable  
许可：MIT

## 1. 目的
TYPE Profile（魔数：`UNV1 TYPE`）是 UNIV 类型生态核心载体：组织命名空间、包、类型（UTI IR）、版本、依赖、签名与解析策略，为数据 Profile（RECD/TABL/TSDB/GRPH 等）提供稳健绑定。

## 2. 允许 Chunk
Schema, StringTable, IndexShard, Attachment  
禁止 DataNode / Blob（除文档附件，不参与类型哈希）

## 3. Manifest（必需）
Attachment: `manifest.cbor`（JSON 副本可选）
```
{
 "namespace": "org.example",
 "package": "payments",
 "version": "1.0.0",
 "description": "...",
 "license": "Apache-2.0",
 "exports": [
   {"name":"Invoice","fingerprint":hex32,"iface_fp":hex32}
 ],
 "dependencies": [
   {"namespace":"org.example","package":"identity","range":"^2.1.0","iface_fp":hex32}
 ],
 "registry_hints": {...ResolverHints 子集...},
 "timestamp": int64_ns,
 "not_before": int64_ns?,
 "not_after": int64_ns?,
 "signatures": [
   {"alg":"ed25519","pub":"base64","sig":"base64","covers":"exports+dependencies+timestamp"}
 ]
}
```
签名覆盖：exports + dependencies + timestamp + not_before/not_after（若存在）。

## 4. UTI IR
见 UTI 规范；类型指纹与接口指纹均来源于规范化 CBOR。接口指纹包含默认值（语义敏感），排除注解和文档。

## 5. 命名与 URN
URN: `urn:univ:<namespace>:<package>:<type>[:<version>]`  
版本：SemVer；MAJOR 不兼容；MINOR 可新增可选字段；PATCH 文档。

## 6. 依赖解析
优先级：内嵌 → 本地缓存 → 远程 endpoint → fallback。  
每解析完成：cache (URN@resolved_version → fingerprint)。  
依赖循环禁止（A↔B）；需拆出基础包 C。

## 7. 泛型与实例化
UTI `type_params` + 约束（kind 集合：`record|enum|scalar|ref` …）。  
实例指纹 = blake3(base_fp || param_fp1 || param_fp2 ...)

## 8. 约束
- range / pattern / unique / foreign_key / custom
- foreign_key JSONPath 子集（见 JSONPath EBNF）
- on: restrict|cascade|nullify

## 9. 外键
UTI foreign_key 结构：
```
{
 "type":"foreign_key",
 "src":["$.user_id"],    // JSONPath 子集
 "target_urn":"urn:univ:org.example:identity:User:1.0.0",
 "tgt":["$.id"],
 "on":"restrict"
}
```

## 10. 注解命名空间
保留前缀：`univ.`；语言生成：`codegen.<lang>.`; 验证：`validate.*`; 存储策略：`storage.*`

## 11. 安全
- Manifest 必须至少一个 ed25519 签名（若 trust.requireSignature=true）
- 依赖深度上限（默认 64）
- 不允许匿名/空 namespace
- 不允许未导出的内部类型被外部引用（引用需列入 exports）

## 12. 去重与缓存
Key: fingerprint → bytes（不可变）  
Cache 校验：二次计算指纹 + 接口指纹匹配

## 13. 字段弃用
字段 `deprecated: true` + 注解：
```
annotations: {
 "univ.deprecated_since":"1.3.0",
 "univ.remove_in":"2.0.0"
}
```
工具可发警告。

## 14. 失败代码（工具建议）
T_OK / T_SIG_INVALID / T_VERSION_CONFLICT / T_DEP_CYCLE / T_DEP_MISSING / T_FINGERPRINT_COLLISION

## 15. 演进策略摘要
| 操作 | 允许版本级 | 备注 |
|------|-----------|------|
| 新增可选字段 | MINOR | 无默认值时读取端视为缺失 |
| 新增必须字段 | MAJOR | |
| 删除字段 | MAJOR | |
| 更改字段类型 | MAJOR | |
| 添加枚举成员（末尾） | MINOR | 接口指纹变更 |
| 枚举重排序/删除 | MAJOR | |
| 默认值改变 | MINOR（接口指纹变） | 工具应警告 |
| 约束收紧 | MINOR | |
| 约束放宽破坏安全 | MAJOR | |

## 16. 示例最小包
exports: [Invoice]  
依赖：空  
signatures: 1  
数据文件引用：SchemaRef=Invoice 指纹

TYPE Profile 规范完成。