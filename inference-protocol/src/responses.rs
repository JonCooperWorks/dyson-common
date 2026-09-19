//! OpenAI Responses ⇄ OpenAI Chat Completions translation.
//!
//! Inbound: a `/v1/responses` request (`input` string or item array +
//! `instructions`) → a Chat Completions body ([`translate_request`]).
//!
//! Outbound: the Chat Completions response → the Responses shape, both the
//! single-object form ([`completion_to_json`]) and a pragmatic streamed event
//! subset ([`StreamState`]), including text, JSON function tools, and Codex
//! custom-tool calls.
//!
//! Launch scope is STATELESS. `previous_response_id` and `store:true` are not
//! supported — the caller rejects requests that set them (see
//! [`crate::proxy::dialect::Dialect::translate_request`]).

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::openai::{ChatCompletion, ChatEvent, FinishReason};

/// Chat Completions only has JSON function tools. Prefix Responses custom-tool
/// names while they cross that dialect boundary so the response translator can
/// restore a `custom_tool_call` without retaining per-request state.
const CUSTOM_TOOL_PREFIX: &str = "swarm_custom__";

/// Some Chat providers emit underscores in place of MCP server hyphens.
/// Resolve only unique aliases of tools actually advertised in this request.
/// Exact names win; collisions and unknown names are never guessed.
#[derive(Clone, Default)]
pub(super) struct ToolNames {
    aliases: BTreeMap<String, Option<String>>,
    declarations: BTreeMap<String, (Option<String>, String)>,
}

impl ToolNames {
    pub(super) fn from_request(body: &Value) -> Self {
        let translated: Vec<Value> = body
            .get("tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .flat_map(translate_tool)
            .collect();
        let names: std::collections::BTreeSet<&str> = translated
            .iter()
            .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
            .collect();
        let mut aliases = BTreeMap::new();
        for name in &names {
            if !name.starts_with("mcp__") || !name.contains('-') {
                continue;
            }
            let alias = name.replace('-', "_");
            if names.contains(alias.as_str()) {
                continue;
            }
            aliases
                .entry(alias)
                .and_modify(|entry| *entry = None)
                .or_insert_with(|| Some((*name).to_owned()));
        }
        let mut declarations = BTreeMap::new();
        for tool in body
            .get("tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if tool.get("type").and_then(Value::as_str) == Some("namespace") {
                if let Some(namespace) = tool.get("name").and_then(Value::as_str) {
                    for child in tool
                        .get("tools")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(name) = child.get("name").and_then(Value::as_str) {
                            declarations.insert(
                                flatten_tool_name(Some(namespace), name),
                                (Some(namespace.to_owned()), name.to_owned()),
                            );
                        }
                    }
                }
            } else if let Some(name) = tool.get("name").and_then(Value::as_str) {
                declarations.insert(name.to_owned(), (None, name.to_owned()));
            }
        }
        Self {
            aliases,
            declarations,
        }
    }

    fn split(&self, name: &str) -> (Option<String>, String) {
        self.declarations
            .get(name)
            .cloned()
            .unwrap_or_else(|| split_tool_name(name))
    }

    pub(super) fn restore(&self, name: &mut String) {
        if let Some(Some(original)) = self.aliases.get(name) {
            tracing::debug!(emitted_tool = %name, declared_tool = %original, "restoring MCP tool name spelling");
            name.clone_from(original);
        }
    }
}

