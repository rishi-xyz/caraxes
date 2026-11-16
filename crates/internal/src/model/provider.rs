//! The model port: the abstraction every backend implements
//! 
//! Callers depends on [`ModelProvider`] and
//! 

use crate::types::{FinishReason, Message, Usage};
use crate::error::Result;
use async_trait::async_trait;

pub struct ModelRequest {
    pub model: String,
    pub messages: Vec<Message>,
}

pub struct ModelResponse {
    /// The assistant text or response, absent when the model produced no content
    pub content: Option<String>,
    pub usage: Option<Usage>,
    pub finish_reason: Option<FinishReason>,
}

/// trait ModelProvider
/// 
/// the port every model backend implements
/// 
/// `Send + Sync` cuz a provider will be shared across tokio tasks once supervisor exists.
#[async_trait]
pub trait ModelProvider: Send  + Sync {
    /// short stable identifier for backend
    fn name(&self) -> &str;

    /// perofrm a single non-streaming completion
    async fn complete(&self, request: ModelRequest) -> Result<ModelResponse>;
}