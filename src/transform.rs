//! # 数据变换系统
//!
//! 提供 UNIV 格式支持的各种数据变换，如字典压缩、列式化、Delta编码等。

use crate::error::{UnivError, Result};
use crate::constants::transform_flags;
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 数据变换器
pub struct DataTransformer {
    /// 变换标志
    pub flags: u16,
}

impl DataTransformer {
    /// 创建新的数据变换器
    /// 
    /// # 参数
    /// 
    /// * `flags` - 变换标志
    pub fn new(flags: u16) -> Self {
        Self { flags }
    }

    /// 应用变换到数据
    /// 
    /// # 参数
    /// 
    /// * `data` - 原始数据
    /// 
    /// # 返回
    /// 
    /// 变换后的数据
    pub fn apply(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut result = data.to_vec();
        
        // 按顺序应用变换
        if self.flags & transform_flags::DICT_STRING != 0 {
            result = self.apply_dict_string(&result)?;
        }
        
        if self.flags & transform_flags::INTEGER_VARINT != 0 {
            result = self.apply_integer_varint(&result)?;
        }
        
        if self.flags & transform_flags::DELTA != 0 {
            result = self.apply_delta(&result)?;
        }
        
        // 其他变换...
        
        Ok(result)
    }

    /// 逆向应用变换
    /// 
    /// # 参数
    /// 
    /// * `data` - 变换后的数据
    /// 
    /// # 返回
    /// 
    /// 恢复的原始数据
    pub fn reverse(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut result = data.to_vec();
        
        // 按逆序逆向变换
        if self.flags & transform_flags::DELTA != 0 {
            result = self.reverse_delta(&result)?;
        }
        
        if self.flags & transform_flags::INTEGER_VARINT != 0 {
            result = self.reverse_integer_varint(&result)?;
        }
        
        if self.flags & transform_flags::DICT_STRING != 0 {
            result = self.reverse_dict_string(&result)?;
        }
        
        Ok(result)
    }

    // 字典-字符串变换
    fn apply_dict_string(&self, data: &[u8]) -> Result<Vec<u8>> {
        // 简化实现：实际应该构建字符串字典并替换重复字符串
        Ok(data.to_vec())
    }

    fn reverse_dict_string(&self, data: &[u8]) -> Result<Vec<u8>> {
        // 简化实现：实际应该使用字典还原字符串
        Ok(data.to_vec())
    }

    // 整数可变长编码变换
    fn apply_integer_varint(&self, data: &[u8]) -> Result<Vec<u8>> {
        // 简化实现：实际应该识别整数字段并应用可变长编码
        Ok(data.to_vec())
    }

    fn reverse_integer_varint(&self, data: &[u8]) -> Result<Vec<u8>> {
        // 简化实现：实际应该解码可变长整数
        Ok(data.to_vec())
    }

    // Delta编码变换
    fn apply_delta(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 8 {
            return Ok(data.to_vec());
        }
        
        // 简化实现：假设数据是8字节整数数组，应用Delta编码
        let mut result = BytesMut::new();
        let mut chunks = data.chunks_exact(8);
        
        if let Some(first) = chunks.next() {
            result.extend_from_slice(first); // 第一个值保持不变
            let mut prev = u64::from_le_bytes(first.try_into().unwrap());
            
            for chunk in chunks {
                let current = u64::from_le_bytes(chunk.try_into().unwrap());
                let delta = current.wrapping_sub(prev);
                result.extend_from_slice(&delta.to_le_bytes());
                prev = current;
            }
        }
        
        // 处理剩余字节
        let remainder = data.len() % 8;
        if remainder > 0 {
            result.extend_from_slice(&data[data.len() - remainder..]);
        }
        
        Ok(result.to_vec())
    }

