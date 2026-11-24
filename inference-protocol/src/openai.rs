//! Shared OpenAI Chat Completions decode helpers.
//!
//! Both dialect shims translate *from* OpenAI Chat Completions — the wire
//! shape OpenRouter (our sole upstream on the `/v1/*` surface) speaks. This
//! module owns the two decode halves the shims share:
//!
//! * [`OpenAiSseParser`] — a small SSE line-buffer that turns streamed
//!   `chat.completion.chunk` events into an ordered stream of semantic
//!   [`ChatEvent`]s, so each dialect only re-encodes events, never re-parses
//!   SSE framing.
//! * [`ChatCompletion`] + [`parse_chat_completion`] — the non-streaming
//!   single-object decode.
//!
//! Neither half touches token accounting: [`crate::proxy::recording_body`]
//! meters usage off the *upstream* OpenAI bytes before they reach any
//! translator, so the numbers a shim emits to the client are cosmetic and
//! never feed billing.

use serde_json::Value;

/// Why the model stopped, normalised across the OpenAI `finish_reason` strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FinishReason {
    Stop,
    ToolCalls,
    Length,
}

impl FinishReason {
    pub(super) fn from_openai(reason: &str) -> Self {
        match reason {
            "tool_calls" | "function_call" => FinishReason::ToolCalls,
            "length" | "max_tokens" => FinishReason::Length,
            _ => FinishReason::Stop,
        }
    }

    /// Anthropic Messages `stop_reason` spelling.
    pub(super) fn anthropic(self) -> &'static str {
        match self {
            FinishReason::Stop => "end_turn",
            FinishReason::ToolCalls => "tool_use",
            FinishReason::Length => "max_tokens",
        }
    }
}

/// Token counts as reported by the upstream `usage` block. Every field is
/// optional — a usage-silent upstream leaves them all `None`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct UsageTokens {
    pub(super) input: Option<i64>,
    pub(super) output: Option<i64>,
    pub(super) total: Option<i64>,
}

impl UsageTokens {
    fn from_json(usage: &Value) -> Self {
        let get = |keys: &[&str]| -> Option<i64> {
            keys.iter()
                .find_map(|k| usage.get(*k).and_then(Value::as_i64))
        };
        Self {
            input: get(&["prompt_tokens", "input_tokens"]),
            output: get(&["completion_tokens", "output_tokens"]),
            total: get(&["total_tokens"]),
        }
    }

    fn is_empty(self) -> bool {
        self.input.is_none() && self.output.is_none() && self.total.is_none()
    }
}

/// A single semantic event decoded from an OpenAI streaming response.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum ChatEvent {
    /// A fragment of assistant text.
    TextDelta(String),
    /// A new tool call opened at `index` (OpenAI's per-choice tool index).
    ToolCallStart {
        index: i64,
        id: String,
        name: String,
    },
    /// A fragment of the JSON arguments for the tool call at `index`.
    ToolCallArgs { index: i64, fragment: String },
    /// The choice finished.
    Finish(FinishReason),
    /// A usage block (may arrive in its own trailing chunk).
    Usage(UsageTokens),
}

/// The most an upstream may send without ending an event. Past it the
/// stream is failed rather than buffered on: an upstream that never sends a
/// blank line would otherwise grow the buffer until memory runs out. Chat
/// chunks are a few hundred bytes; this is room for any real one.
pub(super) const MAX_SSE_EVENT_BYTES: usize = 1024 * 1024;

/// Incremental parser for an OpenAI Chat Completions SSE stream.
///
/// Splits on the SSE event delimiter (a blank line, with lines ended by
/// `\n`, `\r\n` or `\r` as the SSE spec allows), pulls the `data:` payload of
/// each event, and decodes it to zero or more [`ChatEvent`]s. Partial events
/// are buffered (as bytes, so a UTF-8 character split across chunks is
/// whole again) until their delimiter arrives, so a chunk boundary anywhere
/// in the byte stream is handled correctly. More than
/// [`MAX_SSE_EVENT_BYTES`] without a delimiter [overflows](Self::overflowed)
/// the parser: the buffer is dropped and nothing more is decoded.
#[derive(Default)]
pub(super) struct OpenAiSseParser {
    buffer: Vec<u8>,
    /// Where the scan for a delimiter stopped, and where the line it
    /// stopped in began, so each byte is looked at once.
    scanned: usize,
    line_start: usize,
    overflowed: bool,
}

