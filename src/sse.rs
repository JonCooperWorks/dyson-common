//! An incremental `text/event-stream` decoder, per the WHATWG HTML
//! "event stream interpretation" rules, for every place that reads SSE off
//! a byte stream (provider streams, MCP Streamable HTTP, A2A).
//!
//! Feed it chunks as they arrive with [`SseDecoder::push`]; it returns the
//! events each chunk completes. Lines may end in `\n`, `\r\n` or `\r`, even
//! when the `\r\n` pair is split across chunks, and a multi-byte character
//! split across chunks is kept whole. A field's value loses one leading
//! space; `data` lines are joined with `\n`; comment lines (`:`) and
//! unknown fields are ignored; an event with no `data` is not dispatched;
//! an event the stream ends in the middle of is dropped.
//!
//! The decoder holds at most `limit` bytes of an unfinished line and event
//! (1 MiB by default, see [`SseDecoder::with_limit`]): a peer that never
//! ends a line or an event gets [`SseTooLarge`] rather than the process's
//! memory.

/// One dispatched event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// The `event` field, `message` when none was given.
    pub event: String,
    /// The `data` lines, joined with `\n`.
    pub data: String,
    /// The last event id the stream set (it carries over to later events,
    /// as `Last-Event-ID` does).
    pub id: Option<String>,
    /// A `retry` this event set, in milliseconds.
    pub retry: Option<u64>,
}

/// The stream held more than the decoder's limit of one unfinished line
/// and event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SseTooLarge {
    pub limit: usize,
}

impl std::fmt::Display for SseTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "an SSE event longer than {} bytes", self.limit)
    }
}

impl std::error::Error for SseTooLarge {}

/// The default most of one unfinished line and event held.
pub const SSE_DEFAULT_LIMIT: usize = 1024 * 1024;

/// See the [module docs](self).
#[derive(Debug, Clone)]
pub struct SseDecoder {
    limit: usize,
    /// The unfinished line, raw.
    line: Vec<u8>,
    /// The last chunk ended in `\r`: a `\n` starting the next one belongs
    /// to it.
    after_cr: bool,
    event: Option<String>,
    data: String,
    has_data: bool,
    retry: Option<u64>,
    last_id: Option<String>,
}