    fn reverse_delta(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 8 {
            return Ok(data.to_vec());
        }
        
        // Delta解码的逆过程
        let mut result = BytesMut::new();
        let mut chunks = data.chunks_exact(8);
        
        if let Some(first) = chunks.next() {
            result.extend_from_slice(first); // 第一个值保持不变
            let mut prev = u64::from_le_bytes(first.try_into().unwrap());
            
            for chunk in chunks {
                let delta = u64::from_le_bytes(chunk.try_into().unwrap());
                let current = prev.wrapping_add(delta);
                result.extend_from_slice(&current.to_le_bytes());
                prev = current;
            }
        }
        
        // 处理剩余字节
        let remainder = data.len() % 8;
        if remainder > 0 {
            result.extend_from_slice(&data[data.len() - remainder..]);
        }
        
        Ok(result.to_vec())
    }
}

/// 字符串字典
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringDictionary {
    /// 字符串到索引的映射
    string_to_index: HashMap<String, u32>,
    /// 索引到字符串的映射
    index_to_string: Vec<String>,
}

impl StringDictionary {
    /// 创建新的字符串字典
    pub fn new() -> Self {
        Self {
            string_to_index: HashMap::new(),
            index_to_string: Vec::new(),
        }
    }

    /// 添加字符串到字典
    /// 
    /// # 参数
    /// 
    /// * `s` - 要添加的字符串
    /// 
    /// # 返回
    /// 
    /// 字符串的索引
    pub fn add_string(&mut self, s: String) -> u32 {
        if let Some(&index) = self.string_to_index.get(&s) {
            return index;
        }
        
        let index = self.index_to_string.len() as u32;
        self.string_to_index.insert(s.clone(), index);
        self.index_to_string.push(s);
        index
    }

    /// 根据索引获取字符串
    /// 
    /// # 参数
    /// 
    /// * `index` - 字符串索引
    /// 
    /// # 返回
    /// 
    /// 字符串的引用
    pub fn get_string(&self, index: u32) -> Option<&str> {
        self.index_to_string.get(index as usize).map(|s| s.as_str())
    }

    /// 根据字符串获取索引
    /// 
    /// # 参数
    /// 
    /// * `s` - 字符串
    /// 
    /// # 返回
    /// 
    /// 字符串的索引
    pub fn get_index(&self, s: &str) -> Option<u32> {
        self.string_to_index.get(s).copied()
    }

    /// 获取字典大小
    pub fn len(&self) -> usize {
        self.index_to_string.len()
    }

    /// 检查字典是否为空
    pub fn is_empty(&self) -> bool {
        self.index_to_string.is_empty()
    }

    /// 序列化字典
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::ser::into_writer(self, &mut buf)
            .map_err(|e| UnivError::serialization_error(format!("字符串字典序列化失败: {}", e)))?;
        Ok(buf)
    }

    /// 反序列化字典
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        ciborium::de::from_reader(data)
            .map_err(|e| UnivError::deserialization_error(format!("字符串字典反序列化失败: {}", e)))
    }
}

impl Default for StringDictionary {
    fn default() -> Self {
        Self::new()
    }
}

/// 列式数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnarData {
    /// 列名列表
    pub column_names: Vec<String>,
    /// 列数据
    pub columns: Vec<ColumnData>,
    /// 行数
    pub row_count: usize,
}

/// 单列数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnData {
    /// 整数列
    Integer(Vec<i64>),
    /// 浮点数列
    Float(Vec<f64>),
    /// 字符串列
    String(Vec<String>),
    /// 布尔列
    Boolean(Vec<bool>),
    /// 字节列
    Bytes(Vec<Vec<u8>>),
    /// 空值掩码列
    Nulls(Vec<bool>),
}

impl ColumnarData {
    /// 创建新的列式数据
    pub fn new() -> Self {
        Self {
            column_names: Vec::new(),
            columns: Vec::new(),
            row_count: 0,
        }
    }

