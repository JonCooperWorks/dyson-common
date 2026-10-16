//! One conversation as a Session Traces file: JSON Lines, a `session`
//! header line and then a `message` line for each message, the format the
//! Hugging Face trace viewer reads.
//!
//! ```text
//! {"type":"session","harness":"dyson","id":"…","name":"…","first_seen":"…","last_seen":"…"}
//! {"type":"message","message":{"role":"user","content":"…"}}
//! {"type":"message","message":{"role":"assistant","content":"","toolCalls":[{"id":"…","function":{"name":"…","arguments":"…"}}]}}
//! {"type":"message","message":{"role":"tool","content":"…","toolCallId":"…"}}
//! ```
//!
//! A session is plain serde data, so it can be redacted as a JSON document
//! before it is written (see `dyson_secrets::scrub`):
//! `serde_json::to_value(&session)`, scrub, `serde_json::from_value`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Characters of the first user message a session is named with.
const NAME_MOST: usize = 80;

/// What a session file says of the conversation as a whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    /// What ran the conversation: `dyson`, a harness's name.
    pub harness: String,
    pub id: String,
    /// What the viewer lists it as; see [`Session::named_from_first_user`].
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub first_seen: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_seen: Option<DateTime<Utc>>,
    /// Anything else the host records of it (models, turns, a format
    /// version), written beside the fields above.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Who wrote a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    /// A role as the provider APIs name it: OpenAI's `developer` is a
    /// system message, Gemini's `model` an assistant's, `function` a
    /// tool's; anything else is the user's.
    pub fn from_wire(role: &str) -> Self {
        match role {
            "system" | "developer" => Self::System,
            "assistant" | "model" => Self::Assistant,
            "tool" | "function" => Self::Tool,
            _ => Self::User,
        }
    }
}

/// A tool call an assistant message made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub function: Function,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    /// As the model wrote them: usually a JSON object as text.
    pub arguments: String,
}

/// One message of the conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// For a tool message: the call it answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// When it was written, in milliseconds since the epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    /// The model that wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Message {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            reasoning_content: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
            timestamp: None,
            model: None,
        }
    }
}

/// A conversation, ready to be written as one file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub header: Header,
    pub messages: Vec<Message>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Line {
    Session(Header),
    Message { message: Message },
}

/// Why a file could not be read as a session.
#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

impl Session {
    pub fn new(harness: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            header: Header {
                harness: harness.into(),
                id: id.into(),
                name: None,
                first_seen: None,
                last_seen: None,
                extra: Map::new(),
            },
            messages: Vec::new(),
        }
    }

    /// Name the session with the first line of its first user message
    /// that says anything, cut to 80 characters. Left as it is when there
    /// is none.
    pub fn named_from_first_user(mut self) -> Self {
        let name = self
            .messages
            .iter()
            .find(|message| message.role == Role::User && !message.content.trim().is_empty())
            .map(|message| clipped(&message.content, NAME_MOST))
            .filter(|name| !name.is_empty());
        if name.is_some() {
            self.header.name = name;
        }
        self
    }

    /// The file: the header line, then a line a message, each ending in a
    /// newline.
    pub fn to_jsonl(&self) -> String {
        let mut out = line(&Line::Session(self.header.clone()));
        for message in &self.messages {
            out.push_str(&line(&Line::Message {
                message: message.clone(),
            }));
        }
        out
    }

    /// A file read back. Blank lines are skipped.
    pub fn from_jsonl(text: &str) -> Result<Self, ParseError> {
        let mut lines = text.lines().filter(|line| !line.trim().is_empty());
        let header = match lines.next().map(serde_json::from_str::<Line>) {
            Some(Ok(Line::Session(header))) => header,
            Some(Ok(Line::Message { .. })) | None => {
                return Err(ParseError(
                    "a session file starts with its session line".into(),
                ));
            }
            Some(Err(e)) => return Err(ParseError(format!("the session line: {e}"))),
        };
        let mut messages = Vec::new();
        for (n, text) in lines.enumerate() {
            match serde_json::from_str::<Line>(text) {
                Ok(Line::Message { message }) => messages.push(message),
                Ok(Line::Session(_)) => {
                    return Err(ParseError(format!("line {}: a second session line", n + 2)));
                }
                Err(e) => return Err(ParseError(format!("line {}: {e}", n + 2))),
            }
        }
        Ok(Self { header, messages })
    }
}

fn line(line: &Line) -> String {
    let mut out = serde_json::to_string(line).expect("a session serializes");
    out.push('\n');
    out
}

/// The first line of `text` that says anything, cut to `most` characters.
fn clipped(text: &str, most: usize) -> String {
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim();
    if line.chars().count() <= most {
        line.to_owned()
    } else {
        let mut cut: String = line.chars().take(most).collect();
        cut.push('…');
        cut
    }
}

