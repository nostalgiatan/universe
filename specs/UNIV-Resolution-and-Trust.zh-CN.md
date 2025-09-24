# UNIV 解析与信任模型
版本：1.0.0 Release  
状态：Stable

## 1. 解析顺序
SchemaRef 解析优先级：
1) 内嵌 Schema（同文件）
2) 本地缓存（指纹 / URN->指纹映射）
3) 远程 endpoints（ResolverHints.endpoints 顺序）
4) fallback（ipfs:// / oci:// 等）

失败 → E_SCHEMA_RESOLVE_FAIL

## 2. ResolverHints 结构
HeaderExt type=11 (CBOR)：
```
{
 "endpoints": ["https://registry.example","https://mirror.example"],
 "policy": "offline-first" | "online-first",
 "trust": {
    "mode":"pinned"|"ca"|"web-of-trust",
    "keys":["base64pub"...],
    "requireSignature": true,
    "namespaceAllowlist": ["org.example","com.vendor"]
 },
 "fallback": ["ipfs://bafy...","oci://repo/univ"]
}
```

## 3. 签名验证
- Manifest 签名覆盖：exports + dependencies + timestamp + not_before + not_after
- requireSignature=true 且签名缺失/无效 → 拒绝
- mode=pinned：pub 必在 keys 内
- mode=ca：可委托系统信任锚（扩展）
- mode=web-of-trust：后续扩展（未实现）

## 4. 缓存策略
Cache Key：
- (URN@resolved_version) → fingerprint
- fingerprint → TYPE 包字节
缓存校验：重算指纹、一致则可用，否则丢弃

## 5. 外部引用（External Ref）
- external=true 不自动解析（除非策略允许）
- URN namespace 不在 allowlist → E_EXTERNAL_REF_DISALLOWED
- 可配置：strict_external=false → 记录但不跟踪

## 6. 重放与版本降级防护
- not_before / not_after 超界 → 拒绝
- 若缓存存在更高版本且策略 disallow-downgrade → 拒绝较低版本（可配置）

## 7. 审计日志建议字段
```
{
 "time": ts,
 "action": "resolve"|"verify"|"cache_hit"|"cache_store",
 "urn": "...",
 "fingerprint": "...",
 "source": "embedded"|"cache"|"remote"|"fallback",
 "signature_ok": true|false,
 "latency_ms": 12
}
```

## 8. Bundle Manifest 协作
若 HeaderExt.BundleManifestHash 存在 → 解析器可优先加载所有列出的 TYPE 包 → 冻结解析上下文，防止“幽灵升级”。

## 9. 失败分类（解析阶段）
- R_NO_ENDPOINT
- R_SIG_INVALID
- R_NAMESPACE_BLOCKED
- R_CACHE_STALE
- R_VERSION_OUT_OF_RANGE
- R_BUNDLE_MISMATCH

## 10. 推荐超时策略
- 单 endpoint 超时 ≤ 3s
- 总解析时间上限 ≤ 10s（含重试）
- 并行拉取镜像（优先主端点）

## 11. 安全强化建议
- Pin 指纹白名单（配置层）
- Rate Limit 对远程拉取
- 一致性：同 URN 短时内多次解析必须返回同指纹（否则告警）

解析与信任模型完成。