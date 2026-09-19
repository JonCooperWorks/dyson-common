//! Anthropic Messages ⇄ OpenAI Chat Completions translation.
//!
//! Inbound: an Anthropic `/v1/messages` request body → the OpenAI Chat
//! Completions body OpenRouter understands ([`translate_request`]).
//!
//! Outbound: the OpenAI response → Anthropic Messages, both the single-object
//! form ([`completion_to_json`]) and the streamed SSE event sequence
//! ([`StreamState`]). The streamed form re-frames OpenAI `delta` chunks into
//! Anthropic's `message_start` → `content_block_*` → `message_delta` →
//! `message_stop` lifecycle.

use serde_json::{Map, Value};

use super::openai::{ChatCompletion, ChatEvent, FinishReason};

/// Translate an Anthropic Messages request body into an OpenAI Chat
/// Completions request body. `model`, `max_tokens`, `temperature`, `stream`
/// and `stop_sequences` map across; `system` becomes a leading system message;
/// each Anthropic message expands to one or more OpenAI messages.
pub(crate) fn translate_request(body: &Value) -> Result<Value, String> {
    let obj = body
        .as_object()
        .ok_or("request body must be a JSON object")?;

    let model = obj
        .get("model")
        .and_then(Value::as_str)
        .ok_or("`model` is required")?;
    // Anthropic requires max_tokens; carry it through unchanged.
    let max_tokens = obj.get("max_tokens").cloned();

    let mut messages: Vec<Value> = Vec::new();

    // `system` → a leading OpenAI system message (string or text-block array).
    if let Some(system) = obj.get("system") {
        if let Some(text) = system_text(system)
            && !text.is_empty()
        {
            messages.push(serde_json::json!({"role": "system", "content": text}));
        }
    }

    let anthropic_messages = obj
        .get("messages")
        .and_then(Value::as_array)
        .ok_or("`messages` must be an array")?;
    for msg in anthropic_messages {
        translate_message(msg, &mut messages)?;
    }

    let mut out = Map::new();
    out.insert("model".into(), Value::String(model.to_owned()));
    if let Some(mt) = max_tokens {
        out.insert("max_tokens".into(), mt);
    }
    out.insert("messages".into(), Value::Array(messages));

    if let Some(stream) = obj.get("stream") {
        out.insert("stream".into(), stream.clone());
    }
    if let Some(temp) = obj.get("temperature") {
        out.insert("temperature".into(), temp.clone());
    }
    if let Some(top_p) = obj.get("top_p") {
        out.insert("top_p".into(), top_p.clone());
    }
    // Anthropic `stop_sequences` → OpenAI `stop`.
    if let Some(stop) = obj.get("stop_sequences") {
        out.insert("stop".into(), stop.clone());
    }

    if let Some(tools) = obj.get("tools").and_then(Value::as_array) {
        let translated: Vec<Value> = tools.iter().filter_map(translate_tool).collect();
        if !translated.is_empty() {
            out.insert("tools".into(), Value::Array(translated));
        }
    }
    if let Some(choice) = obj.get("tool_choice") {
        out.insert("tool_choice".into(), translate_tool_choice(choice));
    }

    Ok(Value::Object(out))
}

/// Flatten an Anthropic `system` field (string, or array of text blocks) to a
/// single string.
fn system_text(system: &Value) -> Option<String> {
    match system {
        Value::String(s) => Some(s.clone()),
        Value::Array(blocks) => {
            let mut buf = String::new();
            for b in blocks {
                if b.get("type").and_then(Value::as_str) == Some("text")
                    && let Some(t) = b.get("text").and_then(Value::as_str)
                {
                    buf.push_str(t);
                }
            }
            Some(buf)
        }
        _ => None,
    }
}

