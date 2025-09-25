//! # UNIV 文件头处理
//!
//! 处理 UNIV 容器的文件头，包括魔数、Profile、版本信息和扩展字段。

use crate::constants::{MAGIC, header_flags, header_ext_types};
use crate::error::{UnivError, Result};
use crate::profile::Profile;
use bytes::{Buf, BufMut, BytesMut};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// UNIV 文件头结构
/// 
/// 包含魔数、Profile类型、版本信息、标志位和可选的扩展字段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    /// Profile 类型
    pub profile: Profile,
    /// 主版本号
    pub major_version: u8,
    /// 次版本号  
    pub minor_version: u8,
    /// 标志位
    pub flags: u16,
    /// 头部扩展字段
    pub extensions: HashMap<u8, HeaderExtension>,
}

/// 头部扩展字段值
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HeaderExtension {
    /// 生产者信息（UTF-8字符串）
    Producer(String),
    /// 创建时间戳（UTC纳秒）
    CreationTimestamp(u64),
    /// 应用提示（UTF-8字符串）
    AppHint(String),
    /// 默认编解码器
    DefaultCodec { codec: u8, params: Vec<u8> },
    /// 默认哈希算法
    DefaultHashAlg(u8),
    /// Profile次版本/选项（CBOR数据）
    ProfileMinorOptions(Vec<u8>),
    /// 命名空间根（UTF-8字符串）
    NamespaceRoot(String),
    /// 解析器提示（CBOR数据）  
    ResolverHints(Vec<u8>),
    /// 时间窗口大小（秒）- TSDB专用
    WindowSizeSeconds(u32),
    /// 列组提示（UTF-8字符串）- TABL专用
    ColumnGroupHint(String),
    /// 未知扩展（原始字节）
    Unknown(Vec<u8>),
}