/// Where a session's file is kept in a bucket: under `prefix`, by the day
/// it began, `sessions/2026/10/15/<id>.jsonl`. `None` when the id holds a
/// character other than letters, digits, `-`, `_` and `.`, which an object
/// key is kept free of.
pub fn object_key(prefix: &str, first_seen: DateTime<Utc>, id: &str) -> Option<String> {
    let plain = |s: &str, also: &[char]| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') || also.contains(&c))
    };
    let prefix = prefix.trim_matches('/');
    (!id.is_empty() && !id.starts_with('.') && plain(id, &[]) && plain(prefix, &['/'])).then(|| {
        let day = first_seen.format("%Y/%m/%d");
        if prefix.is_empty() {
            format!("{day}/{id}.jsonl")
        } else {
            format!("{prefix}/{day}/{id}.jsonl")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn conversation() -> Session {
        let mut session = Session::new("dyson", "c0ffee");
        session.header.first_seen = Some(at("2026-10-15T09:00:00Z"));
        session.header.last_seen = Some(at("2026-10-15T09:05:00Z"));
        session.header.extra.insert("models".into(), json!(["m1"]));
        session.messages = vec![
            Message::new(Role::from_wire("developer"), "be brief"),
            Message::new(Role::User, "\n  List the files\nin /tmp please"),
            Message {
                tool_calls: vec![ToolCall {
                    id: "call_1".into(),
                    function: Function {
                        name: "ls".into(),
                        arguments: "{\"path\":\"/tmp\"}".into(),
                    },
                }],
                reasoning_content: Some("use ls".into()),
                ..Message::new(Role::Assistant, "")
            },
            Message {
                tool_call_id: Some("call_1".into()),
                ..Message::new(Role::from_wire("function"), "a.txt")
            },
            Message {
                timestamp: Some(1_792_000_000_000),
                model: Some("m1".into()),
                ..Message::new(Role::from_wire("model"), "a.txt is there")
            },
        ];
        session.named_from_first_user()
    }

    #[test]
    fn writes_the_header_then_a_line_a_message() {
        let file = conversation().to_jsonl();
        let lines: Vec<Value> = file
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(file.ends_with('\n'));
        assert_eq!(lines.len(), 6);
        assert_eq!(
            lines[0],
            json!({
                "type": "session", "harness": "dyson", "id": "c0ffee",
                "name": "List the files",
                "first_seen": "2026-10-15T09:00:00Z", "last_seen": "2026-10-15T09:05:00Z",
                "models": ["m1"],
            })
        );
        assert_eq!(
            lines[1],
            json!({"type": "message", "message": {"role": "system", "content": "be brief"}})
        );
        assert_eq!(
            lines[3]["message"],
            json!({
                "role": "assistant", "content": "", "reasoningContent": "use ls",
                "toolCalls": [{"id": "call_1", "function": {"name": "ls", "arguments": "{\"path\":\"/tmp\"}"}}],
            })
        );
        assert_eq!(
            lines[4]["message"],
            json!({"role": "tool", "content": "a.txt", "toolCallId": "call_1"})
        );
        assert_eq!(
            lines[5]["message"],
            json!({"role": "assistant", "content": "a.txt is there", "timestamp": 1_792_000_000_000_i64, "model": "m1"})
        );
    }

    #[test]
    fn reads_back_what_it_wrote() {
        let session = conversation();
        assert_eq!(Session::from_jsonl(&session.to_jsonl()).unwrap(), session);
        assert!(Session::from_jsonl("").is_err());
        assert!(
            Session::from_jsonl(
                "{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"x\"}}"
            )
            .is_err()
        );
    }

    #[test]
    fn a_long_first_message_is_cut_for_the_name() {
        let mut session = Session::new("dyson", "x");
        session.messages = vec![Message::new(Role::User, "word ".repeat(40))];
        let name = session.named_from_first_user().header.name.unwrap();
        assert_eq!(name.chars().count(), NAME_MOST + 1);
        assert!(name.ends_with('…'));
        let unnamed = Session::new("dyson", "x").named_from_first_user();
        assert_eq!(unnamed.header.name, None);
    }

    #[test]
    fn keys_files_by_the_day_and_keeps_keys_plain() {
        let day = at("2026-10-15T23:59:59Z");
        assert_eq!(
            object_key("sessions", day, "c0ffee-1").as_deref(),
            Some("sessions/2026/10/15/c0ffee-1.jsonl")
        );
        assert_eq!(
            object_key("/a/b/", day, "x").as_deref(),
            Some("a/b/2026/10/15/x.jsonl")
        );
        assert_eq!(
            object_key("", day, "x").as_deref(),
            Some("2026/10/15/x.jsonl")
        );
        assert_eq!(object_key("sessions", day, "../x"), None);
        assert_eq!(object_key("sessions", day, "a/b"), None);
        assert_eq!(object_key("sessions", day, ""), None);
        assert_eq!(object_key("s p", day, "x"), None);
    }

    #[test]
    fn a_session_is_redacted_as_a_document() {
        let mut document = serde_json::to_value(conversation()).unwrap();
        document["messages"][1]["content"] = json!("[REDACTED:x]");
        let session: Session = serde_json::from_value(document).unwrap();
        assert_eq!(session.messages[1].content, "[REDACTED:x]");
    }
}