/// Translate an OpenAI Responses request body into a Chat Completions body.
pub(crate) fn translate_request(body: &Value) -> Result<Value, String> {
    let obj = body
        .as_object()
        .ok_or("request body must be a JSON object")?;

    // Stateless launch: refuse the stateful knobs rather than silently drop
    // them (which would surprise a caller relying on server-side threading).
    if obj
        .get("previous_response_id")
        .is_some_and(|v| !v.is_null())
    {
        return Err("`previous_response_id` is not supported (stateless API)".into());
    }
    if obj.get("store").and_then(Value::as_bool) == Some(true) {
        return Err("`store: true` is not supported (stateless API)".into());
    }

    let model = obj
        .get("model")
        .and_then(Value::as_str)
        .ok_or("`model` is required")?;

    let mut messages: Vec<Value> = Vec::new();
    let mut translated_tools: Vec<Value> = obj
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(translate_tool)
        .collect();
    if let Some(instructions) = obj.get("instructions").and_then(Value::as_str)
        && !instructions.is_empty()
    {
        messages.push(serde_json::json!({"role": "system", "content": instructions}));
    }
    match obj.get("input") {
        Some(Value::String(s)) => {
            messages.push(serde_json::json!({"role": "user", "content": s}));
        }
        Some(Value::Array(items)) => {
            translate_input_items(items, &mut messages, &mut translated_tools)?
        }
        Some(Value::Null) | None => {}
        Some(_) => return Err("`input` must be a string or array".into()),
    }

    let mut out = Map::new();
    out.insert("model".into(), Value::String(model.to_owned()));
    // Responses uses `max_output_tokens`; Chat uses `max_tokens`.
    if let Some(mt) = obj.get("max_output_tokens") {
        out.insert("max_tokens".into(), mt.clone());
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
    if !translated_tools.is_empty() {
        out.insert("tools".into(), Value::Array(translated_tools));
    }
    if let Some(tool_choice) = obj.get("tool_choice") {
        out.insert("tool_choice".into(), translate_tool_choice(tool_choice));
    }
    if let Some(parallel) = obj.get("parallel_tool_calls") {
        out.insert("parallel_tool_calls".into(), parallel.clone());
    }
    // OpenRouter accepts its reasoning controls on the Chat Completions
    // surface. Preserve them so a manifest-selected reasoning model retains
    // the Codex client's requested effort instead of silently dropping it.
    if let Some(reasoning) = obj.get("reasoning") {
        out.insert("reasoning".into(), reasoning.clone());
    }

    Ok(Value::Object(out))
}

/// Translate the complete stateless Responses transcript. Codex sends prior
/// function calls and their outputs back as input items on the next turn; both
/// halves must survive or the model loses the tool round trip.
fn translate_input_items(
    items: &[Value],
    out: &mut Vec<Value>,
    tools: &mut Vec<Value>,
) -> Result<(), String> {
    for item in items {
        match item.get("type").and_then(Value::as_str) {
            Some("additional_tools") => {
                let additional = item
                    .get("tools")
                    .and_then(Value::as_array)
                    .ok_or("additional tools item must contain a `tools` array")?;
                tools.extend(additional.iter().flat_map(translate_tool));
            }
            Some("function_call") => {
                let call_id = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                    .ok_or("function call `call_id` is required")?;
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("function call `name` is required")?;
                let name = flatten_tool_name(item.get("namespace").and_then(Value::as_str), name);
                let arguments = item
                    .get("arguments")
                    .map(json_string)
                    .unwrap_or_else(|| "{}".to_owned());
                out.push(serde_json::json!({
                    "role": "assistant",
                    "content": Value::Null,
                    "tool_calls": [{
                        "id": call_id,
                        "type": "function",
                        "function": {"name": name, "arguments": arguments},
                    }],
                }));
            }
            Some("function_call_output") => {
                let call_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .ok_or("function call output `call_id` is required")?;
                let content = item.get("output").map(json_string).unwrap_or_default();
                out.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": content,
                }));
            }
            Some("custom_tool_call") => {
                let call_id = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                    .ok_or("custom tool call `call_id` is required")?;
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("custom tool call `name` is required")?;
                let name = flatten_tool_name(item.get("namespace").and_then(Value::as_str), name);
                let input = item.get("input").map(json_string).unwrap_or_default();
                out.push(serde_json::json!({
                    "role": "assistant",
                    "content": Value::Null,
                    "tool_calls": [{
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": format!("{CUSTOM_TOOL_PREFIX}{name}"),
                            "arguments": serde_json::json!({"input": input}).to_string(),
                        },
                    }],
                }));
            }
            Some("custom_tool_call_output") => {
                let call_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .ok_or("custom tool output `call_id` is required")?;
                let content = item.get("output").map(json_string).unwrap_or_default();
                out.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": content,
                }));
            }
            // Reasoning items are provider-owned opaque state. OpenRouter's
            // Chat surface cannot replay them, but dropping them is safe: the
            // visible assistant/tool transcript remains complete.
            Some("reasoning") => {}
            Some("message") | None => {
                let message = translate_input_item(item)
                    .ok_or("message input item must contain supported `content`")?;
                out.push(message);
            }
            Some(other) => return Err(format!("unsupported Responses input item `{other}`")),
        }
    }
    Ok(())
}

fn json_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

/// Translate one Responses `input` item to a Chat message. Handles the
/// `{role, content}` message item, where `content` is a string or an array of
/// `input_text`/`output_text` parts.
fn translate_input_item(item: &Value) -> Option<Value> {
    // A bare `{type:"message", role, content}` or `{role, content}`.
    let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
    let content = item.get("content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => {
            let mut buf = String::new();
            for p in parts {
                match p.get("type").and_then(Value::as_str) {
                    Some("input_text" | "output_text" | "text") | None => {
                        if let Some(t) = p.get("text").and_then(Value::as_str) {
                            buf.push_str(t);
                        }
                    }
                    _ => {}
                }
            }
            buf
        }
        _ => return None,
    };
    Some(serde_json::json!({"role": role, "content": text}))
}

