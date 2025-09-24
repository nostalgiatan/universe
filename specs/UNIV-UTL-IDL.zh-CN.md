# UTL 文本 IDL 规范
版本：1.0.0 Release  
状态：Stable

## 1. 目的
提供人类可读的类型定义语法 → 可无损编译为 UTI → 指纹稳定。  
指纹总以 UTI Canonical CBOR 结果计算，不直接哈希 UTL 文本。

## 2. 基本结构
```
namespace org.example
package payments
version 1.0.0

// 单行注释 //
// 块注释使用 /* ... */

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

// 约束
constraint Invoice.amount range(min=0)
constraint Invoice.currency pattern("^[A-Z]{3}$")
```

## 3. 基本类型
null bool int uint bigint float32 float64 decimal128 bytes string timestamp date time duration uuid

## 4. 可选字段
`type? name` 或 `optional` 修饰：`optional string name`

## 5. 泛型
```
record Page<T> {
  uint total
  list<T> items
}
```
约束：
```
record Box<T:record> {
  T value
}
```

## 6. 枚举
`enum Currency { USD, EUR, CNY }`

## 7. 联合
`union Shape = Circle | Rect | Triangle`

## 8. 引用
`ref urn:univ:org.example:payments:Invoice:1.0.0`

## 9. decimal128
`decimal128(scale=2)`；scale 必须整数

## 10. 约束语法
```
constraint <Type>.<field> range(min=0,max=100)
constraint <Type>.<field> pattern("^[A-Z]+$")
constraint <Type> unique($.id,$.code)
constraint <Type> foreign_key(
  src=$.user_id,
  target=urn:univ:org.example:identity:User:1.0.0,
  tgt=$.id,
  on=restrict
)
```

## 11. 注解
语法：
```
@doc("Invoice main entity")
@codegen.java.class("org.example.Invoice")
record Invoice { ... }
```
字段注解同理。多个注解按出现顺序收集（顺序不影响指纹）。

## 12. EBNF
```
File          = Header* Definition* Constraint* ;
Header        = Namespace | Package | Version ;
Namespace     = "namespace" Identifier ("." Identifier)* ;
Package       = "package" Identifier ;
Version       = "version" SemVer ;

Definition    = RecordDef | EnumDef | UnionDef | GenericRecordDef ;
RecordDef     = Annotations? "record" Identifier TypeParams? "{" Field* "}" ;
Field         = Annotations? TypeRef Identifier Optional? ;
Optional      = "?" ;
EnumDef       = Annotations? "enum" Identifier "{" EnumMemberList "}" ;
EnumMemberList= Identifier ("," Identifier)* ;
UnionDef      = Annotations? "union" Identifier "=" TypeRef ("|" TypeRef)* ;

TypeParams    = "<" TypeParam ("," TypeParam)* ">" ;
TypeParam     = Identifier (":" ConstraintList)? ;
ConstraintList= Identifier ("," Identifier)* ;

TypeRef       = SimpleType | ComplexType ;
SimpleType    = Identifier ;
ComplexType   = ListType | SetType | MapType | RefType | DecimalType ;
ListType      = "list" "<" TypeRef ">" ;
SetType       = "set" "<" TypeRef ">" ;
MapType       = "map" "<" TypeRef "," TypeRef ">" ;
RefType       = "ref" URN ;
DecimalType   = "decimal128" "(" "scale" "=" Integer ")" ;

Annotations   = Annotation+ ;
Annotation    = "@" Identifier "(" StringLiteral ")" ;

Constraint    = "constraint" ConstraintExpr ;
ConstraintExpr= UniqueConstraint | RangeConstraint | PatternConstraint | ForeignKeyConstraint ;
UniqueConstraint = TypeName "." FieldName "unique" "(" JsonPathList ")" ;
RangeConstraint  = TypeName "." FieldName "range" "(" RangeParams ")" ;
PatternConstraint= TypeName "." FieldName "pattern" "(" StringLiteral ")" ;
ForeignKeyConstraint = TypeName "." FieldName "foreign_key" "(" FKParams ")" ;

JsonPathList  = JsonPath ("," JsonPath)* ;
JsonPath      = "$" ( "." Identifier | "[" Integer "]" )* ;

SemVer        = Integer "." Integer "." Integer ;
Identifier    = /[A-Za-z_][A-Za-z0-9_]*/ ;
Integer       = /[0-9]+/ ;
StringLiteral = '"' ( ~["\\] | Escape )* '"' ;
Escape        = "\\" ["\\/bfnrt] ;
URN           = "urn:univ:" URNBody ;
URNBody       = /[A-Za-z0-9:._-]+/ ;

TypeName      = Identifier ;
FieldName     = Identifier ;
RangeParams   = ( "min" "=" Number ("," "max" "=" Number )? ) |
                ( "max" "=" Number ) ;
Number        = /-?[0-9]+(\.[0-9]+)?/ ;
FKParams      = "src=" JsonPath "," "target=" URN "," "tgt=" JsonPath "," "on=" ("restrict"|"cascade"|"nullify") ;
```

## 13. 错误类别
U_PARSE_ERROR / U_DUP_TYPE / U_DUP_FIELD / U_ENUM_DUP / U_RANGE_PARAM / U_FK_PARAM / U_GENERIC_CONSTRAINT_FAIL

## 14. Round-trip 保证
UTL → UTI → 再生成 UTL 时：
- 注解顺序可能重排
- 格式化差异不影响指纹
- 空结构 / 默认值保持语义

UTL 规范完成。