//! Error types for INI parsing

use thiserror::Error;

/// Errors that can occur during INI parsing
#[derive(Error, Debug)]
pub enum IniError {
    #[error("I/O error: {0}")]
    IoError(String),

    #[error("Parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("Missing required section: [{0}]")]
    MissingSectionError(String),

    #[error("Missing required field '{field}' in section [{section}]")]
    MissingFieldError { section: String, field: String },

    #[error("Invalid value for '{field}': {message}")]
    InvalidValueError { field: String, message: String },

    #[error("Unknown data type: {0}")]
    UnknownDataType(String),

    #[error("Expression parse error: {0}")]
    ExpressionError(String),

    #[error("Include error: circular reference detected for '{0}'")]
    CircularInclude(String),

    #[error("Include error: file not found '{0}'")]
    IncludeNotFound(String),

    #[error("Include error: maximum depth ({0}) exceeded")]
    IncludeDepthExceeded(usize),
}