/// Responses tools → Chat function tools. Codex 0.147 groups every MCP
/// server under a Responses-only `type:"namespace"` container. OpenRouter's
/// Chat Completions surface has no namespace equivalent, so expand the child
/// tools and carry their namespace in a deterministic flat name. The response
/// path reverses this encoding before Codex sees the function call.
fn translate_tool(tool: &Value) -> Vec<Value> {
    // Already Chat-shaped? Pass through.
    if tool.get("function").is_some() {
        return vec![tool.clone()];
    }
    if tool.get("type").and_then(Value::as_str) == Some("namespace") {
        let Some(namespace) = tool.get("name").and_then(Value::as_str) else {
            return Vec::new();
        };
        return tool
            .get("tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|child| {
                let child_name = child.get("name").and_then(Value::as_str)?;
                let name = flatten_tool_name(Some(namespace), child_name);
                Some(translate_named_tool(child, &name))
            })
            .collect();
    }
    let Some(name) = tool.get("name").and_then(Value::as_str) else {
        return Vec::new();
    };
    vec![translate_named_tool(tool, name)]
}

fn translate_named_tool(tool: &Value, name: &str) -> Value {
    if tool.get("type").and_then(Value::as_str) == Some("custom") {
        let mut description = tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("Custom tool input")
            .to_owned();
        description.push_str("\n\nPass the complete raw custom-tool input in the `input` string.");
        if let Some(format) = tool.get("format") {
            description.push_str("\nCustom input format: ");
            description.push_str(&format.to_string());
        }
        return serde_json::json!({
            "type": "function",
            "function": {
                "name": format!("{CUSTOM_TOOL_PREFIX}{name}"),
                "description": description,
                "parameters": {
                    "type": "object",
                    "properties": {
                        "input": {
                            "type": "string",
                            "description": "Complete raw input for the custom tool"
                        }
                    },
                    "required": ["input"],
                    "additionalProperties": false
                }
            }
        });
    }
    let mut func = Map::new();
    func.insert("name".into(), Value::String(name.to_owned()));
    if let Some(desc) = tool.get("description") {
        func.insert("description".into(), desc.clone());
    }
    if let Some(params) = tool.get("parameters") {
        func.insert("parameters".into(), params.clone());
    }
    serde_json::json!({"type": "function", "function": func})
}

fn flatten_tool_name(namespace: Option<&str>, name: &str) -> String {
    match namespace.filter(|namespace| !namespace.is_empty() && *namespace != "functions") {
        None => name.to_owned(),
        Some(namespace) if namespace.ends_with("__") => format!("{namespace}{name}"),
        Some(namespace) => format!("{namespace}__{name}"),
    }
}

/// Reverse [`flatten_tool_name`] for a model-emitted Chat function call.
/// MCP namespaces are canonical `mcp__<server>__`; other non-default
/// namespaces use one `__` separator. Plain names remain top-level functions.
fn split_tool_name(flattened: &str) -> (Option<String>, String) {
    let Some(split) = flattened.rfind("__") else {
        return (None, flattened.to_owned());
    };
    let name = &flattened[split + 2..];
    if name.is_empty() {
        return (None, flattened.to_owned());
    }
    if flattened.starts_with("mcp__") {
        (Some(flattened[..split + 2].to_owned()), name.to_owned())
    } else {
        (Some(flattened[..split].to_owned()), name.to_owned())
    }
}

fn translate_tool_choice(choice: &Value) -> Value {
    let Some(obj) = choice.as_object() else {
        return choice.clone();
    };
    if obj.get("type").and_then(Value::as_str) != Some("function") {
        return choice.clone();
    }
    let Some(name) = obj.get("name").and_then(Value::as_str) else {
        return choice.clone();
    };
    let name = flatten_tool_name(obj.get("namespace").and_then(Value::as_str), name);
    serde_json::json!({"type": "function", "function": {"name": name}})
}

// ---------------------------------------------------------------------------
// Response: non-streaming
// ---------------------------------------------------------------------------

/// Build a Responses object from a decoded chat completion.
#[cfg(test)]
pub(super) fn completion_to_json(c: &ChatCompletion) -> Value {
    completion_to_json_with_tools(c, &ToolNames::default())
}

