//! Open AI compactible adapter
//!
//! All provider specific wire details
//! rest of caraxes only sees [`crate::model::provider`] types
//!
//! You can find most of the things in doc and can use postman for sending a requesting and seeing the response format
//!
//! all of these can also be replaced by a open ai compatiable rust sdk but still choosing to implement this for gain control over the flow.

use bytes::Bytes;
use std::{collections::BTreeMap, mem, pin::Pin, time::Duration};

use futures::{Stream, StreamExt, stream::try_unfold};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{
    error::{
        Error::{self, Cancelled, Config, EmptyResponse},
        Result,
    },
    model::provider::{ModelProvider, ModelRequest, ModelResponse, ModelStream, StreamEvent},
    tools::ToolDefinition,
    types::{FinishReason, Message, Role, ToolCall, Usage},
};

/// Default request timeout
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

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
    request_timeout: Duration,
}

impl OpenAICompatibleProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        OpenAICompatibleProvider {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            request_timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Override the request timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches("/"))
    }

    fn auth_headers(&self) -> Result<HeaderMap> {
        let auth_value = HeaderValue::from_str(&format!("Bearer {}", self.api_key))
            .map_err(|err| Config(format!("invalid API key: {err}")))?;
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, auth_value);
        Ok(headers)
    }

    fn timeout_error(&self) -> Error {
        Error::Timeout {
            secs: self.request_timeout.as_secs(),
        }
    }

    /// Send a request
    ///
    /// send will be bounded by the request timeout
    async fn send(&self, payload: &WireRequest) -> Result<reqwest::Response> {
        let url = self.endpoint();
        let fut = self
            .client
            .post(&url)
            .headers(self.auth_headers()?)
            .json(payload)
            .send();

        let res = tokio::time::timeout(self.request_timeout, fut)
            .await
            .map_err(|_| self.timeout_error())??;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(crate::error::Error::Api {
                status: status.as_u16(),
                body,
            });
        }
        Ok(res)
    }
}

#[async_trait::async_trait]
impl ModelProvider for OpenAICompatibleProvider {
    fn name(&self) -> &str {
        "openai-compatible"
    }

    async fn complete(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelResponse> {
        let auth_value = HeaderValue::from_str(&format!("Bearer {}", self.api_key))
            .map_err(|err| Config(format!("invalid API key: {err}")))?;

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, auth_value);

        let payload = WireRequest::from_domain(&request, false);

        let res = tokio::select! {
            _ = cancel.cancelled() => return Err(Cancelled),
            res = self.send(&payload) => res?, // this is the new i learnt in rust [error propagation operator]
        };

        let body = tokio::select! {
            _ = cancel.cancelled() => return Err(Cancelled),
            body  = tokio::time::timeout(self.request_timeout, res.text()) => body.map_err(|_| self.timeout_error())??,
        };

        let wire: WireResponse = serde_json::from_str(&body)?;
        let response = wire.into_domain()?;
        Ok(response)
    }

