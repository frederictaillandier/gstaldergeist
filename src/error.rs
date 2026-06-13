#[derive(thiserror::Error, Debug)]
pub enum GstaldergeistError {
    #[error("Telegram error: {0}")]
    TelegramError(#[from] teloxide::RequestError),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Date parsing error: {0}")]
    DateError(#[from] chrono::ParseError),

    #[error("Pdf extract error: {0}")]
    PdfExtract(String),

    #[error("Other error: {0}")]
    Other(String),
}

// These conversions collapse the source error into a String message, which
// `#[from]` can't express, so they stay explicit.
impl From<std::io::Error> for GstaldergeistError {
    fn from(error: std::io::Error) -> Self {
        GstaldergeistError::Other(error.to_string())
    }
}

impl From<serde_json::Error> for GstaldergeistError {
    fn from(error: serde_json::Error) -> Self {
        GstaldergeistError::Other(error.to_string())
    }
}

impl From<regex::Error> for GstaldergeistError {
    fn from(error: regex::Error) -> Self {
        GstaldergeistError::Other(error.to_string())
    }
}

impl From<lopdf::Error> for GstaldergeistError {
    fn from(error: lopdf::Error) -> Self {
        GstaldergeistError::PdfExtract(error.to_string())
    }
}

impl From<rusqlite::Error> for GstaldergeistError {
    fn from(error: rusqlite::Error) -> Self {
        GstaldergeistError::Other(error.to_string())
    }
}
