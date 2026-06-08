//! Error types for INI parsing

use thiserror::Error;

/// Errors that can occur during INI parsing
#[derive(Error, Debug)]
pub enum IniError {
    #[error("I/O 错误: {0}")]
    IoError(String),

    #[error("解析错误，第 {line} 行: {message}")]
    ParseError { line: usize, message: String },

    #[error("缺少必要的节: [{0}]")]
    MissingSectionError(String),

    #[error("节 [{section}] 中缺少必要字段 '{field}'")]
    MissingFieldError { section: String, field: String },

    #[error("'{field}' 的值无效: {message}")]
    InvalidValueError { field: String, message: String },

    #[error("未知数据类型: {0}")]
    UnknownDataType(String),

    #[error("表达式解析错误: {0}")]
    ExpressionError(String),

    #[error("包含错误: 检测到 '{0}'")]
    CircularInclude(String),

    #[error("包含错误: 未找到文件 '{0}'")]
    IncludeNotFound(String),

    #[error("包含错误: 超出最大深度 ({0})")]
    IncludeDepthExceeded(usize),
}
