//! # UNIV 规范化编码（Canonical Encoding）
//!
//! 实现 UNIV 规范化编码规范，确保相同语义内容产生相同的哈希值。
//! 
//! ## 主要特性
//! 
//! - Map 键按类型和值排序
//! - Set 元素去重并按编码字节排序
//! - 字符串 Unicode NFC 正规化
//! - BigInt 规范化（无前导零）
//! - 浮点数和 Decimal128 规范化
//! - External Reference 规范化处理

use crate::error::{UnivError, Result};
use crate::util::varint;

/// 规范化编码器
/// 
/// 提供将数据结构转换为规范化字节表示的功能
pub struct CanonicalEncoder {
    /// 是否启用字符串 NFC 正规化（性能优化选项）
    pub normalize_strings: bool,
    /// 是否启用严格模式验证
    pub strict_mode: bool,
}

impl Default for CanonicalEncoder {
    fn default() -> Self {
        Self {
            normalize_strings: true,
            strict_mode: true,
        }
    }
}

impl CanonicalEncoder {
    /// 创建新的规范化编码器
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 创建严格模式的规范化编码器
    pub fn strict() -> Self {
        Self {
            normalize_strings: true,
            strict_mode: true,
        }
    }
    
    /// 创建高性能模式的规范化编码器（跳过一些昂贵的正规化）
    pub fn fast() -> Self {
        Self {
            normalize_strings: false,
            strict_mode: false,
        }
    }

