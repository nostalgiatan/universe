//! # 安全与限制系统
//!
//! 提供 UNIV 容器的安全验证、限制检查和保护机制。

use crate::error::{UnivError, Result};
use crate::util::validation::{SecurityValidator, CycleDetector};
use crate::constants::security_limits;
use std::collections::HashMap;

/// 安全上下文
#[derive(Debug, Clone)]
pub struct SecurityContext {
    /// 安全验证器
    pub validator: SecurityValidator,
    /// 循环检测器
    pub cycle_detector: CycleDetector,
    /// 当前统计信息
    pub stats: SecurityStats,
}

impl SecurityContext {
    /// 创建新的安全上下文
    pub fn new() -> Self {
        Self {
            validator: SecurityValidator::new(),
            cycle_detector: CycleDetector::new(),
            stats: SecurityStats::new(),
        }
    }

    /// 创建自定义安全上下文
    /// 
    /// # 参数
    /// 
    /// * `validator` - 自定义安全验证器
    pub fn with_validator(validator: SecurityValidator) -> Self {
        Self {
            validator,
            cycle_detector: CycleDetector::new(),
            stats: SecurityStats::new(),
        }
    }

    /// 验证容器整体安全性
    /// 
    /// # 返回
    /// 
    /// 如果安全验证通过返回Ok，否则返回错误
    pub fn validate_container(&self) -> Result<()> {
        // 验证块数量
        self.validator.validate_chunk_count(self.stats.chunk_count)?;
        
        // 验证总数据大小
        self.validator.validate_total_size(self.stats.total_raw_size)?;
        
        // 验证引用深度
        if let Some(max_depth) = self.stats.max_reference_depth {
            self.validator.validate_reference_depth(max_depth)?;
        }
        
        // 验证字符串表大小
        if let Some(string_table_size) = self.stats.string_table_size {
            self.validator.validate_string_table_size(string_table_size)?;
        }
        
        Ok(())
    }

    /// 验证单个块的安全性
    /// 
    /// # 参数
    /// 
    /// * `raw_size` - 原始大小
    /// * `compressed_size` - 压缩后大小
    /// 
    /// # 返回
    /// 
    /// 如果验证通过返回Ok，否则返回错误
    pub fn validate_chunk(&self, raw_size: u32, compressed_size: u32) -> Result<()> {
        // 验证块大小
        self.validator.validate_chunk_size(raw_size)?;
        
        // 验证压缩比
        self.validator.validate_compression_ratio(raw_size, compressed_size)?;
        
        Ok(())
    }

    /// 重置安全上下文
    pub fn reset(&mut self) {
        self.cycle_detector.reset();
        self.stats = SecurityStats::new();
    }

    /// 更新统计信息
    pub fn update_stats(&mut self, update: SecurityStatsUpdate) {
        self.stats.apply_update(update);
    }
}

impl Default for SecurityContext {
    fn default() -> Self {
        Self::new()
    }
}

/// 安全统计信息
#[derive(Debug, Clone)]
pub struct SecurityStats {
    /// 块数量
    pub chunk_count: u32,
    /// 总原始数据大小
    pub total_raw_size: u64,
    /// 总压缩数据大小
    pub total_compressed_size: u64,
    /// 最大引用深度
    pub max_reference_depth: Option<u32>,
    /// 字符串表大小
    pub string_table_size: Option<u32>,
    /// 各类型块的数量统计
    pub chunk_type_counts: HashMap<u8, u32>,
    /// 压缩比统计
    pub compression_ratios: Vec<f32>,
}

impl SecurityStats {
    /// 创建新的安全统计信息
    pub fn new() -> Self {
        Self {
            chunk_count: 0,
            total_raw_size: 0,
            total_compressed_size: 0,
            max_reference_depth: None,
            string_table_size: None,
            chunk_type_counts: HashMap::new(),
            compression_ratios: Vec::new(),
        }
    }

