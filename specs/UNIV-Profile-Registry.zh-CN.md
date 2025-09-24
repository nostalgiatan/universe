# UNIV Profile 注册表
版本：1.0.0 Release  
状态：Stable 基线

| Code | 描述 | 稳定级别 | 适用场景 | 备注 |
|------|------|----------|----------|------|
| BLOB | 大对象/媒体 | stable | 文件、模型、视频片段 | CDC + range-map |
| RECD | 结构化记录 | stable | KV/文档/对象集 | 可选列式化 |
| TABL | 分析列式表 | stable | 批处理 / 数据湖 | 行组 + 列统计 |
| TSDB | 时间序列 | stable | Metrics / 时序日志 | Delta + Gorilla |
| GRPH | DAG/图数据 | beta | 共享子结构/知识图 | external ref |
| MIXD | 混合通用 | legacy | 过渡 / 实验 | 不建议生产 |
| TYPE | 类型仓库 | stable | Schema 发布 | 指纹+签名 |
| X*** | 自定义 | experimental | 私有优化 | 首字母 X |

## 1. 稳定级别语义
- stable：保证向后兼容的小版本演进
- beta：可能调整结构；需 --allow-beta
- legacy：计划弃用；提供迁移指引
- experimental：不保证兼容

## 2. 注册流程（建议）
提交扩展文档包含：
- ProfileCode
- 允许 ChunkKind
- 变换策略（允许/禁止/建议）
- 安全限幅差异
- 特定索引结构
- 退出策略（若失败回退至 MIXD 行为）

## 3. 互操作测试要求
新 Profile 需提供：
- 至少 5 个 test vectors（含一个损坏案例）
- 压缩效率对比（与 RECD/TABL 基线）
- 指纹稳定性证明（多实现一致性）

## 4. 弃用流程
1) 标记 legacy  
2) 发布迁移工具  
3) 至少 2 个 Minor 周期后移除（Major 变更）

Profile 注册表完成。