impl Header {
    /// 创建一个新的文件头
    /// 
    /// # 参数
    /// 
    /// * `profile` - Profile类型
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use universe::{Header, Profile};
    /// 
    /// let header = Header::new(Profile::Recd);
    /// assert_eq!(header.profile, Profile::Recd);
    /// ```
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            major_version: 1,
            minor_version: 1, // 更新为 v1.1.0 规范
            flags: 0,
            extensions: HashMap::new(),
        }
    }

    /// 设置标志位
    /// 
    /// # 参数
    /// 
    /// * `flag` - 要设置的标志位
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use universe::{Header, Profile};
    /// use universe::constants::header_flags;
    /// 
    /// let mut header = Header::new(Profile::Recd);
    /// header.set_flag(header_flags::HAS_HEADER_EXT);
    /// assert!(header.has_flag(header_flags::HAS_HEADER_EXT));
    /// ```
    pub fn set_flag(&mut self, flag: u16) {
        self.flags |= flag;
    }

    /// 清除标志位
    /// 
    /// # 参数
    /// 
    /// * `flag` - 要清除的标志位
    pub fn clear_flag(&mut self, flag: u16) {
        self.flags &= !flag;
    }

    /// 检查是否设置了指定标志位
    /// 
    /// # 参数
    /// 
    /// * `flag` - 要检查的标志位
    /// 
    /// # 返回
    /// 
    /// 如果设置了返回true，否则返回false
    pub fn has_flag(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    /// 添加头部扩展字段
    /// 
    /// # 参数
    /// 
    /// * `ext_type` - 扩展字段类型
    /// * `extension` - 扩展字段值
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use universe::{Header, Profile};
    /// use universe::header::{HeaderExtension};
    /// use universe::constants::header_ext_types;
    /// 
    /// let mut header = Header::new(Profile::Recd);
    /// header.add_extension(
    ///     header_ext_types::PRODUCER,
    ///     HeaderExtension::Producer("universe-rust".to_string())
    /// );
    /// ```
    pub fn add_extension(&mut self, ext_type: u8, extension: HeaderExtension) {
        self.extensions.insert(ext_type, extension);
        self.set_flag(header_flags::HAS_HEADER_EXT);
    }

    /// 获取头部扩展字段
    /// 
    /// # 参数
    /// 
    /// * `ext_type` - 扩展字段类型
    /// 
    /// # 返回
    /// 
    /// 扩展字段值的引用，如果不存在返回None
    pub fn get_extension(&self, ext_type: u8) -> Option<&HeaderExtension> {
        self.extensions.get(&ext_type)
    }

    /// 设置生产者信息
    /// 
    /// # 参数
    /// 
    /// * `producer` - 生产者字符串
    pub fn set_producer<S: Into<String>>(&mut self, producer: S) {
        self.add_extension(
            header_ext_types::PRODUCER,
            HeaderExtension::Producer(producer.into()),
        );
    }

    /// 设置创建时间戳为当前时间
    pub fn set_creation_timestamp_now(&mut self) {
        let now = Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        self.add_extension(
            header_ext_types::CREATION_TIMESTAMP,
            HeaderExtension::CreationTimestamp(now),
        );
    }

    /// 设置创建时间戳
    /// 
    /// # 参数
    /// 
    /// * `timestamp` - UTC时间戳（纳秒）
    pub fn set_creation_timestamp(&mut self, timestamp: u64) {
        self.add_extension(
            header_ext_types::CREATION_TIMESTAMP,
            HeaderExtension::CreationTimestamp(timestamp),
        );
    }

    /// 设置命名空间根
    /// 
    /// # 参数
    /// 
    /// * `namespace` - 命名空间字符串
    pub fn set_namespace_root<S: Into<String>>(&mut self, namespace: S) {
        self.add_extension(
            header_ext_types::NAMESPACE_ROOT,
            HeaderExtension::NamespaceRoot(namespace.into()),
        );
    }

    /// 获取生产者信息
    /// 
    /// # 返回
    /// 
    /// 生产者字符串的引用，如果不存在返回None
    pub fn get_producer(&self) -> Option<&str> {
        match self.get_extension(header_ext_types::PRODUCER) {
            Some(HeaderExtension::Producer(producer)) => Some(producer),
            _ => None,
        }
    }

    /// 获取创建时间戳
    /// 
    /// # 返回
    /// 
    /// 创建时间戳，如果不存在返回None
    pub fn get_creation_timestamp(&self) -> Option<DateTime<Utc>> {
        match self.get_extension(header_ext_types::CREATION_TIMESTAMP) {
            Some(HeaderExtension::CreationTimestamp(timestamp)) => {
                DateTime::from_timestamp_nanos(*timestamp as i64).into()
            }
            _ => None,
        }
    }

    /// 获取命名空间根
    /// 
    /// # 返回
    /// 
    /// 命名空间字符串的引用，如果不存在返回None
    pub fn get_namespace_root(&self) -> Option<&str> {
        match self.get_extension(header_ext_types::NAMESPACE_ROOT) {
            Some(HeaderExtension::NamespaceRoot(namespace)) => Some(namespace),
            _ => None,
        }
    }

    /// 序列化文件头到字节流
    /// 
    /// # 返回
    /// 
    /// 序列化后的字节数据
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = BytesMut::new();

        // 写入魔数（4字节）
        buf.put_slice(MAGIC);

        // 写入Profile代码（4字节）
        buf.put_slice(&self.profile.to_code());

        // 写入版本信息（2字节）
        buf.put_u8(self.major_version);
        buf.put_u8(self.minor_version);

        // 写入标志位（2字节，小端序）
        buf.put_u16_le(self.flags);

        // 如果有扩展字段，序列化它们
        if self.has_flag(header_flags::HAS_HEADER_EXT) {
            let ext_data = self.serialize_extensions()?;
            // 写入扩展数据长度（4字节，小端序）
            buf.put_u32_le(ext_data.len() as u32);
            // 写入扩展数据
            buf.put_slice(&ext_data);
        }

        Ok(buf.to_vec())
    }

    /// 从字节流反序列化文件头
    /// 
    /// # 参数
    /// 
    /// * `data` - 要解析的字节数据
    /// 
    /// # 返回
    /// 
    /// 解析结果，包含文件头和消费的字节数
    pub fn deserialize(data: &[u8]) -> Result<(Self, usize)> {
        if data.len() < 12 {
            return Err(UnivError::IncompleteData {
                expected: 12,
                actual: data.len(),
            });
        }

        let mut buf = data;
        let original_len = buf.len();

        // 读取并验证魔数
        let mut magic = [0u8; 4];
        buf.copy_to_slice(&mut magic);
        if magic != *MAGIC {
            return Err(UnivError::InvalidMagic {
                expected: *MAGIC,
                actual: magic,
            });
        }

        // 读取Profile代码
        let mut profile_code = [0u8; 4];
        buf.copy_to_slice(&mut profile_code);
        let profile = Profile::from_code(&profile_code)?;

        // 读取版本信息
        let major_version = buf.get_u8();
        let minor_version = buf.get_u8();

        // 读取标志位
        let flags = buf.get_u16_le();

        let mut extensions = HashMap::new();

        // 如果有扩展字段，读取它们
        if flags & header_flags::HAS_HEADER_EXT != 0 {
            if buf.remaining() < 4 {
                return Err(UnivError::IncompleteData {
                    expected: 4,
                    actual: buf.remaining(),
                });
            }

            let ext_len = buf.get_u32_le() as usize;
            if buf.remaining() < ext_len {
                return Err(UnivError::IncompleteData {
                    expected: ext_len,
                    actual: buf.remaining(),
                });
            }

            let ext_data = &buf[..ext_len];
            extensions = Self::deserialize_extensions(ext_data)?;
            buf.advance(ext_len);
        }

        let consumed = original_len - buf.len();
        let header = Self {
            profile,
            major_version,
            minor_version,
            flags,
            extensions,
        };

        Ok((header, consumed))
    }

    /// 序列化扩展字段
    fn serialize_extensions(&self) -> Result<Vec<u8>> {
        let mut buf = BytesMut::new();

        for (&ext_type, extension) in &self.extensions {
            // 写入扩展类型（1字节）
            buf.put_u8(ext_type);

            // 序列化扩展值
            let ext_data = extension.serialize()?;

            let mut len_buf = Vec::new();
            leb128::write::unsigned(&mut len_buf, ext_data.len() as u64)
                .map_err(|e| UnivError::serialization_error(format!("LEB128编码失败: {}", e)))?;
            buf.put_slice(&len_buf);

            // 写入扩展数据
            buf.put_slice(&ext_data);
        }

        Ok(buf.to_vec())
    }

    /// 反序列化扩展字段
    fn deserialize_extensions(data: &[u8]) -> Result<HashMap<u8, HeaderExtension>> {
        let mut extensions = HashMap::new();
        let mut buf = data;

        while !buf.is_empty() {
            // 读取扩展类型
            if buf.is_empty() {
                break;
            }
            let ext_type = buf[0];
            buf = &buf[1..];

            // 读取扩展长度
            let mut ext_len_buf = buf;
            let ext_len = leb128::read::unsigned(&mut ext_len_buf)
                .map_err(|e| UnivError::deserialization_error(format!("LEB128解码失败: {}", e)))?;
            let _len_bytes = buf.len() - ext_len_buf.len();
            buf = ext_len_buf;

            let ext_len = ext_len as usize;
            if buf.len() < ext_len {
                return Err(UnivError::IncompleteData {
                    expected: ext_len,
                    actual: buf.len(),
                });
            }

            // 读取扩展数据
            let ext_data = &buf[..ext_len];
            buf = &buf[ext_len..];

            // 反序列化扩展值
            let extension = HeaderExtension::deserialize(ext_type, ext_data)?;
            extensions.insert(ext_type, extension);
        }

        Ok(extensions)
    }

    /// 估算序列化后的大小
    /// 
    /// # 返回
    /// 
    /// 估算的字节数
    pub fn estimated_size(&self) -> usize {
        let mut size = 12; // 基础头部大小

        if self.has_flag(header_flags::HAS_HEADER_EXT) {
            size += 4; // 扩展长度字段
            for extension in self.extensions.values() {
                size += 1; // 扩展类型
                size += 5; // 最大LEB128长度
                size += extension.estimated_size();
            }
        }

        size
    }
}

