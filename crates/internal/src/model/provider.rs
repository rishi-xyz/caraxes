//! The model port: the abstraction every backend implements
//! 
//! Callers depends on [`ModelProvider`] and
//! 

use std::pin::Pin;
use crate::types::{FinishReason, Message, Usage};
use crate::error::Result;
use async_trait::async_trait;
use futures::Stream;

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

/// Streaming model response
pub enum StreamEvent {
    /// assistant text
    Token(String),
    /// last event, Always the last item by a stream
    Done {
        usage: Option<Usage>,
        finish_reason: Option<FinishReason>,
    }
}

/// A provider neutral boxed stream of model events.
pub type ModelStream = Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>;

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

    /// Start a streaming completion
    /// yields [`StreamEvent`] until terminal `Done` event.
    async fn stream(&self, request: ModelRequest) -> Result<ModelStream>;
}