impl OpenAiSseParser {
    pub(super) fn feed(&mut self, chunk: &[u8]) -> Vec<ChatEvent> {
        let mut out = Vec::new();
        if self.overflowed {
            return out;
        }
        self.buffer.extend_from_slice(chunk);
        let mut consumed = 0;
        while let Some((event_end, next)) = self.next_event(consumed) {
            decode_event(
                &String::from_utf8_lossy(&self.buffer[consumed..event_end]),
                &mut out,
            );
            consumed = next;
            self.scanned = next;
            self.line_start = next;
        }
        self.buffer.drain(..consumed);
        self.scanned -= consumed;
        self.line_start -= consumed;
        if self.buffer.len() > MAX_SSE_EVENT_BYTES {
            self.overflowed = true;
            self.buffer = Vec::new();
            self.scanned = 0;
            self.line_start = 0;
        }
        out
    }

    /// The end of the event that starts at `from`, and where the next one
    /// starts, when its blank line has arrived.
    fn next_event(&mut self, from: usize) -> Option<(usize, usize)> {
        let buf = &self.buffer;
        let mut i = self.scanned.max(from);
        let mut line_start = self.line_start.max(from);
        while i < buf.len() {
            let eol = match buf[i] {
                b'\n' => 1,
                b'\r' => match buf.get(i + 1) {
                    Some(b'\n') => 2,
                    Some(_) => 1,
                    // A `\r` last: wait to see whether a `\n` follows.
                    None => break,
                },
                _ => {
                    i += 1;
                    continue;
                }
            };
            if i == line_start && line_start > from {
                // An empty line after the event's last one.
                self.scanned = i;
                self.line_start = line_start;
                return Some((line_start, i + eol));
            }
            i += eol;
            line_start = i;
        }
        self.scanned = i;
        self.line_start = line_start;
        None
    }

    /// Whether more than [`MAX_SSE_EVENT_BYTES`] arrived without an event
    /// ending: the stream is to be failed.
    pub(super) fn overflowed(&self) -> bool {
        self.overflowed
    }

    pub(super) fn finish(&mut self) -> Vec<ChatEvent> {
        let mut out = Vec::new();
        let rest = std::mem::take(&mut self.buffer);
        (self.scanned, self.line_start) = (0, 0);
        if !self.overflowed && !rest.trim_ascii().is_empty() {
            decode_event(&String::from_utf8_lossy(&rest), &mut out);
        }
        out
    }
}

fn decode_event(event: &str, out: &mut Vec<ChatEvent>) {
    // Concatenate the `data:` lines of one SSE event (OpenAI uses a single
    // data line per event, but be tolerant of multi-line data).
    let mut payload = String::new();
    for line in event.split(['\r', '\n']) {
        let line = line.trim_start();
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if data == "[DONE]" || data.is_empty() {
                continue;
            }
            if !payload.is_empty() {
                payload.push('\n');
            }
            payload.push_str(data);
        }
    }
    if payload.is_empty() {
        return;
    }
    let Ok(json) = serde_json::from_str::<Value>(&payload) else {
        return;
    };
    decode_chunk(&json, out);
}

/// Decode one `chat.completion.chunk` JSON object into ordered events.
fn decode_chunk(json: &Value, out: &mut Vec<ChatEvent>) {
    if let Some(choices) = json.get("choices").and_then(Value::as_array) {
        for choice in choices {
            let delta = choice.get("delta").unwrap_or(&Value::Null);

            if let Some(text) = delta.get("content").and_then(Value::as_str)
                && !text.is_empty()
            {
                out.push(ChatEvent::TextDelta(text.to_owned()));
            }

            if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
                for tc in tool_calls {
                    let index = tc.get("index").and_then(Value::as_i64).unwrap_or(0);
                    if let Some(id) = tc.get("id").and_then(Value::as_str) {
                        let name = tc
                            .get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        out.push(ChatEvent::ToolCallStart {
                            index,
                            id: id.to_owned(),
                            name,
                        });
                    }
                    if let Some(args) = tc
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(Value::as_str)
                        && !args.is_empty()
                    {
                        out.push(ChatEvent::ToolCallArgs {
                            index,
                            fragment: args.to_owned(),
                        });
                    }
                }
            }

            if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                out.push(ChatEvent::Finish(FinishReason::from_openai(reason)));
            }
        }
    }

    if let Some(usage) = json.get("usage").filter(|u| !u.is_null()) {
        let tokens = UsageTokens::from_json(usage);
        if !tokens.is_empty() {
            out.push(ChatEvent::Usage(tokens));
        }
    }
}