pub(super) fn completion_to_json_with_tools(c: &ChatCompletion, tool_names: &ToolNames) -> Value {
    let response_id = response_id(&c.id);
    let item_id = format!("msg_{}", trimmed(&c.id));
    let mut output: Vec<Value> = Vec::new();
    // Text message item.
    output.push(serde_json::json!({
        "type": "message",
        "id": item_id,
        "status": "completed",
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": c.text,
            "annotations": [],
        }],
    }));
    // Function-call items (best-effort; Responses represents each tool call as
    // its own output item).
    for tc in &c.tool_calls {
        if let Some(flattened_name) = tc.name.strip_prefix(CUSTOM_TOOL_PREFIX) {
            let (namespace, name) = tool_names.split(flattened_name);
            let mut item = serde_json::json!({
                "type": "custom_tool_call",
                "id": format!("ctc_{}", tc.id),
                "call_id": tc.id,
                "name": name,
                "input": custom_tool_input(&tc.input),
                "status": "completed",
            });
            if let Some(namespace) = namespace {
                item["namespace"] = Value::String(namespace);
            }
            output.push(item);
        } else {
            let (namespace, name) = tool_names.split(&tc.name);
            let mut item = serde_json::json!({
                "type": "function_call",
                "id": format!("fc_{}", tc.id),
                "call_id": tc.id,
                "name": name,
                "arguments": tc.input.to_string(),
                "status": "completed",
            });
            if let Some(namespace) = namespace {
                item["namespace"] = Value::String(namespace);
            }
            output.push(item);
        }
    }
    serde_json::json!({
        "id": response_id,
        "object": "response",
        "model": c.model,
        "status": status_for(c.finish_reason),
        "output": output,
        "output_text": c.text,
        "usage": usage_json(c),
    })
}

fn usage_json(c: &ChatCompletion) -> Value {
    serde_json::json!({
        "input_tokens": c.usage.input.unwrap_or(0),
        "output_tokens": c.usage.output.unwrap_or(0),
        "total_tokens": c.usage.total.unwrap_or_else(|| {
            c.usage.input.unwrap_or(0) + c.usage.output.unwrap_or(0)
        }),
    })
}

fn status_for(reason: FinishReason) -> &'static str {
    match reason {
        FinishReason::Length => "incomplete",
        _ => "completed",
    }
}

fn response_id(openai_id: &str) -> String {
    format!("resp_{}", trimmed(openai_id))
}

fn trimmed(openai_id: &str) -> String {
    if openai_id.is_empty() {
        "0".to_owned()
    } else {
        openai_id.trim_start_matches("chatcmpl-").to_owned()
    }
}

fn custom_tool_input(arguments: &Value) -> String {
    arguments
        .get("input")
        .map(json_string)
        .unwrap_or_else(|| json_string(arguments))
}

// ---------------------------------------------------------------------------
// Response: streaming
// ---------------------------------------------------------------------------

/// State machine emitting Responses text and function-call events. Codex uses
/// the streamed function-call argument events to invoke its local tools, so a
/// text-only shim is not sufficient for managed agents.
pub(super) struct StreamState {
    pub(super) tool_names: ToolNames,
    model: String,
    started: bool,
    response_id: String,
    text_item_id: String,
    text_output_index: Option<i64>,
    next_output_index: i64,
    text: String,
    tools: BTreeMap<i64, ToolStream>,
    finish: FinishReason,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    total_tokens: Option<i64>,
}

struct ToolStream {
    output_index: i64,
    item_id: String,
    call_id: String,
    name: String,
    namespace: Option<String>,
    arguments: String,
    custom: bool,
}

impl StreamState {
    pub(super) fn new(model: String) -> Self {
        Self {
            model,
            tool_names: ToolNames::default(),
            started: false,
            response_id: response_id(""),
            text_item_id: "msg_stream".to_owned(),
            text_output_index: None,
            next_output_index: 0,
            text: String::new(),
            tools: BTreeMap::new(),
            finish: FinishReason::Stop,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
        }
    }

