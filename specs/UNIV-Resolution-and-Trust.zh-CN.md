# UNIV 解析与信任模型 v1.0-draft

## 1. Schema 解析优先级
内嵌 > 缓存 > 远程 endpoints (ResolverHints.endpoints 顺序) > fallback

## 2. ResolverHints.trust
```
"trust": {
  "mode":"pinned"|"ca"|"web-of-trust",
  "keys":["base64pub"...],
  "requireSignature": true
}
```
- pinned：所有 manifest 签名公钥须在 keys 中
- ca：允许使用内置根（外部策略）
- web-of-trust：可链式验证（后续扩展）

## 3. 签名覆盖
Manifest 中 exports + dependencies 序列化后 canonical CBOR → 签名。

## 4. 缓存键
- URN+resolved_version → fingerprint
- fingerprint → TYPE 包字节
签名失败立即丢弃缓存条目。

## 5. 外部 Ref 安全
external=true 的 Ref 不自动解析；需要显式策略允许；解析后仍需指纹验证。

## 6. 重放与降级防护
保留 `manifest.timestamp`（ns）与 `not_before`/`not_after`（可选），解析器可拒绝过期或倒退版本（若 policy 指示）。

## 7. 审计日志建议
记录：时间、请求 URN、解析来源(endpoint/cache/embedded)、fingerprint、签名校验结果。