impl HeaderExtension {
    /// 序列化扩展字段值
    fn serialize(&self) -> Result<Vec<u8>> {
        match self {
            HeaderExtension::Producer(s) => Ok(s.as_bytes().to_vec()),
            HeaderExtension::CreationTimestamp(ts) => {
                let mut buf = BytesMut::with_capacity(8);
                buf.put_u64_le(*ts);
                Ok(buf.to_vec())
            }
            HeaderExtension::AppHint(s) => Ok(s.as_bytes().to_vec()),
            HeaderExtension::DefaultCodec { codec, params } => {
                let mut buf = BytesMut::with_capacity(1 + params.len());
                buf.put_u8(*codec);
                buf.put_slice(params);
                Ok(buf.to_vec())
            }
            HeaderExtension::DefaultHashAlg(alg) => Ok(vec![*alg]),
            HeaderExtension::ProfileMinorOptions(data) => Ok(data.clone()),
            HeaderExtension::NamespaceRoot(s) => Ok(s.as_bytes().to_vec()),
            HeaderExtension::ResolverHints(data) => Ok(data.clone()),
            HeaderExtension::WindowSizeSeconds(size) => {
                let mut buf = BytesMut::with_capacity(4);
                buf.put_u32_le(*size);
                Ok(buf.to_vec())
            }
            HeaderExtension::ColumnGroupHint(s) => Ok(s.as_bytes().to_vec()),
            HeaderExtension::Unknown(data) => Ok(data.clone()),
        }
    }