    pub(super) fn handle(&mut self, event: ChatEvent, out: &mut String) {
        match event {
            ChatEvent::TextDelta(text) => {
                self.ensure_started(out);
                self.ensure_text_item(out);
                self.text.push_str(&text);
                push_event(
                    out,
                    "response.output_text.delta",
                    &serde_json::json!({
                        "type": "response.output_text.delta",
                        "item_id": self.text_item_id,
                        "output_index": self.text_output_index.unwrap_or(0),
                        "content_index": 0,
                        "delta": text,
                    }),
                );
            }
            ChatEvent::ToolCallStart { index, id, name } => {
                self.ensure_started(out);
                let output_index = self.next_output_index;
                self.next_output_index += 1;
                let custom_name = name.strip_prefix(CUSTOM_TOOL_PREFIX);
                let custom = custom_name.is_some();
                let flattened_name = custom_name.unwrap_or(&name);
                let (namespace, visible_name) = self.tool_names.split(flattened_name);
                let item_id = format!("{}_{}", if custom { "ctc" } else { "fc" }, trimmed(&id));
                let mut item = if custom {
                    serde_json::json!({
                        "type": "custom_tool_call",
                        "id": item_id,
                        "call_id": id,
                        "name": visible_name,
                        "input": "",
                        "status": "in_progress",
                    })
                } else {
                    serde_json::json!({
                        "type": "function_call",
                        "id": item_id,
                        "call_id": id,
                        "name": visible_name,
                        "arguments": "",
                        "status": "in_progress",
                    })
                };
                if let Some(namespace) = namespace.as_ref() {
                    item["namespace"] = Value::String(namespace.clone());
                }
                push_event(
                    out,
                    "response.output_item.added",
                    &serde_json::json!({
                        "type": "response.output_item.added",
                        "output_index": output_index,
                        "item": item,
                    }),
                );
                self.tools.insert(
                    index,
                    ToolStream {
                        output_index,
                        item_id,
                        call_id: id,
                        name: visible_name,
                        namespace,
                        arguments: String::new(),
                        custom,
                    },
                );
            }
            ChatEvent::ToolCallArgs { index, fragment } => {
                let Some(tool) = self.tools.get_mut(&index) else {
                    return;
                };
                tool.arguments.push_str(&fragment);
                push_event(
                    out,
                    "response.function_call_arguments.delta",
                    &serde_json::json!({
                        "type": "response.function_call_arguments.delta",
                        "item_id": tool.item_id,
                        "output_index": tool.output_index,
                        "delta": fragment,
                    }),
                );
            }
            ChatEvent::Finish(reason) => self.finish = reason,
            ChatEvent::Usage(tokens) => {
                if tokens.input.is_some() {
                    self.input_tokens = tokens.input;
                }
                if tokens.output.is_some() {
                    self.output_tokens = tokens.output;
                }
                if tokens.total.is_some() {
                    self.total_tokens = tokens.total;
                }
            }
        }
    }

    pub(super) fn finish_stream(&mut self, out: &mut String) {
        self.ensure_started(out);
        let mut output = Vec::new();
        if let Some(output_index) = self.text_output_index {
            push_event(
                out,
                "response.output_text.done",
                &serde_json::json!({
                    "type": "response.output_text.done",
                    "item_id": self.text_item_id,
                    "output_index": output_index,
                    "content_index": 0,
                    "text": self.text,
                }),
            );
            let content = serde_json::json!({
                "type": "output_text", "text": self.text, "annotations": [],
            });
            push_event(
                out,
                "response.content_part.done",
                &serde_json::json!({
                    "type": "response.content_part.done",
                    "item_id": self.text_item_id,
                    "output_index": output_index,
                    "content_index": 0,
                    "part": content,
                }),
            );
            let item = serde_json::json!({
                "type": "message",
                "id": self.text_item_id,
                "status": "completed",
                "role": "assistant",
                "content": [content],
            });
            push_event(
                out,
                "response.output_item.done",
                &serde_json::json!({
                    "type": "response.output_item.done",
                    "output_index": output_index,
                    "item": item,
                }),
            );
            output.push((output_index, item));
        }
        for tool in self.tools.values() {
            if tool.custom {
                let input = serde_json::from_str::<Value>(&tool.arguments)
                    .ok()
                    .map(|arguments| custom_tool_input(&arguments))
                    .unwrap_or_else(|| tool.arguments.clone());
                push_event(
                    out,
                    "response.custom_tool_call_input.delta",
                    &serde_json::json!({
                        "type": "response.custom_tool_call_input.delta",
                        "item_id": tool.item_id,
                        "output_index": tool.output_index,
                        "delta": input,
                    }),
                );
                push_event(
                    out,
                    "response.custom_tool_call_input.done",
                    &serde_json::json!({
                        "type": "response.custom_tool_call_input.done",
                        "item_id": tool.item_id,
                        "output_index": tool.output_index,
                        "input": input,
                    }),
                );
                let mut item = serde_json::json!({
                    "type": "custom_tool_call",
                    "id": tool.item_id,
                    "call_id": tool.call_id,
                    "name": tool.name,
                    "input": input,
                    "status": "completed",
                });
                if let Some(namespace) = tool.namespace.as_ref() {
                    item["namespace"] = Value::String(namespace.clone());
                }
                push_event(
                    out,
                    "response.output_item.done",
                    &serde_json::json!({
                        "type": "response.output_item.done",
                        "output_index": tool.output_index,
                        "item": item,
                    }),
                );
                output.push((tool.output_index, item));
                continue;
            }
            push_event(
                out,
                "response.function_call_arguments.done",
                &serde_json::json!({
                    "type": "response.function_call_arguments.done",
                    "item_id": tool.item_id,
                    "output_index": tool.output_index,
                    "arguments": tool.arguments,
                }),
            );
            let mut item = serde_json::json!({
                "type": "function_call",
                "id": tool.item_id,
                "call_id": tool.call_id,
                "name": tool.name,
                "arguments": tool.arguments,
                "status": "completed",
            });
            if let Some(namespace) = tool.namespace.as_ref() {
                item["namespace"] = Value::String(namespace.clone());
            }
            push_event(
                out,
                "response.output_item.done",
                &serde_json::json!({
                    "type": "response.output_item.done",
                    "output_index": tool.output_index,
                    "item": item,
                }),
            );
            output.push((tool.output_index, item));
        }
        output.sort_by_key(|(index, _)| *index);
        let output: Vec<Value> = output.into_iter().map(|(_, item)| item).collect();
        push_event(
            out,
            "response.completed",
            &serde_json::json!({
                "type": "response.completed",
                "response": {
                    "id": self.response_id,
                    "object": "response",
                    "model": self.model,
                    "status": status_for(self.finish),
                    "output": output,
                    "output_text": self.text,
                    "usage": {
                        "input_tokens": self.input_tokens.unwrap_or(0),
                        "output_tokens": self.output_tokens.unwrap_or(0),
                        "total_tokens": self.total_tokens.unwrap_or_else(|| {
                            self.input_tokens.unwrap_or(0) + self.output_tokens.unwrap_or(0)
                        }),
                    },
                }
            }),
        );
    }

