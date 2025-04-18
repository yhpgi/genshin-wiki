use thiserror::Error;
use tokio::task::JoinError;

#[derive(Error, Debug, Clone)]
pub enum AppError {
    #[error("HTTP request failed: {0}")]
    Reqwest(String),
    #[error("Filesystem I/O error: {0}")]
    Io(String),
    #[error("JSON serialization error: {0}")]
    SerdeSerialize(String),
    #[error("JSON parsing error: {0}")]
    SerdeParse(String),
    #[error("API returned an error: retcode={retcode}, message='{message}' (Endpoint: {endpoint}, Lang: {lang})")]
    ApiError {
        retcode: i64,
        message: String,
        endpoint: String,
        lang: String,
    },
    #[error("API response structure invalid: {message} (Endpoint: {endpoint}, Lang: {lang})")]
    ApiResponseInvalid {
        message: String,
        endpoint: String,
        lang: String,
    },
    #[error("Data transformation error: {0}")]
    TransformError(String),
    #[error("HTML parsing error: {0}")]
    HtmlParseError(String),
    #[error("Invalid argument provided: {0}")]
    Argument(String),
    #[error("Tokio task join error: {0}")]
    JoinError(String),
    #[error("Timeout during operation: {0}")]
    Timeout(String),
    #[error("Recursion depth limit ({limit}) reached during {context}")]
    RecursionLimit { context: String, limit: u32 },
    #[error("Hex decoding error: {0}")]
    HexDecode(String),
    #[error("Color parsing error: {0}")]
    ColorParse(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Semaphore acquisition error: {0}")]
    SemaphoreAcquire(String),
    #[error("Unexpected internal error: {0}")]
    Unexpected(String),
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Reqwest(e.to_string())
    }
}
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}
impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        if e.is_io() || e.is_eof() || e.is_syntax() {
            AppError::SerdeParse(e.to_string())
        } else {
            AppError::SerdeSerialize(e.to_string())
        }
    }
}
impl From<JoinError> for AppError {
    fn from(e: JoinError) -> Self {
        AppError::JoinError(e.to_string())
    }
}
impl From<hex::FromHexError> for AppError {
    fn from(e: hex::FromHexError) -> Self {
        AppError::HexDecode(e.to_string())
    }
}

impl From<csscolorparser::ParseColorError> for AppError {
    fn from(e: csscolorparser::ParseColorError) -> Self {
        AppError::ColorParse(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn response_invalid<S: Into<String>>(message: S, endpoint: &str, lang: &str) -> AppError {
        AppError::ApiResponseInvalid {
            message: message.into(),
            endpoint: endpoint.to_string(),
            lang: lang.to_string(),
        }
    }
    pub fn api_error<S: Into<String>>(
        retcode: i64,
        message: S,
        endpoint: &str,
        lang: &str,
    ) -> AppError {
        AppError::ApiError {
            retcode,
            message: message.into(),
            endpoint: endpoint.to_string(),
            lang: lang.to_string(),
        }
    }

    pub fn from_serde_parse(e: serde_json::Error, context: &str) -> AppError {
        AppError::TransformError(format!(
            "Serde parse error during transformation ({}): {}",
            context, e
        ))
    }
}