/// A tool call decoded from a non-streaming chat completion.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ToolCall {
    pub(super) id: String,
    pub(super) name: String,
    /// Parsed arguments object (falls back to `{}` on malformed JSON).
    pub(super) input: Value,
}

/// A fully decoded non-streaming chat completion.
#[derive(Debug, Clone)]
pub(super) struct ChatCompletion {
    pub(super) id: String,
    pub(super) model: String,
    pub(super) text: String,
    pub(super) tool_calls: Vec<ToolCall>,
    pub(super) finish_reason: FinishReason,
    pub(super) usage: UsageTokens,
}

/// Decode a non-streaming OpenAI Chat Completions object.
pub(super) fn parse_chat_completion(v: &Value) -> ChatCompletion {
    let id = v.get("id").and_then(Value::as_str).unwrap_or("").to_owned();
    let model = v
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();

    let choice = v
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first());
    let message = choice.and_then(|c| c.get("message"));

    let text = message
        .and_then(|m| m.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();

    let tool_calls = message
        .and_then(|m| m.get("tool_calls"))
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .filter_map(|tc| {
                    let id = tc.get("id").and_then(Value::as_str)?.to_owned();
                    let func = tc.get("function")?;
                    let name = func.get("name").and_then(Value::as_str)?.to_owned();
                    let input = func
                        .get("arguments")
                        .and_then(Value::as_str)
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        .unwrap_or_else(|| serde_json::json!({}));
                    Some(ToolCall { id, name, input })
                })
                .collect()
        })
        .unwrap_or_default();

    let finish_reason = choice
        .and_then(|c| c.get("finish_reason"))
        .and_then(Value::as_str)
        .map(FinishReason::from_openai)
        .unwrap_or(FinishReason::Stop);

    let usage = v
        .get("usage")
        .filter(|u| !u.is_null())
        .map(UsageTokens::from_json)
        .unwrap_or_default();

    ChatCompletion {
        id,
        model,
        text,
        tool_calls,
        finish_reason,
        usage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(sse: &str) -> Vec<ChatEvent> {
        let mut p = OpenAiSseParser::default();
        let mut out = p.feed(sse.as_bytes());
        out.extend(p.finish());
        out
    }

    #[test]
    fn parses_text_delta_stream() {
        let sse = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
                   data: [DONE]\n\n";
        let events = feed_all(sse);
        assert_eq!(
            events,
            vec![
                ChatEvent::TextDelta("Hel".into()),
                ChatEvent::TextDelta("lo".into()),
                ChatEvent::Finish(FinishReason::Stop),
            ]
        );
    }

    #[test]
    fn parses_crlf_and_cr_delimited_events() {
        let text = |t: &str| {
            format!(
                "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{t}\"}},\"finish_reason\":null}}]}}"
            )
        };
        let expected = vec![
            ChatEvent::TextDelta("a".into()),
            ChatEvent::TextDelta("b".into()),
            ChatEvent::TextDelta("c".into()),
        ];
        for eol in ["\r\n", "\r", "\n"] {
            let sse = format!(
                ": comment{eol}{}{eol}{eol}{}{eol}{eol}{}{eol}{eol}data: [DONE]{eol}{eol}",
                text("a"),
                text("b"),
                text("c")
            );
            // Whole, and one byte at a time (a `\r\n` split across chunks).
            let mut p = OpenAiSseParser::default();
            let mut events = p.feed(sse.as_bytes());
            assert!(p.finish().is_empty());
            assert_eq!(events, expected, "{eol:?}");
            let mut p = OpenAiSseParser::default();
            events = sse.as_bytes().chunks(1).flat_map(|b| p.feed(b)).collect();
            events.extend(p.finish());
            assert_eq!(events, expected, "{eol:?} byte by byte");
        }
        // Mixed endings, and events delivered before the stream ends.
        let mut p = OpenAiSseParser::default();
        let sse = format!("{}\r\n\n{}\n\r\n", text("a"), text("b"));
        assert_eq!(p.feed(sse.as_bytes()), expected[..2]);
    }

    #[test]
    fn a_character_split_across_chunks_is_whole() {
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"h\u{e9}llo\"},\"finish_reason\":null}]}\n\n";
        let mut p = OpenAiSseParser::default();
        let events: Vec<_> = sse.as_bytes().chunks(1).flat_map(|b| p.feed(b)).collect();
        assert_eq!(events, vec![ChatEvent::TextDelta("h\u{e9}llo".into())]);
    }

    #[test]
    fn an_event_that_never_ends_overflows_instead_of_growing() {
        let mut p = OpenAiSseParser::default();
        let head = p.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\n\ndata: \"");
        assert_eq!(head, vec![ChatEvent::TextDelta("ok".into())]);
        let filler = vec![b'a'; 64 * 1024];
        let mut fed = 0;
        while !p.overflowed() {
            assert!(p.feed(&filler).is_empty());
            fed += filler.len();
            assert!(
                fed <= MAX_SSE_EVENT_BYTES + filler.len(),
                "never overflowed"
            );
        }
        assert!(p.buffer.capacity() == 0, "the buffer is let go");
        // Nothing more is read, even a well-formed event.
        assert!(
            p.feed(b"\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\n")
                .is_empty()
        );
        assert!(p.finish().is_empty());
        // Up to the cap is fine.
        let mut p = OpenAiSseParser::default();
        let big = format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{}\"}},\"finish_reason\":null}}]}}",
            "b".repeat(MAX_SSE_EVENT_BYTES - 100)
        );
        for chunk in big.as_bytes().chunks(4096) {
            assert!(p.feed(chunk).is_empty());
        }
        assert!(!p.overflowed());
        assert_eq!(p.feed(b"\n\n").len(), 1);
    }

    #[test]
    fn parses_split_chunk_boundary() {
        let mut p = OpenAiSseParser::default();
        let a = p.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"Hi");
        assert!(a.is_empty(), "no complete event yet");
        let b = p.feed(b"\"},\"finish_reason\":null}]}\n\n");
        assert_eq!(b, vec![ChatEvent::TextDelta("Hi".into())]);
    }

    #[test]
    fn parses_tool_call_and_usage() {
        let sse = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"x\\\":1}\"}}]},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n\
                   data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n\n\
                   data: [DONE]\n\n";
        let events = feed_all(sse);
        assert_eq!(
            events[0],
            ChatEvent::ToolCallStart {
                index: 0,
                id: "call_1".into(),
                name: "bash".into()
            }
        );
        assert_eq!(
            events[1],
            ChatEvent::ToolCallArgs {
                index: 0,
                fragment: "{\"x\":1}".into()
            }
        );
        assert_eq!(events[2], ChatEvent::Finish(FinishReason::ToolCalls));
        assert_eq!(
            events[3],
            ChatEvent::Usage(UsageTokens {
                input: Some(10),
                output: Some(5),
                total: Some(15)
            })
        );
    }

    #[test]
    fn parses_non_streaming_completion() {
        let v = serde_json::json!({
            "id": "chatcmpl-1",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hi there"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5}
        });
        let c = parse_chat_completion(&v);
        assert_eq!(c.text, "Hi there");
        assert_eq!(c.finish_reason, FinishReason::Stop);
        assert_eq!(c.usage.input, Some(3));
        assert_eq!(c.usage.output, Some(2));
        assert!(c.tool_calls.is_empty());
    }

    #[test]
    fn parses_non_streaming_tool_call() {
        let v = serde_json::json!({
            "id": "chatcmpl-2",
            "model": "gpt-4o",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_9",
                        "type": "function",
                        "function": {"name": "lookup", "arguments": "{\"q\":\"a\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let c = parse_chat_completion(&v);
        assert_eq!(c.text, "");
        assert_eq!(c.finish_reason, FinishReason::ToolCalls);
        assert_eq!(c.tool_calls.len(), 1);
        assert_eq!(c.tool_calls[0].id, "call_9");
        assert_eq!(c.tool_calls[0].name, "lookup");
        assert_eq!(c.tool_calls[0].input, serde_json::json!({"q": "a"}));
    }
}
