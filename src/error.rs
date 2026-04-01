use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Invalid proxy configuration: {0}")]
    InvalidProxy(String),

    #[error("Invalid combo format: {0}")]
    InvalidCombo(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("Thread error: {0}")]
    Thread(String),

    #[error("No check function provided")]
    NoCheckFunction,

    #[error("No combos provided")]
    NoCombos,

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}
