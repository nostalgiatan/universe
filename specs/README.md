# UNIV 规范文档

本目录包含 UNIV (Universe) 容器格式的正式规范文档。

## 规范状态

### 正式版本 (v1.0.0 Release)
- `UNIV-Container-Spec.zh-CN.md` - UNIV 容器规范正式版
- `UNIV-Type-Profile-Spec.zh-CN.md` - TYPE Profile 规范正式版
- `UNIV-Canonical-Encoding.zh-CN.md` - 规范化编码正式版
- `UNIV-UTI-IR.zh-CN.md` - UTI IR 正式版
- `UNIV-UTL-IDL.zh-CN.md` - UTL IDL 正式版
- `UNIV-Profile-Registry.zh-CN.md` - Profile 注册表正式版
- `UNIV-Resolution-and-Trust.zh-CN.md` - 解析与信任模型正式版

这些文档是当前实现的基准规范，状态为 **Stable**。

### 草案版本 (Draft)
`Draft/` 目录包含历史草案版本，仅供参考。当前实现不再基于草案，而是基于正式版本规范。

## 实现符合性

本 Rust 实现严格遵循上述 v1.0.0 正式版规范，确保完整的规范符合性和互操作性。