    /// 应用统计更新
    /// 
    /// # 参数
    /// 
    /// * `update` - 统计更新信息
    pub fn apply_update(&mut self, update: SecurityStatsUpdate) {
        match update {
            SecurityStatsUpdate::AddChunk { chunk_type, raw_size, compressed_size } => {
                self.chunk_count += 1;
                self.total_raw_size += raw_size as u64;
                self.total_compressed_size += compressed_size as u64;
                
                *self.chunk_type_counts.entry(chunk_type).or_insert(0) += 1;
                
                if compressed_size > 0 {
                    let ratio = raw_size as f32 / compressed_size as f32;
                    self.compression_ratios.push(ratio);
                }
            }
            SecurityStatsUpdate::SetReferenceDepth(depth) => {
                self.max_reference_depth = Some(
                    self.max_reference_depth.map_or(depth, |current| current.max(depth))
                );
            }
            SecurityStatsUpdate::SetStringTableSize(size) => {
                self.string_table_size = Some(size);
            }
        }
    }

    /// 获取平均压缩比
    /// 
    /// # 返回
    /// 
    /// 平均压缩比
    pub fn average_compression_ratio(&self) -> f32 {
        if self.compression_ratios.is_empty() {
            return 1.0;
        }
        
        let sum: f32 = self.compression_ratios.iter().sum();
        sum / self.compression_ratios.len() as f32
    }

    /// 获取总体压缩比
    /// 
    /// # 返回
    /// 
    /// 总体压缩比（总原始大小/总压缩大小）
    pub fn overall_compression_ratio(&self) -> f32 {
        if self.total_compressed_size == 0 {
            return 1.0;
        }
        
        self.total_raw_size as f32 / self.total_compressed_size as f32
    }

    /// 检查是否超出任何安全限制
    /// 
    /// # 参数
    /// 
    /// * `validator` - 安全验证器
    /// 
    /// # 返回
    /// 
    /// 超出限制的详细信息
    pub fn check_limits(&self, validator: &SecurityValidator) -> Vec<String> {
        let mut violations = Vec::new();
        
        if self.chunk_count > validator.max_chunks {
            violations.push(format!("块数量超限: {} > {}", self.chunk_count, validator.max_chunks));
        }
        
        if self.total_raw_size > validator.max_raw_size {
            violations.push(format!("总数据大小超限: {} > {}", self.total_raw_size, validator.max_raw_size));
        }
        
        if let Some(depth) = self.max_reference_depth {
            if depth > validator.max_ref_depth {
                violations.push(format!("引用深度超限: {} > {}", depth, validator.max_ref_depth));
            }
        }
        
        if let Some(size) = self.string_table_size {
            if size > validator.max_string_table {
                violations.push(format!("字符串表大小超限: {} > {}", size, validator.max_string_table));
            }
        }
        
        violations
    }
}

impl Default for SecurityStats {
    fn default() -> Self {
        Self::new()
    }
}

/// 安全统计更新
#[derive(Debug, Clone)]
pub enum SecurityStatsUpdate {
    /// 添加块
    AddChunk {
        chunk_type: u8,
        raw_size: u32,
        compressed_size: u32,
    },
    /// 设置引用深度
    SetReferenceDepth(u32),
    /// 设置字符串表大小
    SetStringTableSize(u32),
}

/// 安全策略
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    /// 是否启用严格模式
    pub strict_mode: bool,
    /// 是否允许外部引用
    pub allow_external_refs: bool,
    /// 是否要求签名验证
    pub require_signatures: bool,
    /// 允许的最大文件大小
    pub max_file_size: u64,
    /// 允许的Profile列表
    pub allowed_profiles: Vec<String>,
    /// 禁止的变换标志
    pub forbidden_transforms: u16,
}

impl SecurityPolicy {
    /// 创建默认安全策略
    pub fn default_policy() -> Self {
        Self {
            strict_mode: false,
            allow_external_refs: true,
            require_signatures: false,
            max_file_size: security_limits::MAX_RAW_SIZE,
            allowed_profiles: vec![
                "BLOB".to_string(),
                "RECD".to_string(),
                "TABL".to_string(),
                "TSDB".to_string(),
                "GRPH".to_string(),
                "TYPE".to_string(),
            ],
            forbidden_transforms: 0,
        }
    }

    /// 创建严格安全策略
    pub fn strict_policy() -> Self {
        Self {
            strict_mode: true,
            allow_external_refs: false,
            require_signatures: true,
            max_file_size: 100 * 1024 * 1024, // 100 MB
            allowed_profiles: vec![
                "RECD".to_string(),
                "TYPE".to_string(),
            ],
            forbidden_transforms: 0,
        }
    }

