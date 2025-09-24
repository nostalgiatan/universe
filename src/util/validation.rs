//! # 验证工具
//!
//! 提供 UNIV 格式的各种验证功能和安全检查。

use crate::constants::security_limits;
use crate::error::{UnivError, Result};
use std::collections::HashSet;

/// 安全限制验证器
#[derive(Debug, Clone)]
pub struct SecurityValidator {
    /// 最大块数量
    pub max_chunks: u32,
    /// 最大原始数据大小
    pub max_raw_size: u64,
    /// 最大单个块原始大小
    pub max_chunk_raw: u32,
    /// 最大引用深度
    pub max_ref_depth: u32,
    /// 最大字符串表大小
    pub max_string_table: u32,
    /// 压缩膨胀率阈值
    pub compression_ratio_threshold: f32,
}

impl SecurityValidator {
    /// 创建默认的安全验证器
    pub fn new() -> Self {
        Self {
            max_chunks: security_limits::MAX_CHUNKS,
            max_raw_size: security_limits::MAX_RAW_SIZE,
            max_chunk_raw: security_limits::MAX_CHUNK_RAW,
            max_ref_depth: security_limits::MAX_REF_DEPTH,
            max_string_table: security_limits::MAX_STRING_TABLE,
            compression_ratio_threshold: security_limits::COMPRESSION_RATIO_THRESHOLD,
        }
    }

    /// 创建自定义的安全验证器
    /// 
    /// # 参数
    /// 
    /// * `max_chunks` - 最大块数量
    /// * `max_raw_size` - 最大原始数据大小
    /// * `max_chunk_raw` - 最大单个块原始大小
    /// * `max_ref_depth` - 最大引用深度
    /// * `max_string_table` - 最大字符串表大小
    /// * `compression_ratio_threshold` - 压缩膨胀率阈值
    pub fn custom(
        max_chunks: u32,
        max_raw_size: u64,
        max_chunk_raw: u32,
        max_ref_depth: u32,
        max_string_table: u32,
        compression_ratio_threshold: f32,
    ) -> Self {
        Self {
            max_chunks,
            max_raw_size,
            max_chunk_raw,
            max_ref_depth,
            max_string_table,
            compression_ratio_threshold,
        }
    }

    /// 验证块数量
    /// 
    /// # 参数
    /// 
    /// * `chunk_count` - 块数量
    /// 
    /// # 返回
    /// 
    /// 如果验证通过返回Ok，否则返回错误
    pub fn validate_chunk_count(&self, chunk_count: u32) -> Result<()> {
        if chunk_count > self.max_chunks {
            return Err(UnivError::security_limit_exceeded(
                "chunk_count",
                self.max_chunks as u64,
            ));
        }
        Ok(())
    }

    /// 验证总数据大小
    /// 
    /// # 参数
    /// 
    /// * `total_size` - 总数据大小
    /// 
    /// # 返回
    /// 
    /// 如果验证通过返回Ok，否则返回错误
    pub fn validate_total_size(&self, total_size: u64) -> Result<()> {
        if total_size > self.max_raw_size {
            return Err(UnivError::security_limit_exceeded(
                "total_raw_size",
                self.max_raw_size,
            ));
        }
        Ok(())
    }

    /// 验证单个块大小
    /// 
    /// # 参数
    /// 
    /// * `chunk_size` - 块大小
    /// 
    /// # 返回
    /// 
    /// 如果验证通过返回Ok，否则返回错误
    pub fn validate_chunk_size(&self, chunk_size: u32) -> Result<()> {
        if chunk_size > self.max_chunk_raw {
            return Err(UnivError::security_limit_exceeded(
                "chunk_raw_size",
                self.max_chunk_raw as u64,
            ));
        }
        Ok(())
    }

    /// 验证引用深度
    /// 
    /// # 参数
    /// 
    /// * `depth` - 引用深度
    /// 
    /// # 返回
    /// 
    /// 如果验证通过返回Ok，否则返回错误
    pub fn validate_reference_depth(&self, depth: u32) -> Result<()> {
        if depth > self.max_ref_depth {
            return Err(UnivError::ReferenceDepthExceeded {
                current: depth,
                max: self.max_ref_depth,
            });
        }
        Ok(())
    }

    /// 验证字符串表大小
    /// 
    /// # 参数
    /// 
    /// * `string_table_size` - 字符串表大小
    /// 
    /// # 返回
    /// 
    /// 如果验证通过返回Ok，否则返回错误
    pub fn validate_string_table_size(&self, string_table_size: u32) -> Result<()> {
        if string_table_size > self.max_string_table {
            return Err(UnivError::security_limit_exceeded(
                "string_table_size",
                self.max_string_table as u64,
            ));
        }
        Ok(())
    }

