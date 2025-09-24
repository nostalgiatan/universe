# UNIV Profile 注册表 v1.0-draft

| Code | 描述 | 稳定状态 | 备注 |
|------|------|----------|------|
| BLOB | 大对象/媒体 | stable | hash_policy=data-only |
| RECD | 结构化记录 | stable | 可选列式化 |
| TABL | 列式表 | stable | 强制列式 |
| TSDB | 时间序列 | stable | 时间窗口+Gorilla |
| GRPH | 图/DAG | beta | DAG + external ref |
| MIXD | 混合 | legacy | 尽量避免生产 |
| TYPE | 类型仓库 | stable | Schema 分发 |
| X*** | 自定义 | experimental | 首字母 X |

自定义注册建议：提交扩展文档并声明 Transform 策略与安全预算。
,