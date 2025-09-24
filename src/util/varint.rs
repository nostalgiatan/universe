//! # 可变长整数工具
//!
//! 提供 LEB128 可变长整数编码和解码功能。

use crate::error::{UnivError, Result};

/// 可变长整数编解码器
pub struct VarInt;

impl VarInt {
    /// 编码无符号整数为 LEB128 格式
    /// 
    /// # 参数
    /// 
    /// * `value` - 要编码的无符号整数
    /// 
    /// # 返回
    /// 
    /// 编码后的字节数组
    pub fn encode_u64(value: u64) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        leb128::write::unsigned(&mut buf, value)
            .map_err(|e| UnivError::serialization_error(format!("LEB128编码失败: {}", e)))?;
        Ok(buf)
    }

    /// 编码有符号整数为 ZigZag + LEB128 格式
    /// 
    /// # 参数
    /// 
    /// * `value` - 要编码的有符号整数
    /// 
    /// # 返回
    /// 
    /// 编码后的字节数组
    pub fn encode_i64(value: i64) -> Result<Vec<u8>> {
        let zigzag_value = Self::zigzag_encode(value);
        Self::encode_u64(zigzag_value)
    }

    /// 解码 LEB128 格式的无符号整数
    /// 
    /// # 参数
    /// 
    /// * `data` - 要解码的字节数据
    /// 
    /// # 返回
    /// 
    /// 解码后的值和消费的字节数
    pub fn decode_u64(data: &[u8]) -> Result<(u64, usize)> {
        let mut buf = data;
        leb128::read::unsigned(&mut buf)
            .map_err(|e| UnivError::deserialization_error(format!("LEB128解码失败: {}", e)))
            .map(|value| (value, data.len() - buf.len()))
    }

    /// 解码 ZigZag + LEB128 格式的有符号整数
    /// 
    /// # 参数
    /// 
    /// * `data` - 要解码的字节数据
    /// 
    /// # 返回
    /// 
    /// 解码后的值和消费的字节数
    pub fn decode_i64(data: &[u8]) -> Result<(i64, usize)> {
        let (zigzag_value, bytes_read) = Self::decode_u64(data)?;
        let value = Self::zigzag_decode(zigzag_value);
        Ok((value, bytes_read))
    }

    /// ZigZag 编码（将有符号整数编码为无符号整数）
    /// 
    /// # 参数
    /// 
    /// * `value` - 要编码的有符号整数
    /// 
    /// # 返回
    /// 
    /// ZigZag 编码后的无符号整数
    pub fn zigzag_encode(value: i64) -> u64 {
        ((value << 1) ^ (value >> 63)) as u64
    }

    /// ZigZag 解码（将无符号整数解码为有符号整数）
    /// 
    /// # 参数
    /// 
    /// * `value` - 要解码的无符号整数
    /// 
    /// # 返回
    /// 
    /// ZigZag 解码后的有符号整数
    pub fn zigzag_decode(value: u64) -> i64 {
        ((value >> 1) as i64) ^ (-((value & 1) as i64))
    }

    /// 估算编码后的字节长度
    /// 
    /// # 参数
    /// 
    /// * `value` - 要编码的无符号整数
    /// 
    /// # 返回
    /// 
    /// 估算的字节长度
    pub fn encoded_length_u64(value: u64) -> usize {
        if value == 0 {
            return 1;
        }
        
        let mut length = 0;
        let mut v = value;
        while v > 0 {
            length += 1;
            v >>= 7;
        }
        length
    }

    /// 估算有符号整数编码后的字节长度
    /// 
    /// # 参数
    /// 
    /// * `value` - 要编码的有符号整数
    /// 
    /// # 返回
    /// 
    /// 估算的字节长度
    pub fn encoded_length_i64(value: i64) -> usize {
        let zigzag_value = Self::zigzag_encode(value);
        Self::encoded_length_u64(zigzag_value)
    }
}

/// 可变长整数数组编解码器
pub struct VarIntArray;

