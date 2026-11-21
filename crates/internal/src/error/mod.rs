//! Creating some generic and common error types and instances

use thiserror::Error;

/// Errors produced by the Caraxes internal core
#[derive(Debug, Error)]
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
    Api { status: u16, body: String },

    /// body couldn't be decoded
    #[error("failed to decode response: {0}")]
    Decode(#[from] serde_json::Error),

    /// provider returned response with no choices
    #[error("provider returned an empty response")]
    EmptyResponse,

    /// A provider adapter failed in a way that has no more specific variant
    #[error("provider error: {0}")]
    Provider(String),

    /// The request exceeded its configured timeout
    #[error("request timed out after {secs}s")]
    Timeout { secs: u64 },

    /// The request was cancelled before it completed
    #[error("request cancelled")]
    Cancelled,

    /// model supplied a abosulte path, we use workspace realative model //NOTE! : might need to change this for containers / pods later on.
    #[error("absolute paths are not allowed: {0}")]
    AbsolutePath(String),

    /// The underlying fs or process op failed
    /// An os system error for input output
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    ///path outside the workspace
    #[error("path escapes the workspace: {0}")]
    Escape(String),
}

/// alias so call can write `Result<T>`.
pub type Result<T> = std::result::Result<T, Error>;