/// Expand one Anthropic message into one or more OpenAI messages. A user turn
/// carrying `tool_result` blocks emits one `role:"tool"` message per result
/// (plus a `role:"user"` message for any remaining text/image content); an
/// assistant turn with `tool_use` blocks becomes one assistant message with a
/// `tool_calls` array.
fn translate_message(msg: &Value, out: &mut Vec<Value>) -> Result<(), String> {
    let role = msg
        .get("role")
        .and_then(Value::as_str)
        .ok_or("message `role` is required")?;
    let content = msg.get("content").unwrap_or(&Value::Null);

    // Content may be a bare string.
    if let Some(text) = content.as_str() {
        out.push(serde_json::json!({"role": role, "content": text}));
        return Ok(());
    }
    let Some(blocks) = content.as_array() else {
        // Null or unexpected shape → empty message, preserved for role parity.
        out.push(serde_json::json!({"role": role, "content": ""}));
        return Ok(());
    };

    if role == "assistant" {
        let mut text = String::new();
        let mut tool_calls: Vec<Value> = Vec::new();
        for b in blocks {
            match b.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(t) = b.get("text").and_then(Value::as_str) {
                        text.push_str(t);
                    }
                }
                Some("tool_use") => {
                    let id = b.get("id").and_then(Value::as_str).unwrap_or("");
                    let name = b.get("name").and_then(Value::as_str).unwrap_or("");
                    let input = b
                        .get("input")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({}));
                    tool_calls.push(serde_json::json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": input.to_string(),
                        }
                    }));
                }
                // thinking / other blocks aren't representable in OpenAI
                // assistant history; drop them.
                _ => {}
            }
        }
        let mut m = Map::new();
        m.insert("role".into(), Value::String("assistant".into()));
        m.insert(
            "content".into(),
            if text.is_empty() {
                Value::Null
            } else {
                Value::String(text)
            },
        );
        if !tool_calls.is_empty() {
            m.insert("tool_calls".into(), Value::Array(tool_calls));
        }
        out.push(Value::Object(m));
        return Ok(());
    }

    // role == "user" (or anything else treated as user input).
    // tool_result blocks each become their own role:"tool" message; remaining
    // text/image content becomes a single user message.
    let mut user_content: Vec<Value> = Vec::new();
    for b in blocks {
        match b.get("type").and_then(Value::as_str) {
            Some("tool_result") => {
                let tool_use_id = b.get("tool_use_id").and_then(Value::as_str).unwrap_or("");
                out.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": tool_result_text(b.get("content")),
                }));
            }
            Some("text") => {
                if let Some(t) = b.get("text").and_then(Value::as_str) {
                    user_content.push(serde_json::json!({"type": "text", "text": t}));
                }
            }
            Some("image") => {
                if let Some(url) = image_data_url(b) {
                    user_content.push(serde_json::json!({
                        "type": "image_url",
                        "image_url": {"url": url}
                    }));
                }
            }
            _ => {}
        }
    }
    if !user_content.is_empty() {
        // Collapse a single text block to the string form most models prefer.
        if user_content.len() == 1
            && user_content[0].get("type").and_then(Value::as_str) == Some("text")
        {
            let text = user_content[0]
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            out.push(serde_json::json!({"role": "user", "content": text}));
        } else {
            out.push(serde_json::json!({"role": "user", "content": user_content}));
        }
    }
    Ok(())
}

/// Anthropic `tool_result.content` may be a string or an array of blocks;
/// flatten to the plain string OpenAI's `tool` message expects.
fn tool_result_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => {
            let mut buf = String::new();
            for b in blocks {
                if let Some(t) = b.get("text").and_then(Value::as_str) {
                    buf.push_str(t);
                }
            }
            buf
        }
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

/// Build a `data:` URL from an Anthropic base64 image block.
fn image_data_url(block: &Value) -> Option<String> {
    let source = block.get("source")?;
    let media_type = source.get("media_type").and_then(Value::as_str)?;
    let data = source.get("data").and_then(Value::as_str)?;
    Some(format!("data:{media_type};base64,{data}"))
}

/// Anthropic tool → OpenAI `{type:"function", function:{...}}`, mapping
/// `input_schema` → `parameters`.
fn translate_tool(tool: &Value) -> Option<Value> {
    let name = tool.get("name").and_then(Value::as_str)?;
    let mut func = Map::new();
    func.insert("name".into(), Value::String(name.to_owned()));
    if let Some(desc) = tool.get("description") {
        func.insert("description".into(), desc.clone());
    }
    if let Some(schema) = tool.get("input_schema") {
        func.insert("parameters".into(), schema.clone());
    }
    Some(serde_json::json!({"type": "function", "function": func}))
}

/// Anthropic `tool_choice` → OpenAI `tool_choice`.
fn translate_tool_choice(choice: &Value) -> Value {
    match choice.get("type").and_then(Value::as_str) {
        Some("any") => Value::String("required".into()),
        Some("none") => Value::String("none".into()),
        Some("tool") => {
            let name = choice.get("name").and_then(Value::as_str).unwrap_or("");
            serde_json::json!({"type": "function", "function": {"name": name}})
        }
        // `auto` and unknown/missing values both use OpenAI's default mode.
        _ => Value::String("auto".into()),
    }
}

