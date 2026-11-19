//! The model port: the abstraction every backend implements
//!
//! Callers depends on [`ModelProvider`] and
//!

use crate::error::Result;
use crate::tools::ToolDefinition;
use crate::types::{FinishReason, Message, ToolCall, Usage};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

pub struct ModelRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
}

impl ModelRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        ModelRequest {
            model: model.into(),
            messages,
            tools: Vec::new(),
        }
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) ->Self {
        self.tools = tools;
        self
    }
}

pub struct ModelResponse {
    /// The assistant text or response, absent when the model produced no content
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
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
        tool_calls: Vec<ToolCall>,
    },
}

/// A provider neutral boxed stream of model events.
pub type ModelStream = Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>;

/// trait ModelProvider
///
/// the port every model backend implements
///
/// `Send + Sync` cuz a provider will be shared across tokio tasks once supervisor exists.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// short stable identifier for backend
    fn name(&self) -> &str;

    /// perofrm a single non-streaming completion
    async fn complete(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelResponse>;

    /// Start a streaming completion
    /// yields [`StreamEvent`] until terminal `Done` event.
    async fn stream(&self, request: ModelRequest, cancel: CancellationToken)
    -> Result<ModelStream>;
}