    /// 验证Profile是否被允许
    /// 
    /// # 参数
    /// 
    /// * `profile` - Profile代码
    /// 
    /// # 返回
    /// 
    /// 如果允许返回Ok，否则返回错误
    pub fn validate_profile(&self, profile: &str) -> Result<()> {
        if !self.allowed_profiles.contains(&profile.to_string()) {
            return Err(UnivError::UnsupportedProfile { 
                profile: profile.as_bytes().try_into().unwrap_or([0; 4]) 
            });
        }
        Ok(())
    }

    /// 验证变换标志
    /// 
    /// # 参数
    /// 
    /// * `transform_flags` - 变换标志
    /// 
    /// # 返回
    /// 
    /// 如果允许返回Ok，否则返回错误
    pub fn validate_transforms(&self, transform_flags: u16) -> Result<()> {
        if transform_flags & self.forbidden_transforms != 0 {
            return Err(UnivError::InvalidTransform {
                reason: "使用了被禁止的变换".to_string(),
            });
        }
        Ok(())
    }

    /// 验证文件大小
    /// 
    /// # 参数
    /// 
    /// * `file_size` - 文件大小
    /// 
    /// # 返回
    /// 
    /// 如果允许返回Ok，否则返回错误
    pub fn validate_file_size(&self, file_size: u64) -> Result<()> {
        if file_size > self.max_file_size {
            return Err(UnivError::security_limit_exceeded(
                "file_size",
                self.max_file_size,
            ));
        }
        Ok(())
    }
}

/// 攻击检测器
pub struct AttackDetector {
    /// 可疑活动计数器
    suspicious_activities: HashMap<String, u32>,
    /// 检测阈值
    thresholds: AttackThresholds,
}

impl AttackDetector {
    /// 创建新的攻击检测器
    pub fn new() -> Self {
        Self {
            suspicious_activities: HashMap::new(),
            thresholds: AttackThresholds::default(),
        }
    }

    /// 检测压缩炸弹
    /// 
    /// # 参数
    /// 
    /// * `compressed_size` - 压缩大小
    /// * `raw_size` - 解压后大小
    /// 
    /// # 返回
    /// 
    /// 如果检测到攻击返回错误
    pub fn detect_compression_bomb(&mut self, compressed_size: u32, raw_size: u32) -> Result<()> {
        if compressed_size == 0 {
            return Ok(());
        }
        
        let ratio = raw_size as f32 / compressed_size as f32;
        if ratio > self.thresholds.max_compression_ratio {
            let count = self.suspicious_activities
                .entry("compression_bomb".to_string())
                .or_insert(0);
            *count += 1;
            
            if *count > self.thresholds.max_compression_bomb_attempts {
                return Err(UnivError::compression_error(
                    "检测到压缩炸弹攻击".to_string()
                ));
            }
            
            return Err(UnivError::compression_error(format!(
                "可疑的压缩比: {:.2}",
                ratio
            )));
        }
        
        Ok(())
    }

    /// 检测过深引用
    /// 
    /// # 参数
    /// 
    /// * `depth` - 引用深度
    /// 
    /// # 返回
    /// 
    /// 如果检测到攻击返回错误
    pub fn detect_deep_reference(&mut self, depth: u32) -> Result<()> {
        if depth > self.thresholds.max_reference_depth {
            let count = self.suspicious_activities
                .entry("deep_reference".to_string())
                .or_insert(0);
            *count += 1;
            
            return Err(UnivError::ReferenceDepthExceeded {
                current: depth,
                max: self.thresholds.max_reference_depth,
            });
        }
        
        Ok(())
    }

    /// 检测异常块数量
    /// 
    /// # 参数
    /// 
    /// * `chunk_count` - 块数量
    /// 
    /// # 返回
    /// 
    /// 如果检测到攻击返回错误
    pub fn detect_excessive_chunks(&mut self, chunk_count: u32) -> Result<()> {
        if chunk_count > self.thresholds.max_chunk_count {
            return Err(UnivError::security_limit_exceeded(
                "chunk_count",
                self.thresholds.max_chunk_count as u64,
            ));
        }
        
        Ok(())
    }

