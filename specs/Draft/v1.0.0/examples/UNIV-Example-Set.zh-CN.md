# 示例集合

## 1. TYPE 包 (payments)
- 文件魔数：UNV1 TYPE
- 包含：Schema(Invoice,Line)、Manifest、StringTable、签名
- 指纹：F1 (示例)

## 2. RECD 数据文件
- 魔数：UNV1 RECD
- HeaderExt: NamespaceRoot=org.example
- DataNode: Mode=SG, SchemaRef=F1
- RootSet: [{name:"main", node_id:F_DATA}]
- 使用 Dict-String + Integer-Varint
- TOC: nodes({F_DATA}), chunks, roots

## 3. TABL 表文件
- 魔数：UNV1 TABL
- 行组 128K；列块压缩 zstd
- 列统计：min/max/null_count

## 4. TSDB 时间序列
- 魔数：UNV1 TSDB
- HeaderExt: WindowSizeSeconds=60
- Series 索引：series_hash + windows[]
