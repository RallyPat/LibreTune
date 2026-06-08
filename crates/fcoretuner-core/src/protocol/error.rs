//! 协议错误

use thiserror::Error;

/// 协议通信中可能发生的错误
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("串口错误: {0}")]
    SerialError(String),

    #[error("连接超时")]
    Timeout,

    #[error("未连接到 ECU")]
    NotConnected,

    #[error("已连接")]
    AlreadyConnected,

    #[error("ECU 响应无效")]
    InvalidResponse,

    #[error("CRC 不匹配: 期望 {expected:#010x}，实际 {actual:#010x}")]
    CrcMismatch { expected: u32, actual: u32 },

    #[error("签名不匹配: 期望 '{expected}'，实际 '{actual}'")]
    SignatureMismatch { expected: String, actual: String },

    #[error("ECU 返回错误代码: {0}")]
    EcuError(u8),

    #[error("缓冲区溢出: 数据包过大")]
    BufferOverflow,

    #[error("协议错误: {0}")]
    ProtocolError(String),

    #[error("未找到端口: {0}")]
    PortNotFound(String),

    #[error("I/O 错误: {0}")]
    IoError(#[from] std::io::Error),
}