    fn ensure_started(&mut self, out: &mut String) {
        if self.started {
            return;
        }
        self.started = true;
        push_event(
            out,
            "response.created",
            &serde_json::json!({
                "type": "response.created",
                "response": {
                    "id": self.response_id,
                    "object": "response",
                    "model": self.model,
                    "status": "in_progress",
                    "output": [],
                }
            }),
        );
    }

    fn ensure_text_item(&mut self, out: &mut String) {
        if self.text_output_index.is_some() {
            return;
        }
        let output_index = self.next_output_index;
        self.next_output_index += 1;
        self.text_output_index = Some(output_index);
        push_event(
            out,
            "response.output_item.added",
            &serde_json::json!({
                "type": "response.output_item.added",
                "output_index": output_index,
                "item": {
                    "type": "message",
                    "id": self.text_item_id,
                    "status": "in_progress",
                    "role": "assistant",
                    "content": [],
                },
            }),
        );
        push_event(
            out,
            "response.content_part.added",
            &serde_json::json!({
                "type": "response.content_part.added",
                "item_id": self.text_item_id,
                "output_index": output_index,
                "content_index": 0,
                "part": {"type": "output_text", "text": "", "annotations": []},
            }),
        );
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
    fn request_string_input_and_instructions() {
        let body = serde_json::json!({
            "model": "openai/gpt-4o",
            "instructions": "be brief",
            "input": "hello",
            "max_output_tokens": 100,
            "temperature": 0.2
        });
        let out = translate_request(&body).unwrap();
        assert_eq!(out["model"], "openai/gpt-4o");
        assert_eq!(out["max_tokens"], 100);
        assert_eq!(out["temperature"], 0.2);
        assert_eq!(out["messages"][0]["role"], "system");
        assert_eq!(out["messages"][0]["content"], "be brief");
        assert_eq!(out["messages"][1]["role"], "user");
        assert_eq!(out["messages"][1]["content"], "hello");
    }

    #[test]
    fn request_array_input_items() {
        let body = serde_json::json!({
            "model": "m",
            "input": [
                {"role": "user", "content": [{"type": "input_text", "text": "part1 "}]},
                {"role": "assistant", "content": [{"type": "output_text", "text": "ok"}]},
                {"role": "user", "content": "again"}
            ]
        });
        let out = translate_request(&body).unwrap();
        let msgs = out["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "part1 ");
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[1]["content"], "ok");
        assert_eq!(msgs[2]["content"], "again");
    }

    #[test]
    fn request_rejects_stateful_knobs() {
        let a = serde_json::json!({"model": "m", "previous_response_id": "resp_1"});
        assert!(translate_request(&a).is_err());
        let b = serde_json::json!({"model": "m", "store": true});
        assert!(translate_request(&b).is_err());
    }

    #[test]
    fn request_maps_responses_tool_shape() {
        let body = serde_json::json!({
            "model": "m",
            "input": "hi",
            "tools": [{
                "type": "function",
                "name": "lookup",
                "description": "find",
                "parameters": {"type": "object"}
            }]
        });
        let out = translate_request(&body).unwrap();
        assert_eq!(out["tools"][0]["type"], "function");
        assert_eq!(out["tools"][0]["function"]["name"], "lookup");
        assert_eq!(out["tools"][0]["function"]["parameters"]["type"], "object");
    }

    #[test]
    fn request_expands_codex_namespace_tools_for_chat_completions() {
        let body = serde_json::json!({
            "model": "m",
            "input": "list purchases",
            "tools": [
                {
                    "type": "namespace",
                    "name": "functions",
                    "tools": [{
                        "type": "function",
                        "name": "exec_command",
                        "description": "run a command",
                        "parameters": {"type": "object"}
                    }]
                },
                {
                    "type": "namespace",
                    "name": "mcp__beyond-openapi__",
                    "tools": [{
                        "type": "function",
                        "name": "list_purchases",
                        "description": "list live purchases",
                        "parameters": {"type": "object"}
                    }]
                }
            ]
        });
        let out = translate_request(&body).unwrap();
        assert_eq!(out["tools"][0]["function"]["name"], "exec_command");
        assert_eq!(
            out["tools"][1]["function"]["name"],
            "mcp__beyond-openapi__list_purchases"
        );
        assert_eq!(
            out["tools"][1]["function"]["description"],
            "list live purchases"
        );
    }

    #[test]
    fn request_preserves_codex_function_call_round_trip() {
        let body = serde_json::json!({
            "model": "gpt-5.6-luna",
            "input": [
                {"type": "message", "role": "user", "content": "inspect it"},
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "shell",
                    "arguments": "{\"cmd\":\"pwd\"}"
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "/workspace"
                }
            ],
            "tool_choice": {"type": "function", "name": "shell"},
            "parallel_tool_calls": true,
            "reasoning": {"effort": "medium"}
        });
        let out = translate_request(&body).unwrap();
        let messages = out["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(
            messages[1]["tool_calls"][0]["function"]["arguments"],
            "{\"cmd\":\"pwd\"}"
        );
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call_1");
        assert_eq!(messages[2]["content"], "/workspace");
        assert_eq!(out["tool_choice"]["function"]["name"], "shell");
        assert_eq!(out["parallel_tool_calls"], true);
        assert_eq!(out["reasoning"]["effort"], "medium");
    }

