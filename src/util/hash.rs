//! # 哈希工具
//!
//! 提供 UNIV 格式支持的各种哈希算法实现。

use crate::constants::hash_algorithms;
use crate::error::{UnivError, Result};
use blake3;
use sha2::{Sha256, Digest};
use serde::{Deserialize, Serialize};

/// 哈希提供者，支持多种哈希算法
pub struct HashProvider;

impl HashProvider {
    /// 计算数据的哈希值
    /// 
    /// # 参数
    /// 
    /// * `algorithm` - 哈希算法标识
    /// * `data` - 要哈希的数据
    /// 
    /// # 返回
    /// 
    /// 哈希值字节数组
    pub fn hash(algorithm: u8, data: &[u8]) -> Result<Vec<u8>> {
        match algorithm {
            hash_algorithms::BLAKE3 => {
                let hash = blake3::hash(data);
                Ok(hash.as_bytes().to_vec())
            }
            hash_algorithms::SHA256 => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                let result = hasher.finalize();
                Ok(result.to_vec())
            }
            hash_algorithms::CRC32C => {
                let crc = crc32c::crc32c(data);
                Ok(crc.to_le_bytes().to_vec())
            }
            unknown => Err(UnivError::UnsupportedHashAlgorithm { algorithm: unknown }),
        }
    }

    /// 验证数据哈希
    /// 
    /// # 参数
    /// 
    /// * `algorithm` - 哈希算法标识
    /// * `data` - 原始数据
    /// * `expected_hash` - 期望的哈希值
    /// 
    /// # 返回
    /// 
    /// 如果哈希匹配返回Ok，否则返回错误
    pub fn verify_hash(algorithm: u8, data: &[u8], expected_hash: &[u8]) -> Result<()> {
        let computed_hash = Self::hash(algorithm, data)?;
        
        if computed_hash != expected_hash {
            return Err(UnivError::HashMismatch {
                expected: hex::encode(expected_hash),
                actual: hex::encode(&computed_hash),
            });
        }
        
        Ok(())
    }

    /// 获取哈希算法的标准长度
    /// 
    /// # 参数
    /// 
    /// * `algorithm` - 哈希算法标识
    /// 
    /// # 返回
    /// 
    /// 哈希值的标准字节长度
    pub fn hash_length(algorithm: u8) -> Result<usize> {
        match algorithm {
            hash_algorithms::BLAKE3 => Ok(32), // BLAKE3-256
            hash_algorithms::SHA256 => Ok(32), // SHA-256
            hash_algorithms::CRC32C => Ok(4),  // CRC32C
            unknown => Err(UnivError::UnsupportedHashAlgorithm { algorithm: unknown }),
        }
    }

    /// 获取哈希算法的名称
    /// 
    /// # 参数
    /// 
    /// * `algorithm` - 哈希算法标识
    /// 
    /// # 返回
    /// 
    /// 哈希算法的名称
    pub fn algorithm_name(algorithm: u8) -> &'static str {
        match algorithm {
            hash_algorithms::BLAKE3 => "BLAKE3-256",
            hash_algorithms::SHA256 => "SHA-256",
            hash_algorithms::CRC32C => "CRC32C",
            _ => "Unknown",
        }
    }

    /// 检查算法是否适合内容验证
    /// 
    /// # 参数
    /// 
    /// * `algorithm` - 哈希算法标识
    /// 
    /// # 返回
    /// 
    /// 如果适合内容验证返回true，否则返回false
    pub fn is_cryptographic(algorithm: u8) -> bool {
        matches!(algorithm, hash_algorithms::BLAKE3 | hash_algorithms::SHA256)
    }
}

/// 内容哈希结构，包含算法和哈希值
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash {
    /// 哈希算法
    pub algorithm: u8,
    /// 哈希值
    pub hash: Vec<u8>,
}

impl ContentHash {
    /// 创建新的内容哈希
    /// 
    /// # 参数
    /// 
    /// * `algorithm` - 哈希算法标识
    /// * `data` - 要哈希的数据
    /// 
    /// # 返回
    /// 
    /// 新的内容哈希结构
    pub fn new(algorithm: u8, data: &[u8]) -> Result<Self> {
        let hash = HashProvider::hash(algorithm, data)?;
        
        Ok(Self {
            algorithm,
            hash,
        })
    }

    /// 从已有哈希值创建
    /// 
    /// # 参数
    /// 
    /// * `algorithm` - 哈希算法标识
    /// * `hash` - 哈希值
    /// 
    /// # 返回
    /// 
    /// 新的内容哈希结构
    pub fn from_hash(algorithm: u8, hash: Vec<u8>) -> Self {
        Self {
            algorithm,
            hash,
        }
    }

    /// 验证数据是否匹配此哈希
    /// 
    /// # 参数
    /// 
    /// * `data` - 要验证的数据
    /// 
    /// # 返回
    /// 
    /// 如果验证成功返回Ok，否则返回错误
    pub fn verify(&self, data: &[u8]) -> Result<()> {
        HashProvider::verify_hash(self.algorithm, data, &self.hash)
    }