impl VarIntArray {
    /// 编码无符号整数数组
    /// 
    /// # 参数
    /// 
    /// * `values` - 要编码的无符号整数数组
    /// 
    /// # 返回
    /// 
    /// 编码后的字节数组
    pub fn encode_u64_array(values: &[u64]) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        
        // 首先编码数组长度
        leb128::write::unsigned(&mut buf, values.len() as u64)
            .map_err(|e| UnivError::serialization_error(format!("数组长度编码失败: {}", e)))?;
        
        // 然后编码每个值
        for &value in values {
            leb128::write::unsigned(&mut buf, value)
                .map_err(|e| UnivError::serialization_error(format!("数组元素编码失败: {}", e)))?;
        }
        
        Ok(buf)
    }

    /// 解码无符号整数数组
    /// 
    /// # 参数
    /// 
    /// * `data` - 要解码的字节数据
    /// 
    /// # 返回
    /// 
    /// 解码后的值数组和消费的字节数
    pub fn decode_u64_array(data: &[u8]) -> Result<(Vec<u64>, usize)> {
        let mut buf = data;
        let mut total_bytes_read = 0;
        
        // 解码数组长度
        let mut length_buf = buf;
        let length = leb128::read::unsigned(&mut length_buf)
            .map_err(|e| UnivError::deserialization_error(format!("数组长度解码失败: {}", e)))?;
        let bytes_read = buf.len() - length_buf.len();
        buf = length_buf;
        total_bytes_read += bytes_read;
        
        let length = length as usize;
        let mut values = Vec::with_capacity(length);
        
        // 解码每个值
        for _ in 0..length {
            let mut value_buf = buf;
            let value = leb128::read::unsigned(&mut value_buf)
                .map_err(|e| UnivError::deserialization_error(format!("数组元素解码失败: {}", e)))?;
            values.push(value);
            let consumed = buf.len() - value_buf.len();
            buf = value_buf;
            total_bytes_read += consumed;
        }
        
        Ok((values, total_bytes_read))
    }

    /// 编码有符号整数数组（使用ZigZag编码）
    /// 
    /// # 参数
    /// 
    /// * `values` - 要编码的有符号整数数组
    /// 
    /// # 返回
    /// 
    /// 编码后的字节数组
    pub fn encode_i64_array(values: &[i64]) -> Result<Vec<u8>> {
        let zigzag_values: Vec<u64> = values.iter()
            .map(|&v| VarInt::zigzag_encode(v))
            .collect();
        Self::encode_u64_array(&zigzag_values)
    }

    /// 解码有符号整数数组（使用ZigZag解码）
    /// 
    /// # 参数
    /// 
    /// * `data` - 要解码的字节数据
    /// 
    /// # 返回
    /// 
    /// 解码后的值数组和消费的字节数
    pub fn decode_i64_array(data: &[u8]) -> Result<(Vec<i64>, usize)> {
        let (zigzag_values, bytes_read) = Self::decode_u64_array(data)?;
        let values: Vec<i64> = zigzag_values.iter()
            .map(|&v| VarInt::zigzag_decode(v))
            .collect();
        Ok((values, bytes_read))
    }

    /// 估算数组编码后的字节长度
    /// 
    /// # 参数
    /// 
    /// * `values` - 要编码的无符号整数数组
    /// 
    /// # 返回
    /// 
    /// 估算的字节长度
    pub fn estimated_encoded_length_u64(values: &[u64]) -> usize {
        let mut length = VarInt::encoded_length_u64(values.len() as u64);
        for &value in values {
            length += VarInt::encoded_length_u64(value);
        }
        length
    }
}

/// 写入可变长无符号整数到写入器
/// 
/// # 参数
/// 
/// * `writer` - 数据写入器
/// * `value` - 要写入的值
/// 
/// # 返回
/// 
/// 写入结果
pub fn write_varint<W: std::io::Write>(writer: &mut W, value: u64) -> crate::error::Result<()> {
    leb128::write::unsigned(writer, value)
        .map(|_| ()) // Discard the number of bytes written, just return Ok(())
        .map_err(|e| crate::error::UnivError::serialization_error(format!("可变长整数写入失败: {}", e)))
}

