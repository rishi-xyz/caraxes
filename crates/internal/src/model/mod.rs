pub mod openai;
pub mod provider;

use std::{env, time::Duration};

use tracing::debug;

use crate::error::{Error::MissingApiKey, Result};
use crate::model::openai::OpenAICompatibleProvider;
use crate::model::provider::ModelProvider;

const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
const DEFAULT_MODEL: &str = "nvidia/nemotron-3-ultra-550b-a55b:free";
const DEFAULT_API_KEY_ENV: &str = "OPENROUTER_API_KEY";
const DEFAULT_TIMEOUT_SECS: u64 = 60;

pub enum ProviderKind {
    OpenAICompatible,
}

pub struct ModelConfig {
    pub provider: ProviderKind,
    pub base_url: String,
    pub model: String,
    api_key_env: String,
    /// How long a single request will take before it timeouts and aborted
    pub timeout: Duration,
}

fn timeout_from_env() -> Duration {
    let secs = env::var("CARAXES_TIMEOUT_SECS")
        .ok()
        .and_then(|time_str| time_str.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

impl ModelConfig {
    pub fn from_env() -> Self {
        ModelConfig {
            provider: ProviderKind::OpenAICompatible,
            base_url: env::var("CARAXES_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string()),
            model: env::var("CARAXES_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            api_key_env: env::var("CARAXES_API_KEY_ENV")
                .unwrap_or_else(|_| DEFAULT_API_KEY_ENV.to_string()),
            timeout: timeout_from_env(),
        }
    }
}

pub fn build_provider(config: &ModelConfig) -> Result<Box<dyn ModelProvider>> {
    debug!(
        model = %config.model,
        base_url = %config.base_url,
        timeout_secs = config.timeout.as_secs(),
        "building model provider"
    );

    match config.provider {
        ProviderKind::OpenAICompatible => {
            let api_key = env::var(&config.api_key_env)
                .map_err(|_| MissingApiKey(config.api_key_env.clone()))?;
            Ok(Box::new(
                OpenAICompatibleProvider::new(config.base_url.clone(), api_key)
                    .with_timeout(config.timeout),
            ))
        }
    }
}
