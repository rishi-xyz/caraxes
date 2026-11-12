use std::env;

use reqwest::Error;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

use crate::error::Error::EmptyResponse;
use crate::{
    error::{
        Error::{Config, MissingApiKey},
        Result,
    },
    types::{ChatMessages, ChatRequest, ChatResponse, Role},
};

const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
const DEFAULT_MODEL: &str = "nvidia/nemotron-3-ultra-550b-a55b:free";

pub struct ModelConfig {
    base_url: String,
    model: String,
}

impl ModelConfig {
    pub fn from_env() -> Self {
        let base_url: String =
            env::var("BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let model: String = env::var("BASE_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        ModelConfig { base_url, model }
    }
}

pub async fn llm_call(prompt: &str) -> Result<String> {
    let config = ModelConfig::from_env();
    
    let api_key = env::var("API_KEY").map_err(|_| MissingApiKey(format!("API key")))?;
    
    let auth_value = HeaderValue::from_str(&format!("Bearer {api_key}"))
        .map_err(|err| Config(format!("invalid api key: {err}")))?;
    
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, auth_value);

    let payload = ChatRequest::new(config.model, vec![ChatMessages::new(Role::User, prompt)]);

    let url = format!("{}/chat/completions", config.base_url.trim_end_matches("/"));

    let res = reqwest::Client::new()
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
    
    let chat_response: ChatResponse = serde_json::from_str(&body)?;

    let choice = chat_response
        .choices
        .into_iter()
        .next()
        .ok_or(EmptyResponse)?;
    
    Ok(choice.message.content.unwrap_or_default())
}