    #[test]
    fn request_reflattens_namespaced_function_call_history() {
        let body = serde_json::json!({
            "model": "m",
            "input": [{
                "type": "function_call",
                "call_id": "call_1",
                "namespace": "mcp__beyond-openapi__",
                "name": "list_purchases",
                "arguments": "{}"
            }]
        });
        let out = translate_request(&body).unwrap();
        assert_eq!(
            out["messages"][0]["tool_calls"][0]["function"]["name"],
            "mcp__beyond-openapi__list_purchases"
        );
    }

    #[test]
    fn request_translates_current_codex_additional_and_custom_tools() {
        let body = serde_json::json!({
            "model": "gpt-5.6-luna",
            "input": [
                {
                    "type": "additional_tools",
                    "role": "developer",
                    "tools": [{
                        "type": "custom",
                        "name": "exec",
                        "description": "Run JavaScript",
                        "format": {"type": "grammar", "syntax": "lark", "definition": "start: /.+/"}
                    }]
                },
                {"type": "message", "role": "user", "content": "run pwd"},
                {
                    "type": "custom_tool_call",
                    "call_id": "call_exec",
                    "name": "exec",
                    "input": "text(await tools.exec_command({cmd: 'pwd'}).output);"
                },
                {
                    "type": "custom_tool_call_output",
                    "call_id": "call_exec",
                    "output": "/workspace"
                }
            ],
            "store": false,
            "stream": true
        });
        let out = translate_request(&body).unwrap();
        assert_eq!(out["tools"][0]["function"]["name"], "swarm_custom__exec");
        assert_eq!(
            out["tools"][0]["function"]["parameters"]["properties"]["input"]["type"],
            "string"
        );
        let messages = out["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(
            messages[1]["tool_calls"][0]["function"]["name"],
            "swarm_custom__exec"
        );
        assert!(
            messages[1]["tool_calls"][0]["function"]["arguments"]
                .as_str()
                .unwrap()
                .contains("tools.exec_command")
        );
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call_exec");
    }