// ---------------------------------------------------------------------------
// Response: non-streaming
// ---------------------------------------------------------------------------

/// Build an Anthropic Messages response object from a decoded OpenAI chat
/// completion.
pub(super) fn completion_to_json(c: &ChatCompletion) -> Value {
    let mut content: Vec<Value> = Vec::new();
    if !c.text.is_empty() {
        content.push(serde_json::json!({"type": "text", "text": c.text}));
    }
    for tc in &c.tool_calls {
        content.push(serde_json::json!({
            "type": "tool_use",
            "id": tc.id,
            "name": tc.name,
            "input": tc.input,
        }));
    }
    serde_json::json!({
        "id": message_id(&c.id),
        "type": "message",
        "role": "assistant",
        "model": c.model,
        "content": content,
        "stop_reason": c.finish_reason.anthropic(),
        "stop_sequence": Value::Null,
        "usage": {
            "input_tokens": c.usage.input.unwrap_or(0),
            "output_tokens": c.usage.output.unwrap_or(0),
        }
    })
}

fn message_id(openai_id: &str) -> String {
    if openai_id.is_empty() {
        "msg_0".to_owned()
    } else {
        format!("msg_{openai_id}")
    }
}

// ---------------------------------------------------------------------------
// Response: streaming
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenBlock {
    Text,
    Tool,
}

/// State machine that re-frames OpenAI streaming events into an Anthropic SSE
/// event sequence. Drives `message_start` once, opens/closes text and
/// `tool_use` content blocks as events arrive, and emits `message_delta` +
/// `message_stop` at the end.
pub(super) struct StreamState {
    model: String,
    started: bool,
    /// Anthropic content-block index counter.
    next_index: i64,
    open: Option<OpenBlock>,
    /// Anthropic index of the currently-open block.
    open_index: i64,
    /// OpenAI tool index of the currently-open tool block, if any.
    open_tool_index: Option<i64>,
    finish: FinishReason,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
}

impl StreamState {
    pub(super) fn new(model: String) -> Self {
        Self {
            model,
            started: false,
            next_index: 0,
            open: None,
            open_index: 0,
            open_tool_index: None,
            finish: FinishReason::Stop,
            input_tokens: None,
            output_tokens: None,
        }
    }

    pub(super) fn handle(&mut self, event: ChatEvent, out: &mut String) {
        match event {
            ChatEvent::TextDelta(text) => {
                self.ensure_started(out);
                if self.open == Some(OpenBlock::Tool) {
                    self.close_block(out);
                }
                if self.open.is_none() {
                    self.open_text(out);
                }
                push_event(
                    out,
                    "content_block_delta",
                    &serde_json::json!({
                        "type": "content_block_delta",
                        "index": self.open_index,
                        "delta": {"type": "text_delta", "text": text},
                    }),
                );
            }
            ChatEvent::ToolCallStart { index, id, name } => {
                self.ensure_started(out);
                // A new tool index always opens a fresh block.
                if self.open.is_some() {
                    self.close_block(out);
                }
                self.open_tool(out, index, &id, &name);
            }
            ChatEvent::ToolCallArgs { index, fragment } => {
                self.ensure_started(out);
                if self.open == Some(OpenBlock::Tool) && self.open_tool_index == Some(index) {
                    push_event(
                        out,
                        "content_block_delta",
                        &serde_json::json!({
                            "type": "content_block_delta",
                            "index": self.open_index,
                            "delta": {"type": "input_json_delta", "partial_json": fragment},
                        }),
                    );
                }
            }
            ChatEvent::Finish(reason) => {
                self.finish = reason;
            }
            ChatEvent::Usage(tokens) => {
                if tokens.input.is_some() {
                    self.input_tokens = tokens.input;
                }
                if tokens.output.is_some() {
                    self.output_tokens = tokens.output;
                }
            }
        }
    }