    /// 添加列
    /// 
    /// # 参数
    /// 
    /// * `name` - 列名
    /// * `data` - 列数据
    pub fn add_column(&mut self, name: String, data: ColumnData) -> Result<()> {
        let column_row_count = match &data {
            ColumnData::Integer(v) => v.len(),
            ColumnData::Float(v) => v.len(),
            ColumnData::String(v) => v.len(),
            ColumnData::Boolean(v) => v.len(),
            ColumnData::Bytes(v) => v.len(),
            ColumnData::Nulls(v) => v.len(),
        };

        if self.row_count == 0 {
            self.row_count = column_row_count;
        } else if self.row_count != column_row_count {
            return Err(UnivError::InvalidTransform {
                reason: format!("列行数不匹配: 期望 {}, 实际 {}", self.row_count, column_row_count),
            });
        }

        self.column_names.push(name);
        self.columns.push(data);
        Ok(())
    }

    /// 获取列数
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// 获取行数
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// 序列化列式数据
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::ser::into_writer(self, &mut buf)
            .map_err(|e| UnivError::serialization_error(format!("列式数据序列化失败: {}", e)))?;
        Ok(buf)
    }

    /// 反序列化列式数据
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        ciborium::de::from_reader(data)
            .map_err(|e| UnivError::deserialization_error(format!("列式数据反序列化失败: {}", e)))
    }
}

impl Default for ColumnarData {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_transformer() {
        let transformer = DataTransformer::new(0); // 无变换
        let data = b"Hello, World!";
        
        let transformed = transformer.apply(data).unwrap();
        let restored = transformer.reverse(&transformed).unwrap();
        
        assert_eq!(data, restored.as_slice());
    }

    #[test]
    fn test_delta_transform() {
        let transformer = DataTransformer::new(transform_flags::DELTA);
        
        // 测试数据：三个8字节整数
        let data = [
            1u64.to_le_bytes(),
            3u64.to_le_bytes(),
            6u64.to_le_bytes(),
        ].concat();
        
        let transformed = transformer.apply(&data).unwrap();
        let restored = transformer.reverse(&transformed).unwrap();
        
        assert_eq!(data, restored);
    }

    #[test]
    fn test_string_dictionary() {
        let mut dict = StringDictionary::new();
        
        let index1 = dict.add_string("hello".to_string());
        let index2 = dict.add_string("world".to_string());
        let index3 = dict.add_string("hello".to_string()); // 重复
        
        assert_eq!(index1, 0);
        assert_eq!(index2, 1);
        assert_eq!(index3, 0); // 应该返回相同的索引
        
        assert_eq!(dict.get_string(0), Some("hello"));
        assert_eq!(dict.get_string(1), Some("world"));
        assert_eq!(dict.get_index("hello"), Some(0));
        assert_eq!(dict.get_index("world"), Some(1));
        
        assert_eq!(dict.len(), 2);
    }

    #[test]
    fn test_columnar_data() {
        let mut columnar = ColumnarData::new();
        
        let int_col = ColumnData::Integer(vec![1, 2, 3]);
        let str_col = ColumnData::String(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        
        assert!(columnar.add_column("id".to_string(), int_col).is_ok());
        assert!(columnar.add_column("name".to_string(), str_col).is_ok());
        
        assert_eq!(columnar.column_count(), 2);
        assert_eq!(columnar.row_count(), 3);
        
        // 测试行数不匹配的情况
        let bad_col = ColumnData::Integer(vec![1, 2]); // 只有2行
        assert!(columnar.add_column("bad".to_string(), bad_col).is_err());
    }

    #[test]
    fn test_dictionary_serialization() {
        let mut dict = StringDictionary::new();
        dict.add_string("test1".to_string());
        dict.add_string("test2".to_string());
        
        let serialized = dict.serialize().unwrap();
        let deserialized = StringDictionary::deserialize(&serialized).unwrap();
        
        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized.get_string(0), Some("test1"));
        assert_eq!(deserialized.get_string(1), Some("test2"));
    }
}