    /// 反序列化扩展字段值
    fn deserialize(ext_type: u8, data: &[u8]) -> Result<Self> {
        match ext_type {
            header_ext_types::PRODUCER => {
                let s = std::str::from_utf8(data)?;
                Ok(HeaderExtension::Producer(s.to_string()))
            }
            header_ext_types::CREATION_TIMESTAMP => {
                if data.len() != 8 {
                    return Err(UnivError::deserialization_error("时间戳长度错误"));
                }
                let mut buf = data;
                let ts = buf.get_u64_le();
                Ok(HeaderExtension::CreationTimestamp(ts))
            }
            header_ext_types::APP_HINT => {
                let s = std::str::from_utf8(data)?;
                Ok(HeaderExtension::AppHint(s.to_string()))
            }
            header_ext_types::DEFAULT_CODEC => {
                if data.is_empty() {
                    return Err(UnivError::deserialization_error("编解码器数据为空"));
                }
                let codec = data[0];
                let params = data[1..].to_vec();
                Ok(HeaderExtension::DefaultCodec { codec, params })
            }
            header_ext_types::DEFAULT_HASH_ALG => {
                if data.len() != 1 {
                    return Err(UnivError::deserialization_error("哈希算法长度错误"));
                }
                Ok(HeaderExtension::DefaultHashAlg(data[0]))
            }
            header_ext_types::PROFILE_MINOR_OPTIONS => {
                Ok(HeaderExtension::ProfileMinorOptions(data.to_vec()))
            }
            header_ext_types::NAMESPACE_ROOT => {
                let s = std::str::from_utf8(data)?;
                Ok(HeaderExtension::NamespaceRoot(s.to_string()))
            }
            header_ext_types::RESOLVER_HINTS => {
                Ok(HeaderExtension::ResolverHints(data.to_vec()))
            }
            header_ext_types::WINDOW_SIZE_SECONDS => {
                if data.len() != 4 {
                    return Err(UnivError::deserialization_error("窗口大小长度错误"));
                }
                let mut buf = data;
                let size = buf.get_u32_le();
                Ok(HeaderExtension::WindowSizeSeconds(size))
            }
            header_ext_types::COLUMN_GROUP_HINT => {
                let s = std::str::from_utf8(data)?;
                Ok(HeaderExtension::ColumnGroupHint(s.to_string()))
            }
            _ => Ok(HeaderExtension::Unknown(data.to_vec())),
        }
    }

    /// 估算序列化后的大小
    fn estimated_size(&self) -> usize {
        match self {
            HeaderExtension::Producer(s) => s.len(),
            HeaderExtension::CreationTimestamp(_) => 8,
            HeaderExtension::AppHint(s) => s.len(),
            HeaderExtension::DefaultCodec { params, .. } => 1 + params.len(),
            HeaderExtension::DefaultHashAlg(_) => 1,
            HeaderExtension::ProfileMinorOptions(data) => data.len(),
            HeaderExtension::NamespaceRoot(s) => s.len(),
            HeaderExtension::ResolverHints(data) => data.len(),
            HeaderExtension::WindowSizeSeconds(_) => 4,
            HeaderExtension::ColumnGroupHint(s) => s.len(),
            HeaderExtension::Unknown(data) => data.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_creation() {
        let header = Header::new(Profile::Recd);
        assert_eq!(header.profile, Profile::Recd);
        assert_eq!(header.major_version, 1);
        assert_eq!(header.minor_version, 1); // 更新为 v1.1.0 规范
        assert_eq!(header.flags, 0);
    }

    #[test]
    fn test_header_flags() {
        let mut header = Header::new(Profile::Recd);
        assert!(!header.has_flag(header_flags::HAS_HEADER_EXT));
        
        header.set_flag(header_flags::HAS_HEADER_EXT);
        assert!(header.has_flag(header_flags::HAS_HEADER_EXT));
        
        header.clear_flag(header_flags::HAS_HEADER_EXT);
        assert!(!header.has_flag(header_flags::HAS_HEADER_EXT));
    }

    #[test]
    fn test_header_extensions() {
        let mut header = Header::new(Profile::Recd);
        header.set_producer("universe-rust");
        header.set_namespace_root("org.example");
        
        assert_eq!(header.get_producer(), Some("universe-rust"));
        assert_eq!(header.get_namespace_root(), Some("org.example"));
        assert!(header.has_flag(header_flags::HAS_HEADER_EXT));
    }

    #[test]
    fn test_header_serialization() {
        let mut header = Header::new(Profile::Recd);
        header.set_producer("test");
        
        let serialized = header.serialize().unwrap();
        let (deserialized, consumed) = Header::deserialize(&serialized).unwrap();
        
        assert_eq!(consumed, serialized.len());
        assert_eq!(deserialized.profile, header.profile);
        assert_eq!(deserialized.get_producer(), header.get_producer());
    }

    #[test]
    fn test_invalid_magic() {
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(b"XXXX"); // 错误的魔数
        data[4..8].copy_from_slice(b"RECD");
        
        let result = Header::deserialize(&data);
        assert!(matches!(result, Err(UnivError::InvalidMagic { .. })));
    }

    #[test]
    fn test_incomplete_data() {
        let data = vec![0u8; 8]; // 数据太短
        let result = Header::deserialize(&data);
        assert!(matches!(result, Err(UnivError::IncompleteData { .. })));
    }
}