    pub(super) fn finish_stream(&mut self, out: &mut String) {
        self.ensure_started(out);
        if self.open.is_some() {
            self.close_block(out);
        }
        // Anthropic reports output_tokens in message_delta; include
        // input_tokens too when the upstream surfaced it so clients see real
        // totals (input is unknown at message_start time for OpenAI streams).
        let mut usage = Map::new();
        if let Some(input) = self.input_tokens {
            usage.insert("input_tokens".into(), Value::from(input));
        }
        usage.insert(
            "output_tokens".into(),
            Value::from(self.output_tokens.unwrap_or(0)),
        );
        push_event(
            out,
            "message_delta",
            &serde_json::json!({
                "type": "message_delta",
                "delta": {"stop_reason": self.finish.anthropic(), "stop_sequence": Value::Null},
                "usage": Value::Object(usage),
            }),
        );
        push_event(
            out,
            "message_stop",
            &serde_json::json!({"type": "message_stop"}),
        );
    }

    fn ensure_started(&mut self, out: &mut String) {
        if self.started {
            return;
        }
        self.started = true;
        push_event(
            out,
            "message_start",
            &serde_json::json!({
                "type": "message_start",
                "message": {
                    "id": "msg_stream",
                    "type": "message",
                    "role": "assistant",
                    "model": self.model,
                    "content": [],
                    "stop_reason": Value::Null,
                    "stop_sequence": Value::Null,
                    // input_tokens is unknown until the trailing usage chunk;
                    // real totals land in the closing message_delta.
                    "usage": {"input_tokens": 0, "output_tokens": 0},
                }
            }),
        );
    }

    fn open_text(&mut self, out: &mut String) {
        self.open = Some(OpenBlock::Text);
        self.open_index = self.next_index;
        self.next_index += 1;
        self.open_tool_index = None;
        push_event(
            out,
            "content_block_start",
            &serde_json::json!({
                "type": "content_block_start",
                "index": self.open_index,
                "content_block": {"type": "text", "text": ""},
            }),
        );
    }

    fn open_tool(&mut self, out: &mut String, tool_index: i64, id: &str, name: &str) {
        self.open = Some(OpenBlock::Tool);
        self.open_index = self.next_index;
        self.next_index += 1;
        self.open_tool_index = Some(tool_index);
        push_event(
            out,
            "content_block_start",
            &serde_json::json!({
                "type": "content_block_start",
                "index": self.open_index,
                "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}},
            }),
        );
    }

    fn close_block(&mut self, out: &mut String) {
        push_event(
            out,
            "content_block_stop",
            &serde_json::json!({"type": "content_block_stop", "index": self.open_index}),
        );
        self.open = None;
        self.open_tool_index = None;
    }
}

fn push_event(out: &mut String, event: &str, data: &Value) {
    out.push_str("event: ");
    out.push_str(event);
    out.push_str("\ndata: ");
    out.push_str(&data.to_string());
    out.push_str("\n\n");
}

#[cfg(test)]
mod tests {
    use super::super::openai::{OpenAiSseParser, parse_chat_completion};
    use super::*;

    #[test]
    fn request_maps_system_messages_tools_and_max_tokens() {
        let body = serde_json::json!({
            "model": "anthropic/claude-3.5",
            "max_tokens": 256,
            "system": "be terse",
            "temperature": 0.5,
            "messages": [{"role": "user", "content": "hello"}],
            "tools": [{
                "name": "lookup",
                "description": "find",
                "input_schema": {"type": "object", "properties": {"q": {"type": "string"}}}
            }],
            "tool_choice": {"type": "any"}
        });
        let out = translate_request(&body).unwrap();
        assert_eq!(out["model"], "anthropic/claude-3.5");
        assert_eq!(out["max_tokens"], 256);
        assert_eq!(out["temperature"], 0.5);
        assert_eq!(out["messages"][0]["role"], "system");
        assert_eq!(out["messages"][0]["content"], "be terse");
        assert_eq!(out["messages"][1]["role"], "user");
        assert_eq!(out["messages"][1]["content"], "hello");
        assert_eq!(out["tools"][0]["type"], "function");
        assert_eq!(out["tools"][0]["function"]["name"], "lookup");
        assert_eq!(
            out["tools"][0]["function"]["parameters"]["properties"]["q"]["type"],
            "string"
        );
        assert_eq!(out["tool_choice"], "required");
    }

