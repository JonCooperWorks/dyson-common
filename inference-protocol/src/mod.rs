//! Request/response dialect shims for the customer-facing `/v1/*` inference
//! API.
//!
//! OpenRouter — the sole upstream on the `api.<hostname>` surface — speaks only
//! OpenAI Chat Completions. To sell the same catalogue through the Anthropic
//! Messages SDK (`/v1/messages`) and the OpenAI Responses SDK
//! (`/v1/responses`), this module translates at the edge:
//!
//! 1. **Inbound** ([`Dialect::translate_request`]): the client's dialect
//!    request JSON → a Chat Completions body, done in
//!    the hosting application's HTTP adapter *before* the hosting application's upstream pipeline runs, so the
//!    balance gate, policy, budget reservation, model routing, audit row and
//!    frozen-margin billing all apply unchanged.
//!
//! 2. **Outbound** ([`ResponseTranslator`] + [`TranslatingStream`]): the
//!    upstream Chat Completions response (single JSON object or streamed
//!    `chat.completion.chunk` SSE) → the client's dialect.
//!
//! ## Billing invariant
//!
//! Usage metering (the hosting application's metering body) reads the
//! **upstream OpenAI bytes** to count tokens. The dialect translator is a
//! *downstream* layer: `forward_core` wraps the recording body first (usage is
//! captured off the untranslated OpenAI stream) and only then wraps the
//! translator ([`TranslatingStream`]) around it. So translation can never
//! perturb the numbers that bill — it only re-encodes the client-facing copy.

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::Stream;
use serde_json::Value;

pub mod model_routing;
mod surface;
pub use surface::{
    InferenceOperation, InferenceRoute, classify_route, dialect_for_path, qualify_model,
    supported_routes,
};

pub mod anthropic;
mod openai;
pub mod responses;

use openai::{OpenAiSseParser, parse_chat_completion};

/// A customer-facing request dialect the `/v1/*` surface can translate to and
/// from OpenAI Chat Completions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// Anthropic Messages (`POST /v1/messages`).
    Anthropic,
    /// OpenAI Responses (`POST /v1/responses`).
    Responses,
}

impl Dialect {
    /// Translate a client request body into an OpenAI Chat Completions body.
    /// Returns a human-readable error string on a malformed / unsupported
    /// request (rendered into [`Self::request_error_body`] by the caller).
    pub fn translate_request(self, body: &Value) -> Result<Value, String> {
        match self {
            Dialect::Anthropic => anthropic::translate_request(body),
            Dialect::Responses => responses::translate_request(body),
        }
    }

    /// Dialect-specific error envelope for a request-translation failure, so a
    /// stock SDK surfaces a clean `.error.message`.
    pub fn request_error_body(self, message: &str) -> Value {
        error_envelope(self, "invalid_request_error", message)
    }

    /// The dialect's own name for an error class. The two SDK families spell
    /// the same conditions differently and each one's client code branches on
    /// its own spelling, so a shared `kind` string would be wrong for one of
    /// them. `Kind` is the neutral middle the callers speak.
    pub fn error_type(self, kind: ErrorKind) -> &'static str {
        match (self, kind) {
            (Dialect::Anthropic, ErrorKind::RateLimit) => "rate_limit_error",
            (Dialect::Responses, ErrorKind::RateLimit) => "rate_limit_exceeded",
            (Dialect::Anthropic, ErrorKind::Permission) => "permission_error",
            (Dialect::Responses, ErrorKind::Permission) => "permission_denied",
            (Dialect::Anthropic, ErrorKind::Authentication) => "authentication_error",
            (Dialect::Responses, ErrorKind::Authentication) => "invalid_api_key",
            (_, ErrorKind::InvalidRequest) => "invalid_request_error",
            (Dialect::Anthropic, ErrorKind::NotFound) => "not_found_error",
            (Dialect::Responses, ErrorKind::NotFound) => "not_found",
            (Dialect::Anthropic, ErrorKind::Billing) => "billing_error",
            (Dialect::Responses, ErrorKind::Billing) => "insufficient_quota",
            (Dialect::Anthropic, ErrorKind::Upstream) => "api_error",
            (Dialect::Responses, ErrorKind::Upstream) => "server_error",
        }
    }
}

