# UTL 文本 IDL 草案 v1.0-draft

## 1. 语法概览
```
namespace org.example
package payments
version 1.0.0

// 注释使用 //
record Invoice {
  uuid id
  timestamp issued_at
  decimal128(scale=2) amount
  enum Currency { USD, EUR, CNY }
  list<Line> lines
}

record Line {
  string? desc
  uint qty
  decimal128(scale=2) unit_price
}

constraint Invoice.amount range(min=0)
constraint Invoice.currency pattern("^[A-Z]{3}$")
```

## 2. 基本类型关键字
null,bool,int,uint,float32,float64,decimal128,bytes,string,timestamp,date,time,duration,uuid

## 3. 可选
`type? name` 或 在 record field 末尾加 `?`

## 4. 泛型
```
record Page<T> {
  uint total
  list<T> items
}
```

## 5. 引用
`ref urn:univ:org.example:payments:Invoice:1.0.0`

## 6. 约束语法
```
constraint <Type>.<field> range(min=0,max=100)
constraint <Type>.<field> pattern("regex")
constraint <Type> unique($.id,$.code)
constraint <Type> foreign_key(src=$.user_id, target=urn:univ:org.example:identity:User:1.0.0, tgt=$.id, on=restrict)
```

## 7. 枚举
`enum Currency { USD, EUR }`

## 8. 转换
工具：utl → UTI → 指纹  
反向：UTI → utl（注解丢失时补）