impl Default for SseDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::with_limit(SSE_DEFAULT_LIMIT)
    }

    /// A decoder holding at most `limit` bytes of an unfinished line and
    /// event.
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            line: Vec::new(),
            after_cr: false,
            event: None,
            data: String::new(),
            has_data: false,
            retry: None,
            last_id: None,
        }
    }

    /// The last event id the stream set, for a reconnect's `Last-Event-ID`.
    pub fn last_event_id(&self) -> Option<&str> {
        self.last_id.as_deref()
    }

    /// The events `chunk` completes, in order.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>, SseTooLarge> {
        let mut out = Vec::new();
        let mut rest = chunk;
        if self.after_cr {
            self.after_cr = false;
            if let Some(stripped) = rest.strip_prefix(b"\n") {
                rest = stripped;
            }
        }
        while let Some(at) = rest.iter().position(|&b| b == b'\n' || b == b'\r') {
            self.line.extend_from_slice(&rest[..at]);
            self.check()?;
            let line = std::mem::take(&mut self.line);
            if let Some(event) = self.line_done(&String::from_utf8_lossy(&line)) {
                out.push(event);
            }
            let ended_cr = rest[at] == b'\r';
            rest = &rest[at + 1..];
            if ended_cr {
                match rest.first() {
                    Some(b'\n') => rest = &rest[1..],
                    None => self.after_cr = true,
                    Some(_) => {}
                }
            }
        }
        self.line.extend_from_slice(rest);
        self.check()?;
        Ok(out)
    }

    fn check(&self) -> Result<(), SseTooLarge> {
        if self.line.len() + self.data.len() > self.limit {
            Err(SseTooLarge { limit: self.limit })
        } else {
            Ok(())
        }
    }

    fn line_done(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            return self.dispatch();
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = Some(value.to_owned()),
            "data" => {
                if self.has_data {
                    self.data.push('\n');
                }
                self.data.push_str(value);
                self.has_data = true;
            }
            "id" if !value.contains('\0') => self.last_id = Some(value.to_owned()),
            "retry" if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                self.retry = value.parse().ok();
            }
            _ => {}
        }
        None
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        let event = self.event.take();
        let retry = self.retry.take();
        let data = std::mem::take(&mut self.data);
        if !std::mem::replace(&mut self.has_data, false) {
            return None;
        }
        Some(SseEvent {
            event: event
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "message".into()),
            data,
            id: self.last_id.clone(),
            retry,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(decoder: &mut SseDecoder, chunks: &[&[u8]]) -> Vec<SseEvent> {
        chunks
            .iter()
            .flat_map(|chunk| decoder.push(chunk).unwrap())
            .collect()
    }

    #[test]
    fn decodes_fields_comments_and_multiline_data() {
        let mut decoder = SseDecoder::new();
        let events = all(
            &mut decoder,
            &[b": hello\nevent: tool_call\ndata: {\"a\":1}\ndata:second\nid: 7\nretry: 1500\nbogus: x\n\ndata\n\n"],
        );
        assert_eq!(
            events,
            vec![
                SseEvent {
                    event: "tool_call".into(),
                    data: "{\"a\":1}\nsecond".into(),
                    id: Some("7".into()),
                    retry: Some(1500),
                },
                SseEvent {
                    event: "message".into(),
                    data: String::new(),
                    id: Some("7".into()),
                    retry: None,
                },
            ]
        );
        assert_eq!(decoder.last_event_id(), Some("7"));
    }

    #[test]
    fn any_line_ending_and_any_split() {
        let text = "event: a\r\ndata: x\r\n\r\nevent: b\rdata: y\r\rdata: z\n\n";
        let whole = all(&mut SseDecoder::new(), &[text.as_bytes()]);
        assert_eq!(
            whole
                .iter()
                .map(|e| (e.event.as_str(), e.data.as_str()))
                .collect::<Vec<_>>(),
            vec![("a", "x"), ("b", "y"), ("message", "z")]
        );
        // Every split point, including inside \r\n and inside a character.
        let text = "data: h\u{e9}llo\r\n\r\ndata: \u{1f600}\r\n\r\n";
        let bytes = text.as_bytes();
        for at in 0..=bytes.len() {
            let events = all(&mut SseDecoder::new(), &[&bytes[..at], &bytes[at..]]);
            assert_eq!(
                events.iter().map(|e| e.data.as_str()).collect::<Vec<_>>(),
                vec!["h\u{e9}llo", "\u{1f600}"],
                "split at {at}"
            );
        }
    }

    #[test]
    fn no_data_no_event_and_an_unfinished_event_is_dropped() {
        let events = all(
            &mut SseDecoder::new(),
            &[b"event: ping\n\n: keepalive\n\ndata: cut"],
        );
        assert!(events.is_empty(), "{events:?}");
    }

    #[test]
    fn a_line_or_event_over_the_limit_is_an_error() {
        let mut decoder = SseDecoder::with_limit(16);
        assert!(decoder.push(b"data: 0123456789").is_ok());
        assert_eq!(decoder.push(b"abcdef"), Err(SseTooLarge { limit: 16 }));
        let mut decoder = SseDecoder::with_limit(16);
        assert!(decoder.push(b"data: 0123456789\n").is_ok());
        assert!(decoder.push(b"data: 0123456789\n").is_err());
    }

    #[test]
    fn an_id_with_nul_is_ignored_and_retry_must_be_digits() {
        let events = all(
            &mut SseDecoder::new(),
            &[b"id: 1\nid: a\0b\nretry: 12x\ndata: d\n\n"],
        );
        assert_eq!(events[0].id.as_deref(), Some("1"));
        assert_eq!(events[0].retry, None);
    }
}