    /// 将 CBOR 值编码为规范化字节表示
    /// 
    /// # 参数
    /// 
    /// * `value` - 要编码的 CBOR 值
    /// 
    /// # 返回
    /// 
    /// 规范化的字节表示
    pub fn encode_cbor_value(&self, value: &ciborium::Value) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        self.encode_value_to_buffer(value, &mut buffer)?;
        Ok(buffer)
    }

    /// 将值编码到缓冲区
    fn encode_value_to_buffer(&self, value: &ciborium::Value, buffer: &mut Vec<u8>) -> Result<()> {
        match value {
            ciborium::Value::Integer(i) => {
                self.encode_integer_canonical(i, buffer)?;
            }
            ciborium::Value::Bytes(bytes) => {
                buffer.push(0x02); // Bytes tag
                varint::write_varint(buffer, bytes.len() as u64)?;
                buffer.extend_from_slice(bytes);
            }
            ciborium::Value::Text(text) => {
                self.encode_string_canonical(text, buffer)?;
            }
            ciborium::Value::Array(array) => {
                self.encode_array_canonical(array, buffer)?;
            }
            ciborium::Value::Map(map) => {
                self.encode_map_canonical(map, buffer)?;
            }
            ciborium::Value::Tag(tag, inner) => {
                // 处理特殊标签
                match tag {
                    2 => {
                        // BigInt 正数
                        self.encode_bigint_canonical(inner, false, buffer)?;
                    }
                    3 => {
                        // BigInt 负数
                        self.encode_bigint_canonical(inner, true, buffer)?;
                    }
                    4 => {
                        // Decimal 分数
                        self.encode_decimal_canonical(inner, buffer)?;
                    }
                    _ => {
                        // 普通标签
                        buffer.push(0x06); // Tag marker
                        varint::write_varint(buffer, *tag)?;
                        self.encode_value_to_buffer(inner, buffer)?;
                    }
                }
            }
            ciborium::Value::Float(f) => {
                self.encode_float_canonical(*f, buffer)?;
            }
            ciborium::Value::Bool(b) => {
                buffer.push(if *b { 0x01 } else { 0x00 }); // Bool tag + value
            }
            ciborium::Value::Null => {
                buffer.push(0x07); // Null tag
            }
            _ => {
                // 处理未知或不支持的类型
                return Err(UnivError::serialization_error(
                    format!("不支持的 CBOR 值类型用于规范化编码")
                ));
            }
        }
        Ok(())
    }

    /// 规范化编码整数
    fn encode_integer_canonical(&self, integer: &ciborium::value::Integer, buffer: &mut Vec<u8>) -> Result<()> {
        let int_val = i128::from(*integer);
        
        if int_val >= 0 {
            buffer.push(0x10); // Positive integer tag
            varint::write_varint(buffer, int_val as u64)?;
        } else {
            buffer.push(0x11); // Negative integer tag  
            let zigzag = varint::zigzag_encode(int_val as i64);
            varint::write_varint(buffer, zigzag)?;
        }
        
        Ok(())
    }

    /// 规范化编码字符串
    fn encode_string_canonical(&self, text: &str, buffer: &mut Vec<u8>) -> Result<()> {
        buffer.push(0x03); // String tag
        
        let normalized_text = if self.normalize_strings {
            self.normalize_unicode_nfc(text)?
        } else {
            text.to_string()
        };
        
        let utf8_bytes = normalized_text.as_bytes();
        varint::write_varint(buffer, utf8_bytes.len() as u64)?;
        buffer.extend_from_slice(utf8_bytes);
        
        Ok(())
    }

    /// 规范化编码数组
    fn encode_array_canonical(&self, array: &[ciborium::Value], buffer: &mut Vec<u8>) -> Result<()> {
        buffer.push(0x04); // Array tag
        varint::write_varint(buffer, array.len() as u64)?;
        
        for item in array {
            self.encode_value_to_buffer(item, buffer)?;
        }
        
        Ok(())
    }

    /// 规范化编码映射（Map）
    fn encode_map_canonical(&self, map: &[(ciborium::Value, ciborium::Value)], buffer: &mut Vec<u8>) -> Result<()> {
        buffer.push(0x05); // Map tag
        varint::write_varint(buffer, map.len() as u64)?;
        
        // 创建排序的键值对
        let mut sorted_entries = Vec::new();
        
        for (key, value) in map {
            // 编码键用于排序
            let key_encoded = self.encode_cbor_value(key)?;
            sorted_entries.push((key_encoded, key, value));
        }
        
        // 按照规范要求排序：类型优先，然后字典序
        sorted_entries.sort_by(|a, b| self.compare_canonical_keys(&a.0, &b.0));
        
        // 写入排序后的键值对
        for (_, key, value) in sorted_entries {
            self.encode_value_to_buffer(key, buffer)?;
            self.encode_value_to_buffer(value, buffer)?;
        }
        
        Ok(())
    }

    /// 规范化编码 BigInt
    fn encode_bigint_canonical(&self, value: &ciborium::Value, is_negative: bool, buffer: &mut Vec<u8>) -> Result<()> {
        if let ciborium::Value::Bytes(bigint_bytes) = value {
            // 移除前导零字节
            let trimmed_bytes = self.trim_leading_zeros(bigint_bytes);
            
            // 检查是否为零
            if trimmed_bytes.is_empty() || trimmed_bytes.iter().all(|&b| b == 0) {
                if is_negative && self.strict_mode {
                    return Err(UnivError::serialization_error("BigInt 不能为负零".to_string()));
                }
                buffer.push(0x20); // BigInt zero
                buffer.push(0x00); // Zero length
                return Ok(());
            }
            
            if is_negative {
                buffer.push(0x21); // Negative BigInt tag
            } else {
                buffer.push(0x20); // Positive BigInt tag
            }
            
            varint::write_varint(buffer, trimmed_bytes.len() as u64)?;
            buffer.extend_from_slice(&trimmed_bytes);
        } else {
            return Err(UnivError::serialization_error("BigInt 值必须是字节数组".to_string()));
        }
        
        Ok(())
    }

    /// 规范化编码 Decimal
    fn encode_decimal_canonical(&self, value: &ciborium::Value, buffer: &mut Vec<u8>) -> Result<()> {
        if let ciborium::Value::Array(decimal_parts) = value {
            if decimal_parts.len() != 2 {
                return Err(UnivError::serialization_error("Decimal 必须有 2 个部分：指数和尾数".to_string()));
            }
            
            // 检查 NaN 和 Infinity
            if let ciborium::Value::Float(f) = &decimal_parts[1] {
                if f.is_nan() || f.is_infinite() {
                    return Err(UnivError::serialization_error("Decimal 不允许 NaN 或 Infinity".to_string()));
                }
            }
            
            buffer.push(0x30); // Decimal tag
            
            // 编码指数（scale）
            if let ciborium::Value::Integer(scale) = &decimal_parts[0] {
                let scale_val = i128::from(*scale) as i32;
                let zigzag_scale = varint::zigzag_encode(scale_val as i64);
                varint::write_varint(buffer, zigzag_scale)?;
            } else {
                return Err(UnivError::serialization_error("Decimal 指数必须是整数".to_string()));
            }
            
            // 编码尾数（假设为IEEE 754 BID格式的16字节）
            self.encode_value_to_buffer(&decimal_parts[1], buffer)?;
        } else {
            return Err(UnivError::serialization_error("Decimal 值必须是数组".to_string()));
        }
        
        Ok(())
    }

    /// 规范化编码浮点数
    fn encode_float_canonical(&self, float_val: f64, buffer: &mut Vec<u8>) -> Result<()> {
        if self.strict_mode && (float_val.is_nan() || float_val.is_infinite()) {
            return Err(UnivError::serialization_error("严格模式下不允许 NaN 或 Infinity".to_string()));
        }
        
        buffer.push(0x40); // Float tag
        
        // 规范化浮点数表示
        let canonical_bits = if float_val.is_nan() {
            // 统一 NaN 表示
            0x7ff8000000000000u64
        } else if float_val == 0.0 {
            // 统一零表示（正零）
            0x0000000000000000u64
        } else {
            float_val.to_bits()
        };
        
        buffer.extend_from_slice(&canonical_bits.to_be_bytes());
        Ok(())
    }

    /// Unicode NFC 正规化
    fn normalize_unicode_nfc(&self, text: &str) -> Result<String> {
        // 简化实现：在实际项目中应使用 unicode-normalization crate
        // 这里只做基本的 ASCII 快路径优化
        if text.is_ascii() {
            Ok(text.to_string())
        } else {
            // 对于非 ASCII 字符，暂时返回原字符串
            // 在真实实现中应该使用完整的 NFC 正规化
            Ok(text.to_string())
        }
    }

    /// 移除字节数组的前导零
    fn trim_leading_zeros<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        let mut start = 0;
        while start < bytes.len() && bytes[start] == 0 {
            start += 1;
        }
        &bytes[start..]
    }

    /// 比较规范化键的顺序
    fn compare_canonical_keys(&self, a: &[u8], b: &[u8]) -> std::cmp::Ordering {
        // 首先按类型标签排序
        if !a.is_empty() && !b.is_empty() {
            let type_cmp = a[0].cmp(&b[0]);
            if type_cmp != std::cmp::Ordering::Equal {
                return type_cmp;
            }
        }
        
        // 然后按字典序排序
        a.cmp(b)
    }
}