    /// 验证压缩比
    /// 
    /// # 参数
    /// 
    /// * `raw_size` - 原始大小
    /// * `compressed_size` - 压缩后大小
    /// 
    /// # 返回
    /// 
    /// 如果验证通过返回Ok，否则返回错误或警告
    pub fn validate_compression_ratio(&self, raw_size: u32, compressed_size: u32) -> Result<()> {
        if compressed_size == 0 {
            return Ok(()); // 避免除零
        }

        let ratio = compressed_size as f32 / raw_size as f32;
        if ratio >= self.compression_ratio_threshold {
            return Err(UnivError::compression_error(format!(
                "压缩膨胀率过高: {:.2}, 阈值: {:.2}",
                ratio,
                self.compression_ratio_threshold
            )));
        }

        Ok(())
    }
}

impl Default for SecurityValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// 引用循环检测器
#[derive(Debug, Clone)]
pub struct CycleDetector {
    /// 已访问的节点集合
    visited: HashSet<String>,
    /// 当前路径中的节点
    path: Vec<String>,
}

impl CycleDetector {
    /// 创建新的循环检测器
    pub fn new() -> Self {
        Self {
            visited: HashSet::new(),
            path: Vec::new(),
        }
    }

    /// 开始访问一个节点
    /// 
    /// # 参数
    /// 
    /// * `node_id` - 节点标识符
    /// 
    /// # 返回
    /// 
    /// 如果没有循环返回Ok，否则返回错误
    pub fn visit(&mut self, node_id: &str) -> Result<()> {
        if self.path.contains(&node_id.to_string()) {
            return Err(UnivError::CircularReference {
                node_id: node_id.to_string(),
            });
        }

        self.path.push(node_id.to_string());
        self.visited.insert(node_id.to_string());
        Ok(())
    }

    /// 结束访问一个节点
    /// 
    /// # 参数
    /// 
    /// * `node_id` - 节点标识符
    pub fn leave(&mut self, node_id: &str) {
        if let Some(pos) = self.path.iter().position(|x| x == node_id) {
            self.path.remove(pos);
        }
    }

    /// 检查节点是否已被访问
    /// 
    /// # 参数
    /// 
    /// * `node_id` - 节点标识符
    /// 
    /// # 返回
    /// 
    /// 如果已访问返回true，否则返回false
    pub fn is_visited(&self, node_id: &str) -> bool {
        self.visited.contains(node_id)
    }

    /// 获取当前路径深度
    /// 
    /// # 返回
    /// 
    /// 当前路径的深度
    pub fn current_depth(&self) -> usize {
        self.path.len()
    }

    /// 重置检测器状态
    pub fn reset(&mut self) {
        self.visited.clear();
        self.path.clear();
    }
}

impl Default for CycleDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// UTF-8 字符串验证器
pub struct StringValidator;

impl StringValidator {
    /// 验证并规范化UTF-8字符串
    /// 
    /// # 参数
    /// 
    /// * `data` - 字节数据
    /// * `normalize` - 是否进行NFC规范化
    /// 
    /// # 返回
    /// 
    /// 规范化后的字符串
    pub fn validate_and_normalize(data: &[u8], normalize: bool) -> Result<String> {
        let s = std::str::from_utf8(data)?;
        
        if normalize {
            // 注意：这里应该使用真正的Unicode NFC规范化
            // 为了简化依赖，暂时直接返回原字符串
            Ok(s.to_string())
        } else {
            Ok(s.to_string())
        }
    }

    /// 检查字符串是否为有效的标识符
    /// 
    /// # 参数
    /// 
    /// * `s` - 要检查的字符串
    /// 
    /// # 返回
    /// 
    /// 如果是有效标识符返回true，否则返回false
    pub fn is_valid_identifier(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }

        let first_char = s.chars().next().unwrap();
        if !first_char.is_alphabetic() && first_char != '_' {
            return false;
        }

        s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    }

    /// 检查字符串是否为有效的命名空间
    /// 
    /// # 参数
    /// 
    /// * `namespace` - 要检查的命名空间字符串
    /// 
    /// # 返回
    /// 
    /// 如果是有效命名空间返回true，否则返回false
    pub fn is_valid_namespace(namespace: &str) -> bool {
        if namespace.is_empty() {
            return false;
        }

        // 检查反向域名格式，如 "org.example"
        let parts: Vec<&str> = namespace.split('.').collect();
        if parts.len() < 2 {
            return false;
        }

        parts.iter().all(|part| {
            !part.is_empty() && 
            part.chars().all(|c| c.is_alphanumeric() || c == '-') &&
            part.chars().next().map_or(false, |c| c.is_alphabetic())
        })
    }
}

/// 数据完整性验证器
pub struct IntegrityValidator;

impl IntegrityValidator {
    /// 验证数据长度
    /// 
    /// # 参数
    /// 
    /// * `data` - 数据
    /// * `expected_length` - 期望长度
    /// 
    /// # 返回
    /// 
    /// 如果长度匹配返回Ok，否则返回错误
    pub fn validate_length(data: &[u8], expected_length: usize) -> Result<()> {
        if data.len() != expected_length {
            return Err(UnivError::IncompleteData {
                expected: expected_length,
                actual: data.len(),
            });
        }
        Ok(())
    }