/// Error classes the `/v1/*` surface reports, named independently of any one
/// SDK's vocabulary. [`Dialect::error_type`] maps each to the wire string the
/// requesting SDK expects; [`error_body`] renders the whole envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    RateLimit,
    Permission,
    Authentication,
    InvalidRequest,
    NotFound,
    Billing,
    Upstream,
}

impl ErrorKind {
    /// The OpenAI Chat-Completions spelling — what the unprefixed and
    /// `/openai` paths return, since they carry no translating dialect.
    pub fn openai_type(self) -> &'static str {
        match self {
            ErrorKind::RateLimit => "rate_limit_exceeded",
            ErrorKind::Permission => "permission_denied",
            ErrorKind::Authentication => "invalid_api_key",
            ErrorKind::InvalidRequest => "invalid_request_error",
            ErrorKind::NotFound => "not_found",
            ErrorKind::Billing => "insufficient_quota",
            ErrorKind::Upstream => "server_error",
        }
    }
}

/// The error envelope for a client that may or may not be speaking a shim
/// dialect. `None` is the native OpenAI surface (no translation layer), so it
/// gets the OpenAI shape — which is also the shape the Anthropic SDK would
/// choke on, hence the split.
///
/// Every error the customer API emits goes through here, so a stock SDK can
/// always read `.error.message` no matter which of the three base URLs it was
/// pointed at.
pub fn error_body(dialect: Option<Dialect>, kind: ErrorKind, message: &str) -> Value {
    error_body_with_code(dialect, kind, None, message)
}

/// [`error_body`] plus a specific `error.code`.
///
/// `type` has to be one of the handful of strings the SDKs know, which throws
/// away the precise reason — "insufficient_quota" covers both an exhausted
/// prepaid balance and a per-key monthly cap, and a caller wants to tell those
/// apart. `code` is where OpenAI's own error object puts exactly that
/// distinction, so our closed-enum denial codes live there: SDKs classify on
/// `type`, integrators branch on `code`, and neither has to guess.
pub fn error_body_with_code(
    dialect: Option<Dialect>,
    kind: ErrorKind,
    code: Option<&str>,
    message: &str,
) -> Value {
    let mut body = match dialect {
        Some(d) => error_envelope(d, d.error_type(kind), message),
        None => serde_json::json!({
            "error": {"message": message, "type": kind.openai_type()},
        }),
    };
    if let Some(code) = code
        && let Some(err) = body.get_mut("error").and_then(Value::as_object_mut)
    {
        err.insert("code".to_owned(), Value::String(code.to_owned()));
    }
    body
}

/// Build the dialect's error envelope. Anthropic uses a top-level
/// `{"type":"error", ...}`; Responses/OpenAI use a bare `{"error": {...}}`.
fn error_envelope(dialect: Dialect, kind: &str, message: &str) -> Value {
    match dialect {
        Dialect::Anthropic => serde_json::json!({
            "type": "error",
            "error": {"type": kind, "message": message},
        }),
        Dialect::Responses => serde_json::json!({
            "error": {"type": kind, "message": message},
        }),
    }
}

/// Re-wrap an upstream error body in our own envelope, keeping only the
/// message.
///
/// The upstream is OpenRouter, whose error bodies carry sibling keys we must
/// not relay — most notably `user_id`, the operator's OpenRouter account id.
/// Relaying the raw body leaked that to every customer who mistyped a model
/// name. Keeping only `error.message` also means the customer sees one error
/// shape whichever base URL they used, instead of OpenRouter's shape on the
/// native path and ours on the shims.
/// The class is taken from the HTTP status rather than the upstream's own
/// `type` string, which is neither stable nor in our vocabulary.
pub fn sanitize_upstream_error(dialect: Option<Dialect>, status: u16, raw: &[u8]) -> Vec<u8> {
    let value: Value = serde_json::from_slice(raw).unwrap_or(Value::Null);
    let (_, message) = extract_openai_error(&value);
    error_body(dialect, upstream_error_kind(status), &message)
        .to_string()
        .into_bytes()
}