    /// 获取十六进制表示
    /// 
    /// # 返回
    /// 
    /// 哈希值的十六进制字符串
    pub fn hex(&self) -> String {
        hex::encode(&self.hash)
    }

    /// 获取哈希算法名称
    /// 
    /// # 返回
    /// 
    /// 算法名称字符串
    pub fn algorithm_name(&self) -> &'static str {
        HashProvider::algorithm_name(self.algorithm)
    }

    /// 获取哈希长度
    /// 
    /// # 返回
    /// 
    /// 哈希值的字节长度
    pub fn len(&self) -> usize {
        self.hash.len()
    }

    /// 检查是否为空哈希
    /// 
    /// # 返回
    /// 
    /// 如果哈希为空返回true，否则返回false
    pub fn is_empty(&self) -> bool {
        self.hash.is_empty()
    }
}

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.algorithm_name(), self.hex())
    }
}

impl std::str::FromStr for ContentHash {
    type Err = UnivError;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(UnivError::deserialization_error("无效的哈希格式，应为 algorithm:hash"));
        }

        let algorithm = match parts[0] {
            "BLAKE3-256" => hash_algorithms::BLAKE3,
            "SHA-256" => hash_algorithms::SHA256,
            "CRC32C" => hash_algorithms::CRC32C,
            _ => return Err(UnivError::deserialization_error("不支持的哈希算法")),
        };

        let hash = hex::decode(parts[1])
            .map_err(|_| UnivError::deserialization_error("无效的十六进制哈希值"))?;

        Ok(Self::from_hash(algorithm, hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake3_hash() {
        let data = b"Hello, World!";
        let hash = HashProvider::hash(hash_algorithms::BLAKE3, data).unwrap();
        
        assert_eq!(hash.len(), 32);
        
        // 验证哈希
        assert!(HashProvider::verify_hash(hash_algorithms::BLAKE3, data, &hash).is_ok());
        
        // 验证错误数据应该失败
        let wrong_data = b"Hello, World?";
        assert!(HashProvider::verify_hash(hash_algorithms::BLAKE3, wrong_data, &hash).is_err());
    }

    #[test]
    fn test_crc32c_hash() {
        let data = b"Test data";
        let hash = HashProvider::hash(hash_algorithms::CRC32C, data).unwrap();
        
        assert_eq!(hash.len(), 4);
        assert!(HashProvider::verify_hash(hash_algorithms::CRC32C, data, &hash).is_ok());
    }

    #[test]
    fn test_sha256_hash() {
        let data = b"SHA-256 test data";
        let hash = HashProvider::hash(hash_algorithms::SHA256, data).unwrap();
        
        // SHA-256 应该产生32字节哈希
        assert_eq!(hash.len(), 32);
        
        // 验证哈希
        assert!(HashProvider::verify_hash(hash_algorithms::SHA256, data, &hash).is_ok());
        
        // 验证错误数据应该失败
        let wrong_data = b"Wrong data";
        assert!(HashProvider::verify_hash(hash_algorithms::SHA256, wrong_data, &hash).is_err());
        
        // 验证SHA-256的确定性（相同输入产生相同输出）
        let hash2 = HashProvider::hash(hash_algorithms::SHA256, data).unwrap();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_content_hash() {
        let data = b"Content hash test";
        let content_hash = ContentHash::new(hash_algorithms::BLAKE3, data).unwrap();
        
        assert_eq!(content_hash.algorithm, hash_algorithms::BLAKE3);
        assert_eq!(content_hash.len(), 32);
        assert!(!content_hash.is_empty());
        
        // 验证数据
        assert!(content_hash.verify(data).is_ok());
        
        // 验证错误数据应该失败
        let wrong_data = b"Wrong content";
        assert!(content_hash.verify(wrong_data).is_err());
    }

    #[test]
    fn test_content_hash_display() {
        let data = b"Display test";
        let content_hash = ContentHash::new(hash_algorithms::BLAKE3, data).unwrap();
        
        let display_str = content_hash.to_string();
        assert!(display_str.starts_with("BLAKE3-256:"));
        
        // 测试解析
        let parsed: ContentHash = display_str.parse().unwrap();
        assert_eq!(parsed.algorithm, content_hash.algorithm);
        assert_eq!(parsed.hash, content_hash.hash);
    }

    #[test]
    fn test_hash_algorithm_properties() {
        assert_eq!(HashProvider::hash_length(hash_algorithms::BLAKE3).unwrap(), 32);
        assert_eq!(HashProvider::hash_length(hash_algorithms::CRC32C).unwrap(), 4);
        
        assert!(HashProvider::is_cryptographic(hash_algorithms::BLAKE3));
        assert!(!HashProvider::is_cryptographic(hash_algorithms::CRC32C));
        
        assert_eq!(HashProvider::algorithm_name(hash_algorithms::BLAKE3), "BLAKE3-256");
        assert_eq!(HashProvider::algorithm_name(hash_algorithms::CRC32C), "CRC32C");
    }

    #[test]
    fn test_unsupported_algorithm() {
        let result = HashProvider::hash(255, b"test");
        assert!(matches!(result, Err(UnivError::UnsupportedHashAlgorithm { algorithm: 255 })));
    }
}