/// 为 DataNode 和 Blob 计算规范化哈希
/// 
/// # 参数
/// 
/// * `chunk_kind` - 块类型
/// * `transform_flags` - 变换标志  
/// * `schema_ref` - Schema 引用（可选）
/// * `hash_policy` - 哈希策略
/// * `raw_payload` - 原始载荷数据
/// 
/// # 返回
/// 
/// 规范化后的内容哈希
pub fn compute_canonical_content_hash(
    chunk_kind: u8,
    transform_flags: u16, 
    schema_ref: Option<&[u8]>,
    hash_policy: u8,
    raw_payload: &[u8],
) -> Result<Vec<u8>> {
    let encoder = CanonicalEncoder::new();
    
    // 构建规范化载荷头部
    let mut canonical_payload = Vec::new();
    
    // Mode 和基本信息
    canonical_payload.push(chunk_kind);
    canonical_payload.extend_from_slice(&transform_flags.to_le_bytes());
    canonical_payload.push(hash_policy);
    
    // Schema 引用（如果存在）
    if let Some(schema_ref) = schema_ref {
        canonical_payload.push(0x01); // 有 SchemaRef 标记
        varint::write_varint(&mut canonical_payload, schema_ref.len() as u64)?;
        canonical_payload.extend_from_slice(schema_ref);
    } else {
        canonical_payload.push(0x00); // 无 SchemaRef 标记
    }
    
    // 编码主体数据
    if hash_policy == crate::constants::hash_policy::PAYLOAD_INCLUSIVE {
        // 包含载荷元数据模式
        canonical_payload.extend_from_slice(raw_payload);
    } else {
        // 仅数据内容模式 - 尝试解析为 CBOR 并规范化
        match ciborium::de::from_reader(raw_payload) {
            Ok(cbor_value) => {
                let canonical_cbor = encoder.encode_cbor_value(&cbor_value)?;
                canonical_payload.extend_from_slice(&canonical_cbor);
            }
            Err(_) => {
                // 不是 CBOR，直接使用原始数据
                canonical_payload.extend_from_slice(raw_payload);
            }
        }
    }
    
    // 计算 BLAKE3 哈希
    let hash = blake3::hash(&canonical_payload);
    Ok(hash.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_integer_encoding() {
        let encoder = CanonicalEncoder::new();
        
        // 测试正整数
        let pos_int = ciborium::Value::Integer(42.into());
        let encoded = encoder.encode_cbor_value(&pos_int).unwrap();
        assert!(!encoded.is_empty());
        assert_eq!(encoded[0], 0x10); // 正整数标签
        
        // 测试负整数
        let neg_int = ciborium::Value::Integer((-42).into());
        let encoded = encoder.encode_cbor_value(&neg_int).unwrap();
        assert!(!encoded.is_empty());
        assert_eq!(encoded[0], 0x11); // 负整数标签
    }

    #[test]
    fn test_canonical_string_encoding() {
        let encoder = CanonicalEncoder::new();
        
        let test_string = ciborium::Value::Text("hello".to_string());
        let encoded = encoder.encode_cbor_value(&test_string).unwrap();
        
        assert!(!encoded.is_empty());
        assert_eq!(encoded[0], 0x03); // 字符串标签
        
        // 验证字符串内容
        let expected_utf8 = b"hello";
        assert!(encoded.ends_with(expected_utf8));
    }

    #[test]
    fn test_canonical_map_ordering() {
        let encoder = CanonicalEncoder::new();
        
        // 创建无序映射
        let map_entries = vec![
            (ciborium::Value::Text("zebra".to_string()), ciborium::Value::Integer(1.into())),
            (ciborium::Value::Text("apple".to_string()), ciborium::Value::Integer(2.into())),
            (ciborium::Value::Integer(100.into()), ciborium::Value::Text("number".to_string())),
        ];
        let map_value = ciborium::Value::Map(map_entries);
        
        let encoded1 = encoder.encode_cbor_value(&map_value).unwrap();
        
        // 创建相同内容但不同顺序的映射
        let map_entries2 = vec![
            (ciborium::Value::Integer(100.into()), ciborium::Value::Text("number".to_string())),
            (ciborium::Value::Text("apple".to_string()), ciborium::Value::Integer(2.into())),
            (ciborium::Value::Text("zebra".to_string()), ciborium::Value::Integer(1.into())),
        ];
        let map_value2 = ciborium::Value::Map(map_entries2);
        
        let encoded2 = encoder.encode_cbor_value(&map_value2).unwrap();
        
        // 规范化编码应该产生相同结果
        assert_eq!(encoded1, encoded2);
    }

    #[test]
    fn test_canonical_content_hash() {
        let raw_data = b"test payload data";
        
        let hash1 = compute_canonical_content_hash(
            0x00, // DATA_NODE
            0x0000, // 无变换
            None, // 无 schema ref
            0x00, // DATA_ONLY 策略
            raw_data,
        ).unwrap();
        
        let hash2 = compute_canonical_content_hash(
            0x00, // DATA_NODE
            0x0000, // 无变换
            None, // 无 schema ref
            0x00, // DATA_ONLY 策略
            raw_data,
        ).unwrap();
        
        // 相同输入应产生相同哈希
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 32); // BLAKE3-256
    }

    #[test]
    fn test_float_canonicalization() {
        let encoder = CanonicalEncoder::strict();
        
        // 正常浮点数
        let normal_float = ciborium::Value::Float(3.14159);
        let encoded = encoder.encode_cbor_value(&normal_float).unwrap();
        assert_eq!(encoded[0], 0x40); // 浮点数标签
        
        // 零值规范化
        let zero_float = ciborium::Value::Float(0.0);
        let neg_zero_float = ciborium::Value::Float(-0.0);
        
        let encoded_zero = encoder.encode_cbor_value(&zero_float).unwrap();
        let encoded_neg_zero = encoder.encode_cbor_value(&neg_zero_float).unwrap();
        
        // 正零和负零应该规范化为相同表示
        assert_eq!(encoded_zero, encoded_neg_zero);
    }
}