/// Classify an upstream HTTP status into our error vocabulary. An upstream
/// 401/403 means *our* platform credential was rejected — that is a server-side
/// fault from the customer's point of view, never an authentication problem
/// with the `sk_` key they sent, so it deliberately lands on `Upstream`.
fn upstream_error_kind(status: u16) -> ErrorKind {
    match status {
        400 | 422 => ErrorKind::InvalidRequest,
        402 => ErrorKind::Billing,
        404 => ErrorKind::NotFound,
        429 => ErrorKind::RateLimit,
        _ => ErrorKind::Upstream,
    }
}

/// Re-encoder for the outgoing response stream. One of these is built per
/// request once the upstream status + streaming mode are known.
pub struct ResponseTranslator {
    encoder: Box<dyn Encoder>,
}

impl ResponseTranslator {
    /// Construct the translator for a given dialect.
    ///
    /// * `streaming` — the client asked for `stream:true` (upstream is SSE).
    /// * `status` — the upstream's HTTP status. On a non-2xx the body is an
    ///   error object, so we always buffer it and re-wrap it as the dialect's
    ///   error envelope regardless of `streaming`.
    /// * `model` — the requested model id, echoed into the streamed
    ///   `message_start` / `response.created` framing (the upstream's own
    ///   model field isn't available until the first chunk).
    pub fn build(dialect: Dialect, streaming: bool, status: u16, model: &str) -> Self {
        Self::build_with_request(dialect, streaming, status, model, &Value::Null)
    }

    pub fn build_with_request(
        dialect: Dialect,
        streaming: bool,
        status: u16,
        model: &str,
        request: &Value,
    ) -> Self {
        let tool_names = responses::ToolNames::from_request(request);
        let encoder: Box<dyn Encoder> = if !(200..300).contains(&status) {
            Box::new(ErrorEncoder::new(Some(dialect), status))
        } else if streaming {
            match dialect {
                Dialect::Anthropic => Box::new(AnthropicStreamEncoder::new(model)),
                Dialect::Responses => Box::new(ResponsesStreamEncoder::new(model, tool_names)),
            }
        } else {
            Box::new(CompletionEncoder::new(dialect, tool_names))
        };
        Self { encoder }
    }

    /// An error-only translator for the **native OpenAI** customer path, which
    /// has no dialect to translate into and so never built a translator at all.
    /// Success bodies still stream through untouched; this exists purely so a
    /// non-2xx gets our envelope rather than the upstream's — the same
    /// protection the shim dialects have always had by accident of translation.
    pub fn errors_only(status: u16) -> Self {
        Self {
            encoder: Box::new(ErrorEncoder::new(None, status)),
        }
    }

    fn feed(&mut self, chunk: &[u8]) -> Vec<u8> {
        self.encoder.feed(chunk)
    }

    fn finish(&mut self) -> Vec<u8> {
        self.encoder.finish()
    }
}

/// Re-encodes upstream OpenAI bytes into a target dialect. `feed` handles a
/// streamed chunk (may buffer and return nothing); `finish` flushes any
/// trailing/buffered output at end-of-stream.
trait Encoder: Send {
    fn feed(&mut self, chunk: &[u8]) -> Vec<u8>;
    fn finish(&mut self) -> Vec<u8>;
}

struct AnthropicStreamEncoder {
    parser: OpenAiSseParser,
    state: anthropic::StreamState,
    finished: bool,
}

impl AnthropicStreamEncoder {
    fn new(model: &str) -> Self {
        Self {
            parser: OpenAiSseParser::default(),
            state: anthropic::StreamState::new(model.to_owned()),
            finished: false,
        }
    }
}

impl Encoder for AnthropicStreamEncoder {
    fn feed(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut out = String::new();
        for e in self.parser.feed(chunk) {
            self.state.handle(e, &mut out);
        }
        out.into_bytes()
    }

    fn finish(&mut self) -> Vec<u8> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        let mut out = String::new();
        for e in self.parser.finish() {
            self.state.handle(e, &mut out);
        }
        self.state.finish_stream(&mut out);
        out.into_bytes()
    }
}

struct ResponsesStreamEncoder {
    parser: OpenAiSseParser,
    state: responses::StreamState,
    finished: bool,
    tool_names: responses::ToolNames,
}