    #[test]
    fn request_expands_tool_use_and_tool_result_turns() {
        let body = serde_json::json!({
            "model": "m",
            "max_tokens": 16,
            "messages": [
                {"role": "user", "content": "run it"},
                {"role": "assistant", "content": [
                    {"type": "text", "text": "sure"},
                    {"type": "tool_use", "id": "call_1", "name": "bash", "input": {"cmd": "ls"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "call_1", "content": "file.txt"}
                ]}
            ]
        });
        let out = translate_request(&body).unwrap();
        let msgs = out["messages"].as_array().unwrap();
        // user, assistant(+tool_calls), tool
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[1]["content"], "sure");
        assert_eq!(msgs[1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(msgs[1]["tool_calls"][0]["function"]["name"], "bash");
        // arguments is a JSON string
        assert_eq!(
            msgs[1]["tool_calls"][0]["function"]["arguments"],
            "{\"cmd\":\"ls\"}"
        );
        assert_eq!(msgs[2]["role"], "tool");
        assert_eq!(msgs[2]["tool_call_id"], "call_1");
        assert_eq!(msgs[2]["content"], "file.txt");
    }

    #[test]
    fn non_streaming_response_maps_text_and_stop_reason() {
        let v = serde_json::json!({
            "id": "chatcmpl-x",
            "model": "m",
            "choices": [{"message": {"content": "hi"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 7, "completion_tokens": 3}
        });
        let c = parse_chat_completion(&v);
        let a = completion_to_json(&c);
        assert_eq!(a["type"], "message");
        assert_eq!(a["role"], "assistant");
        assert_eq!(a["content"][0]["type"], "text");
        assert_eq!(a["content"][0]["text"], "hi");
        assert_eq!(a["stop_reason"], "end_turn");
        assert_eq!(a["usage"]["input_tokens"], 7);
        assert_eq!(a["usage"]["output_tokens"], 3);
        assert_eq!(a["id"], "msg_chatcmpl-x");
    }

    #[test]
    fn non_streaming_response_maps_tool_use() {
        let v = serde_json::json!({
            "id": "c1",
            "model": "m",
            "choices": [{"message": {"content": null, "tool_calls": [{
                "id": "call_1", "type": "function",
                "function": {"name": "bash", "arguments": "{\"cmd\":\"ls\"}"}
            }]}, "finish_reason": "tool_calls"}]
        });
        let c = parse_chat_completion(&v);
        let a = completion_to_json(&c);
        assert_eq!(a["stop_reason"], "tool_use");
        assert_eq!(a["content"][0]["type"], "tool_use");
        assert_eq!(a["content"][0]["id"], "call_1");
        assert_eq!(a["content"][0]["name"], "bash");
        assert_eq!(a["content"][0]["input"]["cmd"], "ls");
    }

    /// Feed an OpenAI SSE text stream through the parser + state machine and
    /// assert the Anthropic event lifecycle comes out in order.
    #[test]
    fn streaming_response_emits_anthropic_lifecycle() {
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"content\":\"lo\"},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
                   data: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2}}\n\n\
                   data: [DONE]\n\n";
        let mut parser = OpenAiSseParser::default();
        let mut state = StreamState::new("m".into());
        let mut out = String::new();
        for e in parser.feed(sse.as_bytes()) {
            state.handle(e, &mut out);
        }
        for e in parser.finish() {
            state.handle(e, &mut out);
        }
        state.finish_stream(&mut out);

        // Ordered event types.
        let types: Vec<&str> = out
            .lines()
            .filter_map(|l| l.strip_prefix("event: "))
            .collect();
        assert_eq!(
            types,
            vec![
                "message_start",
                "content_block_start",
                "content_block_delta",
                "content_block_delta",
                "content_block_stop",
                "message_delta",
                "message_stop",
            ]
        );
        assert!(out.contains("\"text_delta\""));
        assert!(out.contains("\"stop_reason\":\"end_turn\""));
        assert!(out.contains("\"output_tokens\":2"));
        assert!(out.contains("\"input_tokens\":5"));
    }

    #[test]
    fn streaming_tool_call_becomes_tool_use_block() {
        let sse = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}}]},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n\
                   data: [DONE]\n\n";
        let mut parser = OpenAiSseParser::default();
        let mut state = StreamState::new("m".into());
        let mut out = String::new();
        for e in parser.feed(sse.as_bytes()) {
            state.handle(e, &mut out);
        }
        for e in parser.finish() {
            state.handle(e, &mut out);
        }
        state.finish_stream(&mut out);

        assert!(out.contains("\"type\":\"tool_use\""));
        assert!(out.contains("\"name\":\"bash\""));
        assert!(out.contains("\"input_json_delta\""));
        assert!(out.contains("{\\\"cmd\\\":\\\"ls\\\"}"));
        assert!(out.contains("\"stop_reason\":\"tool_use\""));
    }
}