/// 从读取器读取可变长无符号整数
/// 
/// # 参数
/// 
/// * `reader` - 数据读取器
/// 
/// # 返回
/// 
/// 读取的值
pub fn read_varint<R: std::io::Read>(reader: &mut R) -> crate::error::Result<u64> {
    leb128::read::unsigned(reader)
        .map_err(|e| crate::error::UnivError::deserialization_error(format!("可变长整数读取失败: {}", e)))
}

/// ZigZag 编码（公开版本）
/// 
/// # 参数
/// 
/// * `value` - 要编码的有符号整数
/// 
/// # 返回
/// 
/// ZigZag 编码后的无符号整数
pub fn zigzag_encode(value: i64) -> u64 {
    VarInt::zigzag_encode(value)
}

/// ZigZag 解码（公开版本）
/// 
/// # 参数
/// 
/// * `value` - 要解码的无符号整数
/// 
/// # 返回
/// 
/// ZigZag 解码后的有符号整数
pub fn zigzag_decode(value: u64) -> i64 {
    VarInt::zigzag_decode(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_u64_encoding() {
        let test_values = vec![0, 1, 127, 128, 255, 256, 16383, 16384, u64::MAX];
        
        for &value in &test_values {
            let encoded = VarInt::encode_u64(value).unwrap();
            let (decoded, bytes_read) = VarInt::decode_u64(&encoded).unwrap();
            
            assert_eq!(value, decoded);
            assert_eq!(bytes_read, encoded.len());
        }
    }

    #[test]
    fn test_varint_i64_encoding() {
        let test_values = vec![0, 1, -1, 127, -127, 128, -128, i64::MAX, i64::MIN];
        
        for &value in &test_values {
            let encoded = VarInt::encode_i64(value).unwrap();
            let (decoded, bytes_read) = VarInt::decode_i64(&encoded).unwrap();
            
            assert_eq!(value, decoded);
            assert_eq!(bytes_read, encoded.len());
        }
    }

    #[test]
    fn test_zigzag_encoding() {
        let test_cases = vec![
            (0, 0),
            (-1, 1),
            (1, 2),
            (-2, 3),
            (2, 4),
            (i64::MAX, u64::MAX - 1),
            (i64::MIN, u64::MAX),
        ];
        
        for (signed, expected_unsigned) in test_cases {
            let encoded = VarInt::zigzag_encode(signed);
            assert_eq!(encoded, expected_unsigned);
            
            let decoded = VarInt::zigzag_decode(encoded);
            assert_eq!(decoded, signed);
        }
    }

    #[test]
    fn test_varint_array_encoding() {
        let test_values = vec![0, 1, 127, 128, 16383, 16384];
        
        let encoded = VarIntArray::encode_u64_array(&test_values).unwrap();
        let (decoded, bytes_read) = VarIntArray::decode_u64_array(&encoded).unwrap();
        
        assert_eq!(test_values, decoded);
        assert_eq!(bytes_read, encoded.len());
    }

    #[test]
    fn test_varint_i64_array_encoding() {
        let test_values = vec![0, -1, 1, -127, 127, -128, 128];
        
        let encoded = VarIntArray::encode_i64_array(&test_values).unwrap();
        let (decoded, bytes_read) = VarIntArray::decode_i64_array(&encoded).unwrap();
        
        assert_eq!(test_values, decoded);
        assert_eq!(bytes_read, encoded.len());
    }

    #[test]
    fn test_encoded_length_estimation() {
        let test_cases = vec![
            (0, 1),
            (127, 1),
            (128, 2),
            (16383, 2),
            (16384, 3),
        ];
        
        for (value, expected_length) in test_cases {
            let estimated = VarInt::encoded_length_u64(value);
            let actual = VarInt::encode_u64(value).unwrap().len();
            
            assert_eq!(estimated, expected_length);
            assert_eq!(actual, expected_length);
        }
    }

    #[test]
    fn test_empty_array() {
        let empty_array: Vec<u64> = Vec::new();
        
        let encoded = VarIntArray::encode_u64_array(&empty_array).unwrap();
        let (decoded, bytes_read) = VarIntArray::decode_u64_array(&encoded).unwrap();
        
        assert_eq!(empty_array, decoded);
        assert_eq!(bytes_read, encoded.len());
        assert_eq!(encoded.len(), 1); // 只有长度字段
    }
}