    /// 验证最小数据长度
    /// 
    /// # 参数
    /// 
    /// * `data` - 数据
    /// * `min_length` - 最小长度
    /// 
    /// # 返回
    /// 
    /// 如果长度足够返回Ok，否则返回错误
    pub fn validate_min_length(data: &[u8], min_length: usize) -> Result<()> {
        if data.len() < min_length {
            return Err(UnivError::IncompleteData {
                expected: min_length,
                actual: data.len(),
            });
        }
        Ok(())
    }

    /// 验证数据范围
    /// 
    /// # 参数
    /// 
    /// * `data` - 数据
    /// * `offset` - 偏移量
    /// * `length` - 长度
    /// 
    /// # 返回
    /// 
    /// 如果范围有效返回Ok，否则返回错误
    pub fn validate_range(data: &[u8], offset: usize, length: usize) -> Result<()> {
        if offset + length > data.len() {
            return Err(UnivError::IncompleteData {
                expected: offset + length,
                actual: data.len(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_validator() {
        let validator = SecurityValidator::new();
        
        // 测试块数量验证
        assert!(validator.validate_chunk_count(1000).is_ok());
        assert!(validator.validate_chunk_count(validator.max_chunks + 1).is_err());
        
        // 测试总大小验证
        assert!(validator.validate_total_size(1024 * 1024).is_ok());
        assert!(validator.validate_total_size(validator.max_raw_size + 1).is_err());
        
        // 测试块大小验证
        assert!(validator.validate_chunk_size(1024).is_ok());
        assert!(validator.validate_chunk_size(validator.max_chunk_raw + 1).is_err());
        
        // 测试引用深度验证
        assert!(validator.validate_reference_depth(100).is_ok());
        assert!(validator.validate_reference_depth(validator.max_ref_depth + 1).is_err());
    }

    #[test]
    fn test_compression_ratio_validation() {
        let validator = SecurityValidator::new();
        
        // 正常压缩比
        assert!(validator.validate_compression_ratio(1000, 100).is_ok());
        
        // 压缩膨胀（可能的攻击）
        assert!(validator.validate_compression_ratio(100, 3200).is_err());
        
        // 边界情况
        assert!(validator.validate_compression_ratio(100, 0).is_ok());
    }

    #[test]
    fn test_cycle_detector() {
        let mut detector = CycleDetector::new();
        
        // 正常访问序列
        assert!(detector.visit("A").is_ok());
        assert!(detector.visit("B").is_ok());
        assert!(detector.visit("C").is_ok());
        
        detector.leave("C");
        detector.leave("B");
        detector.leave("A");
        
        // 检测循环
        assert!(detector.visit("A").is_ok());
        assert!(detector.visit("B").is_ok());
        assert!(detector.visit("A").is_err()); // 循环引用
    }

    #[test]
    fn test_string_validator() {
        // 测试有效标识符
        assert!(StringValidator::is_valid_identifier("test"));
        assert!(StringValidator::is_valid_identifier("_private"));
        assert!(StringValidator::is_valid_identifier("test_123"));
        assert!(!StringValidator::is_valid_identifier("123test"));
        assert!(!StringValidator::is_valid_identifier(""));
        
        // 测试有效命名空间
        assert!(StringValidator::is_valid_namespace("org.example"));
        assert!(StringValidator::is_valid_namespace("com.company.project"));
        assert!(!StringValidator::is_valid_namespace("invalid"));
        assert!(!StringValidator::is_valid_namespace(""));
        assert!(!StringValidator::is_valid_namespace("123.invalid"));
    }

    #[test]
    fn test_integrity_validator() {
        let data = b"test data";
        
        // 测试长度验证
        assert!(IntegrityValidator::validate_length(data, 9).is_ok());
        assert!(IntegrityValidator::validate_length(data, 10).is_err());
        
        // 测试最小长度验证
        assert!(IntegrityValidator::validate_min_length(data, 5).is_ok());
        assert!(IntegrityValidator::validate_min_length(data, 10).is_err());
        
        // 测试范围验证
        assert!(IntegrityValidator::validate_range(data, 0, 9).is_ok());
        assert!(IntegrityValidator::validate_range(data, 5, 4).is_ok());
        assert!(IntegrityValidator::validate_range(data, 5, 5).is_err());
    }

    #[test]
    fn test_utf8_validation() {
        let valid_utf8 = "Hello, 世界!".as_bytes();
        let result = StringValidator::validate_and_normalize(valid_utf8, false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, 世界!");
        
        let invalid_utf8 = &[0xFF, 0xFE];
        let result = StringValidator::validate_and_normalize(invalid_utf8, false);
        assert!(result.is_err());
    }
}