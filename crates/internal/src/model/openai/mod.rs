//! Open AI compactible adapter
//!
//! All provider specific wire details
//! rest of caraxes only sees [`crate::model::provider`] types

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::{
    error::{
        Error::{Config, EmptyResponse},
        Result,
    },
    model::provider::{ModelProvider, ModelRequest, ModelResponse},
    types::{FinishReason, Message, Usage},
};

/// # OpenAICompatibleProvider
///
/// Adapter for any OpenAI compatible endpoint `/chat/completions`
/// (OpenRouter, OpenAI, a local server)
///
/// fn new creates a new open ai  provider instance
///
/// ```rust
/// let provider = OpenAICompatibleProvider::new(base_url: "base_url", api_key: env::var("API_KEY"))
/// ```

pub struct OpenAICompatibleProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl OpenAICompatibleProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        OpenAICompatibleProvider {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches("/"))
    }
}

#[async_trait::async_trait]
impl ModelProvider for OpenAICompatibleProvider {
    fn name(&self) -> &str {
        "openai-compatible"
    }

    async fn complete(&self, request: ModelRequest) -> Result<ModelResponse> {
        let auth_value = HeaderValue::from_str(&format!("Bearer {}", self.api_key))
            .map_err(|err| Config(format!("invalid API key: {err}")))?;

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, auth_value);

        let payload = WireRequest {
            model: &request.model,
            messages: &request.messages,
        };

        let url = self.endpoint();

        let res = self
            .client
            .post(&url)
            .headers(headers)
            .json(&payload)
            .send()
            .await?;

        let status = res.status();

        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(crate::error::Error::Api {
                status: status.as_u16(),
                body,
            });
        }

        let body = res.text().await?;
        let wire: WireResponse = serde_json::from_str(&body)?;
        let response = wire.into_domain()?;
        Ok(response)
    }
}

/// # WireRequest
///
/// Request body
///
/// Doing reference to avoid message conversion and clone, borrowing direcctly
#[derive(Debug, Serialize)]
struct WireRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
}

#[derive(Debug, Serialize, Deserialize)]
struct WireResponse {
    choices: Vec<WireChoice>,
    usage: Option<Usage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WireChoice {
    message: WireMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WireMessage {
    content: Option<String>,
}

impl WireResponse {
    fn into_domain(self) -> Result<ModelResponse> {
        let choice = self.choices.into_iter().next().ok_or(EmptyResponse)?;
        Ok(ModelResponse {
            content: choice.message.content,
            usage: self.usage,
            finish_reason: choice
                .finish_reason
                .as_deref()
                .map(FinishReason::parse_from_provider), // this is better as it never fails
        })
    }
}