impl ResponsesStreamEncoder {
    fn new(model: &str, tool_names: responses::ToolNames) -> Self {
        Self {
            parser: OpenAiSseParser::default(),
            state: {
                let mut state = responses::StreamState::new(model.to_owned());
                state.tool_names = tool_names.clone();
                state
            },
            finished: false,
            tool_names,
        }
    }
}

impl Encoder for ResponsesStreamEncoder {
    fn feed(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut out = String::new();
        for mut e in self.parser.feed(chunk) {
            if let openai::ChatEvent::ToolCallStart { name, .. } = &mut e {
                self.tool_names.restore(name);
            }
            self.state.handle(e, &mut out);
        }
        out.into_bytes()
    }

    fn finish(&mut self) -> Vec<u8> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        let mut out = String::new();
        for mut e in self.parser.finish() {
            if let openai::ChatEvent::ToolCallStart { name, .. } = &mut e {
                self.tool_names.restore(name);
            }
            self.state.handle(e, &mut out);
        }
        self.state.finish_stream(&mut out);
        out.into_bytes()
    }
}

/// Buffers a non-streaming success body and re-encodes the single chat
/// completion object into the target dialect at `finish`.
struct CompletionEncoder {
    dialect: Dialect,
    buf: Vec<u8>,
    finished: bool,
    tool_names: responses::ToolNames,
}

impl CompletionEncoder {
    fn new(dialect: Dialect, tool_names: responses::ToolNames) -> Self {
        Self {
            dialect,
            buf: Vec::new(),
            finished: false,
            tool_names,
        }
    }
}

impl Encoder for CompletionEncoder {
    fn feed(&mut self, chunk: &[u8]) -> Vec<u8> {
        self.buf.extend_from_slice(chunk);
        Vec::new()
    }

    fn finish(&mut self) -> Vec<u8> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        let value: Value = serde_json::from_slice(&self.buf).unwrap_or(Value::Null);
        let mut completion = parse_chat_completion(&value);
        if self.dialect == Dialect::Responses {
            for call in &mut completion.tool_calls {
                self.tool_names.restore(&mut call.name);
            }
        }
        let out = match self.dialect {
            Dialect::Anthropic => anthropic::completion_to_json(&completion),
            Dialect::Responses => {
                responses::completion_to_json_with_tools(&completion, &self.tool_names)
            }
        };
        out.to_string().into_bytes()
    }
}

/// Buffers an error body (any status, streaming or not) and re-emits it in our
/// envelope. `dialect: None` is the native OpenAI surface.
///
/// Deliberately keeps only the upstream's `error.message` — see
/// [`sanitize_upstream_error`] for why the rest must not be relayed.
struct ErrorEncoder {
    dialect: Option<Dialect>,
    status: u16,
    buf: Vec<u8>,
    finished: bool,
}

impl ErrorEncoder {
    fn new(dialect: Option<Dialect>, status: u16) -> Self {
        Self {
            dialect,
            status,
            buf: Vec::new(),
            finished: false,
        }
    }
}

impl Encoder for ErrorEncoder {
    fn feed(&mut self, chunk: &[u8]) -> Vec<u8> {
        self.buf.extend_from_slice(chunk);
        Vec::new()
    }

    fn finish(&mut self) -> Vec<u8> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        sanitize_upstream_error(self.dialect, self.status, &self.buf)
    }
}

/// Pull `{type, message}` out of an OpenAI-style error body, with fallbacks.
fn extract_openai_error(value: &Value) -> (String, String) {
    let err = value.get("error");
    let kind = err
        .and_then(|e| e.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("upstream_error")
        .to_owned();
    let message = err
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| "upstream request failed".to_owned());
    (kind, message)
}

/// Stream adapter that re-encodes the recording body's OpenAI bytes into the
/// client's dialect. It sits *downstream* of
/// the hosting application's metering body, so usage metering has already
/// seen the untranslated bytes (see the module-level billing invariant).
pub struct TranslatingStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>,
    translator: ResponseTranslator,
    done: bool,
}

impl TranslatingStream {
    pub fn new<S>(inner: S, translator: ResponseTranslator) -> Self
    where
        S: Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
    {
        Self {
            inner: Box::pin(inner),
            translator,
            done: false,
        }
    }
}

