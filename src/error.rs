#[derive(thiserror::Error, Debug)]
pub enum GstaldergeistError {
    #[error("Telegram error: {0}")]
    TelegramError(teloxide::RequestError),

    #[error("Network error: {0}")]
    NetworkError(reqwest::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Date parsing error: {0}")]
    DateError(chrono::ParseError),

    #[error("Pdf extract error: {0}")]
    PdfExtract(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

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

impl From<teloxide::RequestError> for GstaldergeistError {
    fn from(error: teloxide::RequestError) -> Self {
        GstaldergeistError::TelegramError(error)
    }
}

impl From<reqwest::Error> for GstaldergeistError {
    fn from(error: reqwest::Error) -> Self {
        GstaldergeistError::NetworkError(error)
    }
}

impl From<chrono::ParseError> for GstaldergeistError {
    fn from(error: chrono::ParseError) -> Self {
        GstaldergeistError::DateError(error)
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