use thiserror::Error; 


// Creating some generic and common error types and instances

/// Errors produced by the Caraxes internal core
#[derive(Debug, Error, )]
pub enum Error {
    /// A required API key is missing
    #[error("missing API key: {0}")]
    MissingApiKey(String),

    /// A config value is invalid or wrong
    #[error("invalid configuration: {0}")]
    Config(String),

    /// the http request itself failed
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Provider ans with non -success status code
    #[error("API returned status {status}:{body}")]
    Api{status: u16, body:String},

    /// body couldn't be decoded
    #[error("failed to decode response: {0}")]
    Decode(#[from] serde_json::Error),

    /// provider returned response with no choices
    #[error("provider returned an empty response")]
    EmptyResponse,
}

/// alias so call can write `Result<T>`.
pub type Result<T> = std::result::Result<T, Error>;