impl Stream for TranslatingStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if this.done {
                return Poll::Ready(None);
            }
            match this.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(chunk))) => {
                    let out = this.translator.feed(&chunk);
                    if out.is_empty() {
                        // Buffered (non-streaming) encoders swallow chunks and
                        // flush at finish; keep pulling the upstream.
                        continue;
                    }
                    return Poll::Ready(Some(Ok(Bytes::from(out))));
                }
                Poll::Ready(Some(Err(e))) => {
                    this.done = true;
                    return Poll::Ready(Some(Err(e)));
                }
                Poll::Ready(None) => {
                    this.done = true;
                    let out = this.translator.finish();
                    if out.is_empty() {
                        return Poll::Ready(None);
                    }
                    return Poll::Ready(Some(Ok(Bytes::from(out))));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[test]
    fn responses_restore_only_unambiguous_declared_mcp_names() {
        for streaming in [false, true] {
            for (declared, emitted, expected) in [
                (
                    vec!["mcp__beyond-openapi__get_listing"],
                    "mcp__beyond_openapi__get_listing",
                    "mcp__beyond-openapi__",
                ),
                (
                    vec!["mcp__beyond-openapi__get_listing"],
                    "mcp__beyond-openapi__get_listing",
                    "mcp__beyond-openapi__",
                ),
                (
                    vec!["mcp__beyond-openapi__get_listing"],
                    "mcp__other__get_listing",
                    "mcp__other__",
                ),
                (
                    vec![
                        "mcp__beyond-openapi__get_listing",
                        "mcp__beyond_openapi__get_listing",
                    ],
                    "mcp__beyond_openapi__get_listing",
                    "mcp__beyond_openapi__",
                ),
                (
                    vec!["mcp__a-b_c__get_listing", "mcp__a_b-c__get_listing"],
                    "mcp__a_b_c__get_listing",
                    "mcp__a_b_c__",
                ),
            ] {
                let request = serde_json::json!({"tools": declared.iter().map(|name| serde_json::json!({"type":"function", "function":{"name":name}})).collect::<Vec<_>>()});
                let mut translator = ResponseTranslator::build_with_request(
                    Dialect::Responses,
                    streaming,
                    200,
                    "m",
                    &request,
                );
                let call = serde_json::json!({"index":0,"id":"call_1","type":"function","function":{"name":emitted,"arguments":"{\"path\":{\"listing_search_id\":-1}}"}});
                let raw = if streaming {
                    format!(
                        "data: {}\n\ndata: [DONE]\n\n",
                        serde_json::json!({"choices":[{"delta":{"tool_calls":[call]},"finish_reason":"tool_calls"}]})
                    )
                } else {
                    serde_json::json!({"id":"c","model":"m","choices":[{"message":{"content":"","tool_calls":[call]},"finish_reason":"tool_calls"}]}).to_string()
                };
                // Exercise arbitrary network chunk boundaries too.
                let mut output = Vec::new();
                for chunk in raw.as_bytes().chunks(7) {
                    output.extend(translator.feed(chunk));
                }
                output.extend(translator.finish());
                let output = String::from_utf8(output).unwrap();
                let result: Value = if streaming {
                    output
                        .lines()
                        .filter_map(|line| line.strip_prefix("data: "))
                        .filter_map(|data| serde_json::from_str::<Value>(data).ok())
                        .find(|event| event["type"] == "response.completed")
                        .unwrap()["response"]
                        .clone()
                } else {
                    serde_json::from_str(&output).unwrap()
                };
                let tool = result["output"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|item| item["type"] == "function_call")
                    .unwrap();
                assert_eq!(tool["namespace"], expected, "{streaming}: {emitted}");
                assert_eq!(tool["name"], "get_listing");
                assert_eq!(tool["arguments"], "{\"path\":{\"listing_search_id\":-1}}");
            }
        }
    }

    #[test]
    fn responses_preserves_original_tool_declarations_in_all_wire_events() {
        for streaming in [false, true] {
            for (namespace, name) in [
                (Some("mcp__beyond_openapi"), "list_user_listings"),
                (Some("mcp__beyond-openapi__"), "list_user_listings"),
                (Some("multi_agent_v1"), "wait_agent"),
                (None, "mcp__beyond_openapi__list_user_listings"),
                (None, "plain__tool"),
            ] {
                let tool = serde_json::json!({"type":"function", "name":name,"parameters":{"type":"object"}});
                let declaration = match namespace {
                    Some(ns) => serde_json::json!({"type":"namespace","name":ns,"tools":[tool]}),
                    None => tool,
                };
                let original = serde_json::json!({"model":"m","input":"read saved listings","tools":[declaration]});
                let chat = Dialect::Responses.translate_request(&original).unwrap();
                let flat = chat["tools"][0]["function"]["name"].as_str().unwrap();
                let emitted = flat.replace('-', "_");
                let call = serde_json::json!({"index":0,"id":"call_1","type":"function","function":{"name":emitted,"arguments":"{}"}});
                let raw = if streaming {
                    format!(
                        "data: {}\n\ndata: [DONE]\n\n",
                        serde_json::json!({"choices":[{"delta":{"tool_calls":[call]},"finish_reason":"tool_calls"}]})
                    )
                } else {
                    serde_json::json!({"id":"c","model":"m","choices":[{"message":{"tool_calls":[call]},"finish_reason":"tool_calls"}]}).to_string()
                };
                let mut translator = ResponseTranslator::build_with_request(
                    Dialect::Responses,
                    streaming,
                    200,
                    "m",
                    &original,
                );
                let mut output = Vec::new();
                for chunk in raw.as_bytes().chunks(7) {
                    output.extend(translator.feed(chunk));
                }
                output.extend(translator.finish());
                let output = String::from_utf8(output).unwrap();
                let values: Vec<Value> = if streaming {
                    output
                        .lines()
                        .filter_map(|line| line.strip_prefix("data: "))
                        .filter_map(|line| serde_json::from_str(line).ok())
                        .collect()
                } else {
                    vec![serde_json::from_str(&output).unwrap()]
                };
                let mut checked = 0;
                for value in values {
                    let calls: Vec<&Value> = if let Some(item) = value.get("item") {
                        vec![item]
                    } else {
                        value
                            .pointer("/response/output")
                            .or_else(|| value.get("output"))
                            .and_then(Value::as_array)
                            .map(|items| items.iter().collect())
                            .unwrap_or_default()
                    };
                    for call in calls {
                        if call["type"] != "function_call" {
                            continue;
                        }
                        assert_eq!(call["namespace"].as_str(), namespace, "{output}");
                        assert_eq!(call["name"], name, "{output}");
                        checked += 1;
                    }
                }
                assert!(checked >= if streaming { 3 } else { 1 });
            }
        }
    }

    /// End-to-end through the public translator: an OpenAI SSE text stream fed
    /// as upstream bytes comes out as Anthropic Messages SSE, proving the
    /// `feed`/`finish` wiring and the stream adapter agree.
    #[tokio::test]
    async fn translating_stream_reencodes_openai_to_anthropic_sse() {
        let chunks = vec![
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n"
                .as_bytes()
                .to_vec(),
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
             data: [DONE]\n\n"
                .as_bytes()
                .to_vec(),
        ];
        let inner = futures::stream::iter(
            chunks
                .into_iter()
                .map(|c| Ok::<Bytes, std::io::Error>(Bytes::from(c))),
        );
        let translator = ResponseTranslator::build(Dialect::Anthropic, true, 200, "m");
        let stream = TranslatingStream::new(inner, translator);
        let bytes: Vec<u8> = stream
            .map(|r| r.unwrap())
            .collect::<Vec<_>>()
            .await
            .concat();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("event: message_start"));
        assert!(text.contains("event: content_block_delta"));
        assert!(text.contains("\"text\":\"Hi\""));
        assert!(text.contains("event: message_stop"));
    }

    /// Non-streaming: a single OpenAI completion object buffered and re-encoded
    /// to a single Anthropic Messages object.
    #[tokio::test]
    async fn translating_stream_reencodes_non_streaming_completion() {
        let body = serde_json::json!({
            "id": "chatcmpl-1",
            "model": "m",
            "choices": [{"message": {"content": "yo"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1}
        })
        .to_string();
        let inner = futures::stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::from(
            body.into_bytes(),
        ))]);
        let translator = ResponseTranslator::build(Dialect::Anthropic, false, 200, "m");
        let stream = TranslatingStream::new(inner, translator);
        let bytes: Vec<u8> = stream
            .map(|r| r.unwrap())
            .collect::<Vec<_>>()
            .await
            .concat();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["type"], "message");
        assert_eq!(value["content"][0]["text"], "yo");
        assert_eq!(value["stop_reason"], "end_turn");
    }

    /// A mistyped model name used to come back as OpenRouter's raw body, whose
    /// top-level `user_id` is the *operator's* upstream account id. Only the
    /// message may survive.
    #[test]
    fn sanitize_upstream_error_drops_everything_but_the_message() {
        let raw = serde_json::json!({
            "error": {"message": "nope/model is not a valid model ID", "code": 400},
            "user_id": "user_3BGgMlV4QdIpRqJBHJukfpbymrP",
        })
        .to_string();
        for dialect in [None, Some(Dialect::Anthropic), Some(Dialect::Responses)] {
            let out = sanitize_upstream_error(dialect, 400, raw.as_bytes());
            let text = String::from_utf8(out.clone()).unwrap();
            assert!(!text.contains("user_id"), "{dialect:?}: {text}");
            assert!(!text.contains("user_3BG"), "{dialect:?}: {text}");
            let json: Value = serde_json::from_slice(&out).unwrap();
            assert_eq!(
                json["error"]["message"],
                "nope/model is not a valid model ID"
            );
            assert_eq!(json["error"]["type"], "invalid_request_error");
        }
    }

    /// A body we can't parse still produces a valid envelope rather than
    /// relaying whatever bytes the upstream sent.
    #[test]
    fn sanitize_upstream_error_handles_unparseable_bodies() {
        let out = sanitize_upstream_error(None, 502, b"<html>gateway down</html>");
        let json: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["error"]["type"], "server_error");
        assert!(json["error"]["message"].is_string());
    }

    /// The error class comes from the status, so a rate-limited upstream is
    /// reported to the client as a rate limit — in their SDK's vocabulary.
    #[test]
    fn upstream_status_picks_the_error_class() {
        let raw = br#"{"error":{"message":"slow down"}}"#;
        let json: Value =
            serde_json::from_slice(&sanitize_upstream_error(Some(Dialect::Anthropic), 429, raw))
                .unwrap();
        assert_eq!(json["type"], "error");
        assert_eq!(json["error"]["type"], "rate_limit_error");
        let json: Value = serde_json::from_slice(&sanitize_upstream_error(None, 429, raw)).unwrap();
        assert_eq!(json["error"]["type"], "rate_limit_exceeded");
    }

    /// The native OpenAI path has no dialect to translate into, so it never
    /// built a translator — and its errors went out raw. `errors_only` is the
    /// narrow translator that closes that gap.
    #[tokio::test]
    async fn errors_only_translator_sanitizes_the_native_path() {
        let body = serde_json::json!({
            "error": {"message": "bad model"},
            "user_id": "user_leak",
        })
        .to_string();
        let inner = futures::stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::from(
            body.into_bytes(),
        ))]);
        let stream = TranslatingStream::new(inner, ResponseTranslator::errors_only(400));
        let bytes: Vec<u8> = stream
            .map(|r| r.unwrap())
            .collect::<Vec<_>>()
            .await
            .concat();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(!text.contains("user_leak"), "{text}");
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["error"]["message"], "bad model");
        assert!(value.get("type").is_none(), "openai envelope: {value}");
    }

    /// A non-2xx upstream error body is re-wrapped as the Anthropic error
    /// envelope even when the client asked to stream.
    #[tokio::test]
    async fn translating_stream_wraps_error_body() {
        let body = serde_json::json!({
            "error": {"type": "invalid_request_error", "message": "bad model"}
        })
        .to_string();
        let inner = futures::stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::from(
            body.into_bytes(),
        ))]);
        let translator = ResponseTranslator::build(Dialect::Anthropic, true, 400, "m");
        let stream = TranslatingStream::new(inner, translator);
        let bytes: Vec<u8> = stream
            .map(|r| r.unwrap())
            .collect::<Vec<_>>()
            .await
            .concat();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["type"], "error");
        assert_eq!(value["error"]["type"], "invalid_request_error");
        assert_eq!(value["error"]["message"], "bad model");
    }
}
