//! Open AI compactible adapter
//!
//! All provider specific wire details
//! rest of caraxes only sees [`crate::model::provider`] types
//!
//! You can find most of the things in doc and can use postman for sending a requesting and seeing the response format
//!
//! all of these can also be replaced by a open ai compatiable rust sdk but still choosing to implement this for gain control over the flow.

use bytes::Bytes;
use std::{
    pin::Pin,
    time::Duration,
};

use futures::{Stream, StreamExt, stream::try_unfold};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{
    error::{
        Error::{self, Cancelled, Config, EmptyResponse}, Result,
    }, model::provider::{ModelProvider, ModelRequest, ModelResponse, ModelStream, StreamEvent}, types::{FinishReason, Message, Usage},
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
    request_timeout: Duration
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
    async fn send(&self, payload: &WireRequest<'_>) -> Result<reqwest::Response> {
        let url = self.endpoint();
        let fut = self
            .client
            .post(&url)
            .headers(self.auth_headers()?)
            .json(payload)
            .send();

        let res = tokio::time::timeout(self.request_timeout, fut).await.map_err(|_| self.timeout_error())??;

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

    async fn complete(&self, request: ModelRequest, cancel: CancellationToken) -> Result<ModelResponse> {
        let auth_value = HeaderValue::from_str(&format!("Bearer {}", self.api_key))
            .map_err(|err| Config(format!("invalid API key: {err}")))?;

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, auth_value);

        let payload = WireRequest {
            model: &request.model,
            messages: &request.messages,
            stream: false,
            stream_options: None,
        };

        let res  = tokio::select! {
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

    async fn stream(&self, request: ModelRequest, cancel: CancellationToken) -> Result<ModelStream> {
        let payload = WireRequest {
            model: &request.model,
            messages: &request.messages,
            stream: true,
            stream_options: Some(WireStreamingOptions {
                include_usage: true,
            }),
        };

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
struct WireRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    stream: bool,
    stream_options: Option<WireStreamingOptions>,
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

#[derive(Debug, Serialize, Deserialize)]
struct WireMessage {
    content: Option<String>,
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

/// State carried between steps of the SSE decoding stream
struct StreamState<S> {
    bytes: Pin<Box<S>>,
    cancel: CancellationToken,
    /// bytes recieved but not yet split into complete line, holds buffers
    buffer: Vec<u8>,
    finish_reason: Option<FinishReason>,
    usage: Option<Usage>,
    /// bytes reached end of file
    ended: bool,
    /// the terminal `Done` event emitted
    emitted_done: bool,
}

/// Decode a stream of http chunks into a stream of ['StreamEvent'].
///
/// reads only `data` fields
fn event_stream<S>(bytes: S, cancel:CancellationToken) -> impl Stream<Item = Result<StreamEvent>>
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
                            if let Some(content) = choice.delta.content
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
    fn finish(&mut self) -> StreamEvent {
        StreamEvent::Done {
            usage: self.usage,
            finish_reason: self.finish_reason.take(),
        }
    }
}
