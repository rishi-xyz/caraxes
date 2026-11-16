pub mod openai;
pub mod provider;

use std::env;

use crate::error::{
    Error::MissingApiKey,
    Result,
};
use crate::model::openai::OpenAICompatibleProvider;
use crate::model::provider::ModelProvider;

const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
const DEFAULT_MODEL: &str = "nvidia/nemotron-3-ultra-550b-a55b:free";
const DEFAULT_API_KEY_ENV: &str = "OPENROUTER_API_KEY";

pub enum ProviderKind {
    OpenAICompatible,
}

pub struct ModelConfig {
    pub provider: ProviderKind,
    pub base_url: String,
    pub model: String,
    api_key_env: String,
}

impl ModelConfig {
    pub fn from_env() -> Self {
        ModelConfig {
            provider: ProviderKind::OpenAICompatible,
            base_url: env::var("CARAXES_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string()),
            model: env::var("CARAXES_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            api_key_env: env::var("CARAXES_API_KEY_ENV")
                .unwrap_or_else(|_| DEFAULT_API_KEY_ENV.to_string()),
        }
    }
}

pub fn build_provider(config: &ModelConfig) -> Result<Box<dyn ModelProvider>> {
    match config.provider {
        ProviderKind::OpenAICompatible => {
            let api_key = env::var(&config.api_key_env)
                .map_err(|_| MissingApiKey(config.api_key_env.clone()))?;
            Ok(Box::new(OpenAICompatibleProvider::new(
                config.base_url.clone(),
                api_key,
            )))
        }
    }
}