    /// 重置检测器状态
    pub fn reset(&mut self) {
        self.suspicious_activities.clear();
    }

    /// 获取可疑活动统计
    pub fn get_suspicious_activities(&self) -> &HashMap<String, u32> {
        &self.suspicious_activities
    }
}

impl Default for AttackDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// 攻击检测阈值
#[derive(Debug, Clone)]
pub struct AttackThresholds {
    /// 最大压缩比
    pub max_compression_ratio: f32,
    /// 最大压缩炸弹尝试次数
    pub max_compression_bomb_attempts: u32,
    /// 最大引用深度
    pub max_reference_depth: u32,
    /// 最大块数量
    pub max_chunk_count: u32,
}

impl Default for AttackThresholds {
    fn default() -> Self {
        Self {
            max_compression_ratio: security_limits::COMPRESSION_RATIO_THRESHOLD,
            max_compression_bomb_attempts: 3,
            max_reference_depth: security_limits::MAX_REF_DEPTH,
            max_chunk_count: security_limits::MAX_CHUNKS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_context() {
        let mut context = SecurityContext::new();
        
        // 更新统计信息
        context.update_stats(SecurityStatsUpdate::AddChunk {
            chunk_type: 1,
            raw_size: 1024,
            compressed_size: 512,
        });
        
        assert_eq!(context.stats.chunk_count, 1);
        assert_eq!(context.stats.total_raw_size, 1024);
        assert_eq!(context.stats.total_compressed_size, 512);
        
        // 验证容器安全性
        assert!(context.validate_container().is_ok());
    }

    #[test]
    fn test_security_stats() {
        let mut stats = SecurityStats::new();
        
        stats.apply_update(SecurityStatsUpdate::AddChunk {
            chunk_type: 1,
            raw_size: 2000,
            compressed_size: 1000,
        });
        
        stats.apply_update(SecurityStatsUpdate::AddChunk {
            chunk_type: 2,
            raw_size: 4000,
            compressed_size: 1000,
        });
        
        assert_eq!(stats.chunk_count, 2);
        assert_eq!(stats.average_compression_ratio(), 3.0); // (2.0 + 4.0) / 2
        assert_eq!(stats.overall_compression_ratio(), 3.0); // 6000 / 2000
    }

    #[test]
    fn test_security_policy() {
        let policy = SecurityPolicy::default_policy();
        
        assert!(policy.validate_profile("RECD").is_ok());
        assert!(policy.validate_profile("UNKN").is_err());
        
        assert!(policy.validate_file_size(1024).is_ok());
        assert!(policy.validate_file_size(policy.max_file_size + 1).is_err());
    }

    #[test]
    fn test_attack_detector() {
        let mut detector = AttackDetector::new();
        
        // 正常压缩比
        assert!(detector.detect_compression_bomb(1000, 2000).is_ok());
        
        // 可疑压缩比
        assert!(detector.detect_compression_bomb(100, 10000).is_err());
        
        // 正常引用深度
        assert!(detector.detect_deep_reference(100).is_ok());
        
        // 过深引用
        assert!(detector.detect_deep_reference(2000).is_err());
    }

    #[test]
    fn test_strict_policy() {
        let policy = SecurityPolicy::strict_policy();
        
        assert!(policy.strict_mode);
        assert!(!policy.allow_external_refs);
        assert!(policy.require_signatures);
        assert!(policy.validate_profile("RECD").is_ok());
        assert!(policy.validate_profile("BLOB").is_err()); // 严格模式不允许
    }

    #[test]
    fn test_limits_checking() {
        let stats = SecurityStats {
            chunk_count: 2000000, // 超过默认限制
            total_raw_size: 1024,
            total_compressed_size: 512,
            max_reference_depth: Some(2000), // 超过默认限制
            string_table_size: None,
            chunk_type_counts: HashMap::new(),
            compression_ratios: Vec::new(),
        };
        
        let validator = SecurityValidator::new();
        let violations = stats.check_limits(&validator);
        
        assert_eq!(violations.len(), 2); // 两个违规
        assert!(violations[0].contains("块数量超限"));
        assert!(violations[1].contains("引用深度超限"));
    }
}