    async fn stream(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelStream> {
        let payload = WireRequest::from_domain(&request, true);

        let res = tokio::select! {
            _ = cancel.cancelled() => return Err(Cancelled),
            res = self.send(&payload) => res?,
        };
        Ok(Box::pin(event_stream(res.bytes_stream(), cancel)))
    }
}

/// # WireRequest
///
/// Request body
///
/// Doing reference to avoid message conversion and clone, borrowing direcctly
#[derive(Debug, Serialize)]
struct WireRequest {
    model: String,
    messages: Vec<WireMessage>,
    #[serde(skip_serializing_if = "is_false")]
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<WireStreamingOptions>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<WireTool>,
}

impl WireRequest {
    fn from_domain(request: &ModelRequest, stream: bool) -> Self {
        WireRequest {
            model: request.model.clone(),
            messages: request.messages.iter().map(WireMessage::from).collect(),
            stream,
            stream_options: stream.then_some(WireStreamingOptions {
                include_usage: true,
            }),
            tools: request.tools.iter().map(WireTool::from).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct WireMessage {
    #[serde(default = "assitant_role")]
    role: Role,
    #[serde(default)]
    content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<WireToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl From<&Message> for WireMessage {
    fn from(message: &Message) -> Self {
        WireMessage {
            role: message.role,
            content: message.content.clone(),
            tool_calls: message.tool_calls.iter().map(WireToolCall::from).collect(),
            tool_call_id: message.tool_call_id.clone(),
        }
    }
}

fn assitant_role() -> Role {
    Role::Assistant
}

#[derive(Debug, Serialize, Deserialize)]
struct WireToolCall {
    id: String,
    kind: String,
    function: WireFunctionCall,
}

impl From<&ToolCall> for WireToolCall {
    fn from(call: &ToolCall) -> Self {
        WireToolCall {
            id: call.id.clone(),
            kind: "function".to_string(),
            function: WireFunctionCall {
                name: call.name.clone(),
                args: call.arguments.clone(),
            },
        }
    }
}

impl WireToolCall {
    fn into_domain(self) -> ToolCall {
        ToolCall {
            id: self.id,
            name: self.function.name,
            arguments: self.function.args,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct WireFunctionCall {
    name: String,
    args: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Serialize)]
struct WireStreamingOptions {
    include_usage: bool,
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

#[derive(Debug, Deserialize)]
struct WireStreamChunk {
    choices: Vec<WireStreamChoice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct WireStreamChoice {
    delta: WireDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WireDelta {
    content: Option<String>,
    tool_calls: Vec<WireToolCallDelta>,
}

#[derive(Debug, Deserialize, Default)]
struct WireToolCallDelta {
    index: u32,
    id: Option<String>,
    function: Option<WireFunctionDelta>,
}

#[derive(Debug, Deserialize, Default)]
struct WireFunctionDelta {
    name: Option<String>,
    args: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WireTool {
    #[serde(rename = "type")]
    kind: String,
    function: WireToolFunction,
}

fn tool_type() -> String {
    "function".to_string()
}

impl From<&ToolDefinition> for WireTool {
    fn from(tool: &ToolDefinition) -> Self {
        WireTool {
            kind: tool_type(),
            function: WireToolFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                params: tool.params.clone(),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct WireToolFunction {
    name: String,
    description: String,
    params: serde_json::Value,
}

impl WireResponse {
    fn into_domain(self) -> Result<ModelResponse> {
        let choice = self.choices.into_iter().next().ok_or(EmptyResponse)?;
        Ok(ModelResponse {
            content: choice.message.content,
            tool_calls: choice
                .message
                .tool_calls
                .into_iter()
                .map(WireToolCall::into_domain)
                .collect(),
            usage: self.usage,
            finish_reason: choice
                .finish_reason
                .as_deref()
                .map(FinishReason::parse_from_provider), // this is better as it never fails
        })
    }
}

/// State carried between steps of the SSE decoding stream
struct StreamState<S> {
    bytes: Pin<Box<S>>,
    cancel: CancellationToken,
    /// bytes recieved but not yet split into complete line, holds buffers
    buffer: Vec<u8>,
    finish_reason: Option<FinishReason>,
    usage: Option<Usage>,
    tool_calls: BTreeMap<u32, ToolCallBuilder>,
    /// bytes reached end of file
    ended: bool,
    /// the terminal `Done` event emitted
    emitted_done: bool,
}

#[derive(Default)]
struct ToolCallBuilder {
    id: String,
    name: String,
    args: String,
}

impl ToolCallBuilder {
    fn into_tool_calls(self) -> ToolCall {
        ToolCall::new(self.id, self.name, self.args)
    }
}

/// Decode a stream of http chunks into a stream of ['StreamEvent'].
///
/// reads only `data` fields
fn event_stream<S>(bytes: S, cancel: CancellationToken) -> impl Stream<Item = Result<StreamEvent>>
where
    S: Stream<Item = reqwest::Result<Bytes>> + Send + 'static,
{
    try_unfold(
        // returns ok some , ok none , err
        StreamState {
            bytes: Box::pin(bytes),
            cancel,
            buffer: Vec::new(),
            finish_reason: None,
            usage: None,
            tool_calls: BTreeMap::new(),
            ended: false,
            emitted_done: false,
        },
        |mut state| async move {
            // main processin loop , continously process data async until emit is over or err
            loop {
                if state.emitted_done {
                    return Ok(None);
                }

                // A complete line available -> handle it
                if let Some(newline) = state.buffer.iter().position(|&b| b == b'\n') {
                    // b'\n' in char to u8 bytes /// find the position of newline char
                    let line: Vec<u8> = state.buffer.drain(..=newline).collect(); // collect vec elem till that point
                    let line = String::from_utf8_lossy(&line); // convert to string , web search all these functions btw 
                    let line = line.trim_end_matches(['\r', '\n']); // get the line till ends \r , \n

                    if let Some(data) = line.strip_prefix("data:") {
                        // since every text starts with data we are going remove that
                        let data = data.trim_start(); // trim the data string

                        //see for done, if found emmit as true and return
                        if data == "[DONE]" {
                            //see sdk and refernces for these , was also there in postman responses at end of stream
                            state.emitted_done = true;
                            return Ok(Some((state.finish(), state)));
                        }

                        // stream chnk is coming, parse json
                        let chunk: WireStreamChunk = serde_json::from_str(data)?; //might need to look later //SEE!
                        if let Some(usage) = chunk.usage {
                            state.usage = Some(usage.into());
                        }

                        let mut token = None;
                        for choice in chunk.choices {
                            if let Some(reason) = choice.finish_reason {
                                state.finish_reason =
                                    Some(FinishReason::parse_from_provider(&reason));
                            }

                            let WireDelta {
                                content,
                                tool_calls,
                            } = choice.delta;

                            for delta in tool_calls {
                                state.absorvb_tool_call(delta);
                            }
                            if let Some(content) = content
                                && !content.is_empty()
                            {
                                // in antropic one ig we saw a empty content -> ""
                                token = Some(content);
                            }
                        }

                        if let Some(token) = token {
                            return Ok(Some((StreamEvent::Token(token), state)));
                        }
                    }
                    // No data line , keep moving
                    continue;
                }
                // No newline or complete line left in buffer
                if state.ended {
                    state.emitted_done = true;
                    return Ok(Some((state.finish(), state)));
                }

                let cancel = state.cancel.clone();
                let next = state.bytes.next();
                let item = tokio::select! {
                    _ = cancel.cancelled() => return Err(Error::Cancelled),
                    item = next => item,
                };

                match item {
                    Some(Ok(chunk)) => state.buffer.extend_from_slice(&chunk),
                    Some(Err(err)) => return Err(Error::from(err)),
                    None => state.ended = true,
                }
            }
        },
    )
}

impl<S> StreamState<S> {
    fn absorvb_tool_call(&mut self, delta: WireToolCallDelta) {
        let builder = self.tool_calls.entry(delta.index).or_default();

        if let Some(id) = delta.id {
            builder.id = id;
        }

        if let Some(fuction) = delta.function {
            if let Some(name) = fuction.name {
                builder.name.push_str(&name);
            }

            if let Some(args) = fuction.args {
                builder.args.push_str(&args);
            }
        }
    }

    fn finish(&mut self) -> StreamEvent {
        let tool_calls = mem::take(&mut self.tool_calls)
            .into_values()
            .map(ToolCallBuilder::into_tool_calls)
            .collect();
        StreamEvent::Done {
            usage: self.usage,
            finish_reason: self.finish_reason.take(),
            tool_calls,
        }
    }
}