    #[test]
    fn non_streaming_response_has_output_text_and_usage() {
        let v = serde_json::json!({
            "id": "chatcmpl-abc",
            "model": "m",
            "choices": [{"message": {"content": "hello world"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 4, "completion_tokens": 2, "total_tokens": 6}
        });
        let c = parse_chat_completion(&v);
        let r = completion_to_json(&c);
        assert_eq!(r["object"], "response");
        assert_eq!(r["status"], "completed");
        assert_eq!(r["output_text"], "hello world");
        assert_eq!(r["output"][0]["type"], "message");
        assert_eq!(r["output"][0]["content"][0]["type"], "output_text");
        assert_eq!(r["output"][0]["content"][0]["text"], "hello world");
        assert_eq!(r["usage"]["input_tokens"], 4);
        assert_eq!(r["usage"]["output_tokens"], 2);
        assert_eq!(r["usage"]["total_tokens"], 6);
        assert_eq!(r["id"], "resp_abc");
    }

    #[test]
    fn non_streaming_response_restores_codex_mcp_namespace() {
        let v = serde_json::json!({
            "id": "chatcmpl-tools",
            "model": "m",
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "mcp__beyond-openapi__list_purchases",
                            "arguments": "{}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let c = parse_chat_completion(&v);
        let r = completion_to_json(&c);
        assert_eq!(r["output"][1]["type"], "function_call");
        assert_eq!(r["output"][1]["namespace"], "mcp__beyond-openapi__");
        assert_eq!(r["output"][1]["name"], "list_purchases");
    }

    #[test]
    fn streaming_response_emits_created_delta_completed() {
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"content\":\"lo\"},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
                   data: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\n\
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

        let types: Vec<&str> = out
            .lines()
            .filter_map(|l| l.strip_prefix("event: "))
            .collect();
        assert_eq!(
            types,
            vec![
                "response.created",
                "response.output_item.added",
                "response.content_part.added",
                "response.output_text.delta",
                "response.output_text.delta",
                "response.output_text.done",
                "response.content_part.done",
                "response.output_item.done",
                "response.completed",
            ]
        );
        assert!(out.contains("\"delta\":\"Hel\""));
        assert!(out.contains("\"output_text\":\"Hello\""));
        assert!(out.contains("\"input_tokens\":5"));
        assert!(out.contains("\"total_tokens\":7"));
    }

    #[test]
    fn streaming_response_emits_codex_function_call_events() {
        let sse = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"shell\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"cmd\\\":\\\"pwd\\\"}\"}}]},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n\
                   data: {\"choices\":[],\"usage\":{\"prompt_tokens\":8,\"completion_tokens\":4,\"total_tokens\":12}}\n\n\
                   data: [DONE]\n\n";
        let mut parser = OpenAiSseParser::default();
        let mut state = StreamState::new("openai/gpt-5.6-luna".into());
        let mut out = String::new();
        for event in parser.feed(sse.as_bytes()) {
            state.handle(event, &mut out);
        }
        for event in parser.finish() {
            state.handle(event, &mut out);
        }
        state.finish_stream(&mut out);

        let types: Vec<&str> = out
            .lines()
            .filter_map(|line| line.strip_prefix("event: "))
            .collect();
        assert_eq!(
            types,
            vec![
                "response.created",
                "response.output_item.added",
                "response.function_call_arguments.delta",
                "response.function_call_arguments.done",
                "response.output_item.done",
                "response.completed",
            ]
        );
        assert!(out.contains("\"call_id\":\"call_1\""));
        assert!(out.contains("\"name\":\"shell\""));
        assert!(out.contains("{\\\"cmd\\\":\\\"pwd\\\"}"));
        assert!(out.contains("\"total_tokens\":12"));
    }

    #[test]
    fn streaming_response_restores_codex_mcp_namespace() {
        let sse = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"mcp__beyond-openapi__list_purchases\",\"arguments\":\"{}\"}}]},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n\
                   data: [DONE]\n\n";
        let mut parser = OpenAiSseParser::default();
        let mut state = StreamState::new("m".into());
        let mut out = String::new();
        for event in parser.feed(sse.as_bytes()) {
            state.handle(event, &mut out);
        }
        for event in parser.finish() {
            state.handle(event, &mut out);
        }
        state.finish_stream(&mut out);

        assert!(out.contains("\"namespace\":\"mcp__beyond-openapi__\""));
        assert!(out.contains("\"name\":\"list_purchases\""));
        assert!(!out.contains("\"name\":\"mcp__beyond-openapi__list_purchases\""));
    }

    #[test]
    fn streaming_response_restores_codex_custom_tool_call() {
        let sse = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_exec\",\"function\":{\"name\":\"swarm_custom__exec\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"input\\\":\\\"text(true);\\\"}\"}}]},\"finish_reason\":null}]}\n\n\
                   data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n\
                   data: [DONE]\n\n";
        let mut parser = OpenAiSseParser::default();
        let mut state = StreamState::new("openai/gpt-5.6-luna".into());
        let mut out = String::new();
        for event in parser.feed(sse.as_bytes()) {
            state.handle(event, &mut out);
        }
        state.finish_stream(&mut out);

        assert!(out.contains("event: response.custom_tool_call_input.delta"));
        assert!(out.contains("event: response.custom_tool_call_input.done"));
        assert!(out.contains("\"type\":\"custom_tool_call\""));
        assert!(out.contains("\"name\":\"exec\""));
        assert!(out.contains("\"input\":\"text(true);\""));
        assert!(!out.contains("swarm_custom__exec"));
    }
}
