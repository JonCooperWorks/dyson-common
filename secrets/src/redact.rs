//! Taking known secrets out of text and JSON documents, however they are
//! written.
//!
//! A secret is looked for as written, JSON-escaped (once, and twice for a
//! JSON string inside another), percent-encoded, and inside base64 at each
//! of the three alignments (standard and URL-safe alphabets), so it is
//! found in a tool call's arguments, a query string or a Basic
//! `Authorization` header. It is replaced with a token naming its rule and
//! enough of a keyed digest to tell that two places held the same one:
//! `[REDACTED:github-pat:3fa9c1d2]`.
//!
//! Redacting what is already known is not enough before text leaves the
//! box: a detector may find more in what is left. [`scrub`] redacts, scans
//! again with the caller's detectors, takes out whatever they still find,
//! and repeats; a document that still shows a secret after the last round,
//! or shows one that is in none of its strings verbatim, is held back with
//! the reason instead of being passed.
//!
//! Dependency-light (no detector here): pair it with
//! [`Scanner`](crate::Scanner) for the rescans, or with any detector of
//! your own.

use std::{
    collections::{BTreeSet, HashSet},
    sync::{Arc, LazyLock},
};

use aho_corasick::{AhoCorasick, MatchKind};
use base64::Engine as _;
use serde_json::Value;

use crate::Error;
use crate::text::entropy;

/// A value shorter than this is never looked for.
pub const SHORTEST: usize = 4;
/// A value shorter than this is not [`distinctive`].
const SHORTEST_ANYWHERE: usize = 8;
/// So is one this predictable (bits a character), unless it is long: a
/// password like `changeme` is not taken out of every other document.
const ENTROPY_ANYWHERE: f64 = 3.0;
const LONG: usize = 20;
/// A value this long is also looked for inside base64.
const SHORTEST_ENCODED: usize = 12;

/// The token a secret is replaced with: its rule, and the first eight hex
/// digits of its digest (see [`digest`]).
pub fn token(rule_id: &str, digest: &str) -> String {
    format!(
        "[REDACTED:{}:{}]",
        rule_name(rule_id),
        digest.get(..8).unwrap_or(digest)
    )
}

/// A token with a label and no digest, for a value that is not a secret
/// but is still taken out: `[REDACTED:endpoint]`.
pub fn label_token(label: &str) -> String {
    format!("[REDACTED:{}]", rule_name(label))
}

/// A rule as a token names it: the characters a token may hold, capped.
fn rule_name(rule_id: &str) -> String {
    let rule: String = rule_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .take(48)
        .collect();
    if rule.is_empty() {
        "secret".into()
    } else {
        rule
    }
}

/// A keyed digest of a value, as lowercase hex: HMAC-SHA256 under a key
/// the host keeps, so a token tells two places held the same secret
/// without letting anyone confirm a guess of it.
pub fn digest(key: &[u8], value: &str) -> String {
    use hmac::Mac as _;
    let mut mac =
        hmac::Hmac::<sha2::Sha256>::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(value.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

static TOKEN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\[REDACTED:[A-Za-z0-9._-]{1,48}(?::[0-9a-f]{1,8})?\]").expect("token")
});

/// Whether a value is distinctive enough to be taken out of every
/// document, not only the one it was found in: long enough, and either
/// long or unpredictable. A password like `changeme` found in one place
/// would otherwise blank the word everywhere.
pub fn distinctive(value: &str) -> bool {
    let value = value.trim();
    let length = value.chars().count();
    length >= SHORTEST_ANYWHERE && (length >= LONG || entropy(value) >= ENTROPY_ANYWHERE)
}

/// What percent-encoding leaves alone: the unreserved characters.
const ENCODED: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// The base64 of `value` wherever it starts in what was encoded: the three
/// alignments, each without the characters its neighbours share.
fn in_base64(value: &str, out: &mut Vec<String>) {
    let bytes = value.as_bytes();
    if bytes.len() < SHORTEST_ENCODED {
        return;
    }
    for lead in 0..3usize {
        let mut padded = vec![0u8; lead];
        padded.extend_from_slice(bytes);
        let encoded = base64::engine::general_purpose::STANDARD_NO_PAD.encode(&padded);
        // The first characters carry the bytes before it; the last one
        // carries the byte after it unless the length is a whole group.
        let skip = [0, 2, 3][lead];
        let drop = if padded.len() % 3 == 0 { 0 } else { 1 };
        let Some(needle) = encoded.get(skip..encoded.len() - drop) else {
            continue;
        };
        if needle.len() < SHORTEST_ENCODED {
            continue;
        }
        out.push(needle.to_owned());
        if needle.contains(['+', '/']) {
            out.push(needle.replace('+', "-").replace('/', "_"));
        }
    }
}

/// Every form of a value that is looked for.
pub fn needles(value: &str) -> Vec<String> {
    let mut out = vec![value.to_owned()];
    if let Ok(quoted) = serde_json::to_string(value) {
        let escaped = &quoted[1..quoted.len() - 1];
        if escaped != value {
            // Once for a JSON string, twice for one inside another.
            out.push(escaped.to_owned());
            if let Ok(again) = serde_json::to_string(escaped) {
                out.push(again[1..again.len() - 1].to_owned());
            }
        }
    }
    let encoded = percent_encoding::utf8_percent_encode(value, ENCODED).to_string();
    if encoded != value {
        out.push(encoded);
    }
    in_base64(value, &mut out);
    out
}

/// A set of values and what each is replaced with.
struct Matcher {
    found: AhoCorasick,
    tokens: Vec<Arc<str>>,
}

impl Matcher {
    /// `None` when there is nothing to look for.
    fn of(
        values: impl IntoIterator<Item = (String, Arc<str>)>,
        any_case: bool,
    ) -> Result<Option<Self>, Error> {
        let mut seen = HashSet::new();
        let (mut patterns, mut tokens) = (Vec::new(), Vec::new());
        for (pattern, token) in values {
            if pattern.len() >= SHORTEST && seen.insert(pattern.clone()) {
                patterns.push(pattern);
                tokens.push(token);
            }
        }
        if patterns.is_empty() {
            return Ok(None);
        }
        let found = AhoCorasick::builder()
            .match_kind(MatchKind::LeftmostLongest)
            .ascii_case_insensitive(any_case)
            .build(&patterns)
            .map_err(|e| Error(format!("build the redaction matcher: {e}")))?;
        Ok(Some(Self { found, tokens }))
    }

    /// `text` with every match replaced, when there is one.
    fn replace(&self, text: &str, count: &mut usize) -> Option<String> {
        let mut out: Option<String> = None;
        let mut last = 0;
        for found in self.found.find_iter(text) {
            let out = out.get_or_insert_with(|| String::with_capacity(text.len()));
            out.push_str(&text[last..found.start()]);
            out.push_str(&self.tokens[found.pattern().as_usize()]);
            last = found.end();
            *count += 1;
        }
        out.map(|mut out| {
            out.push_str(&text[last..]);
            out
        })
    }
}

/// What a [`Redactor`] takes out; see [`Redactor::builder`].
#[derive(Default)]
pub struct RedactorBuilder {
    secrets: Vec<(String, Arc<str>)>,
    any_case: Vec<(String, Arc<str>)>,
}

impl RedactorBuilder {
    /// A secret, in every form it may be written (see [`needles`]),
    /// replaced with `token`. Whether a short or predictable value should
    /// be taken out of everything is the caller's to decide (see
    /// [`distinctive`]).
    pub fn secret(&mut self, value: &str, token: impl Into<Arc<str>>) -> &mut Self {
        let token = token.into();
        self.secrets.extend(
            needles(value.trim())
                .into_iter()
                .map(|needle| (needle, token.clone())),
        );
        self
    }

    /// A value taken out as written, whatever its ASCII case: a hostname.
    /// Applied after the secrets.
    pub fn any_case(&mut self, value: &str, token: impl Into<Arc<str>>) -> &mut Self {
        self.any_case.push((value.trim().to_owned(), token.into()));
        self
    }

    pub fn build(&mut self) -> Result<Redactor, Error> {
        Ok(Redactor {
            secrets: Matcher::of(std::mem::take(&mut self.secrets), false)?,
            any_case: Matcher::of(std::mem::take(&mut self.any_case), true)?,
        })
    }
}

/// Known values, taken out of text wherever they occur.
#[derive(Default)]
pub struct Redactor {
    secrets: Option<Matcher>,
    any_case: Option<Matcher>,
}

impl Redactor {
    pub fn builder() -> RedactorBuilder {
        RedactorBuilder::default()
    }

    /// Whether there is nothing to take out.
    pub fn is_empty(&self) -> bool {
        self.secrets.is_none() && self.any_case.is_none()
    }

    fn change(&self, text: &str, count: &mut usize) -> Option<String> {
        let first = self
            .secrets
            .as_ref()
            .and_then(|secrets| secrets.replace(text, count));
        let second = self
            .any_case
            .as_ref()
            .and_then(|values| values.replace(first.as_deref().unwrap_or(text), count));
        second.or(first)
    }

    /// `text` redacted, and how many values were taken out of it.
    pub fn text(&self, text: &str) -> (String, usize) {
        let mut count = 0;
        let out = self
            .change(text, &mut count)
            .unwrap_or_else(|| text.to_owned());
        (out, count)
    }

    /// Every string of a document, redacted in place. How many were found.
    /// Keys are left: they are field names.
    pub fn document(&self, document: &mut Value) -> usize {
        let mut count = 0;
        strings(document, &mut |text| self.change(text, &mut count));
        count
    }
}

/// Apply `change` to every string value of a document. Keys are left.
pub fn strings(value: &mut Value, change: &mut impl FnMut(&str) -> Option<String>) {
    match value {
        Value::String(text) => {
            if let Some(changed) = change(text) {
                *text = changed;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| strings(item, change)),
        Value::Object(fields) => fields.values_mut().for_each(|field| strings(field, change)),
        _ => {}
    }
}

/// Every redaction token a document holds.
pub fn tokens_in(document: &Value) -> HashSet<String> {
    let mut found = HashSet::new();
    let mut walk = document.clone();
    strings(&mut walk, &mut |text| {
        found.extend(TOKEN.find_iter(text).map(|token| token.as_str().to_owned()));
        None
    });
    found
}

/// Whether what a detector found is a redaction token, or part of one, or
/// something around one: nothing of a secret. `present` is what
/// [`tokens_in`] found in the document scanned.
pub fn is_token(present: &HashSet<String>, value: &str) -> bool {
    TOKEN.is_match(value) || present.iter().any(|token| token.contains(value))
}

/// A secret a rescan found in a redacted document.
#[derive(Debug, Clone, PartialEq)]
pub struct Leftover {
    pub value: String,
    pub rule_id: String,
    /// What it is replaced with (see [`token`]).
    pub token: String,
    /// Whether it is in what readers of the document see. One that is not
    /// (a stitched-together value the scan found in no string verbatim,
    /// in metadata that is dropped) holds the document back only when it
    /// is shown.
    pub shown: bool,
}

/// What became of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scrubbed {
    /// Nothing more was found: the document may go. How many values were
    /// taken out of it, in all.
    Clean { redactions: usize },
    /// It may not go, and why.
    Held(String),
}

/// Redact `document` with what is known, then scan it with `scan` and take
/// out whatever is still found, up to `rounds` times.
///
/// `scan` sees the document as it stands and returns every secret it
/// finds; it should leave out what the caller has marked as a false
/// positive. What is found is taken out of this document whatever its
/// length or how predictable it is. A scan that fails (a detector could
/// not run) is an error and the document is not to be passed.
pub fn scrub<E>(
    document: &mut Value,
    redactor: &Redactor,
    rounds: usize,
    mut scan: impl FnMut(&Value) -> Result<Vec<Leftover>, E>,
) -> Result<Scrubbed, E> {
    let mut redactions = redactor.document(document);
    for round in 0..=rounds {
        let present = tokens_in(document);
        let left: Vec<Leftover> = scan(document)?
            .into_iter()
            .filter(|found| found.value.trim().chars().count() >= SHORTEST)
            .filter(|found| !is_token(&present, found.value.trim()))
            .collect();
        if left.is_empty() {
            return Ok(Scrubbed::Clean { redactions });
        }
        let rules = || {
            let rules: BTreeSet<&str> = left
                .iter()
                .filter(|found| found.shown)
                .map(|found| found.rule_id.as_str())
                .collect();
            rules.into_iter().take(6).collect::<Vec<_>>().join(", ")
        };
        if round == rounds {
            return Ok(Scrubbed::Held(format!(
                "still shows a secret after {rounds} rounds ({})",
                rules()
            )));
        }
        let mut builder = Redactor::builder();
        for found in &left {
            builder.secret(&found.value, found.token.as_str());
        }
        let local = match builder.build() {
            Ok(local) => local,
            Err(e) => return Ok(Scrubbed::Held(e.to_string())),
        };
        let count = local.document(document);
        if count == 0 {
            // What the scan found is in none of the strings as they stand
            // (another encoding, or stitched from several). Only what is
            // shown keeps the document back.
            return Ok(if left.iter().any(|found| found.shown) {
                Scrubbed::Held(format!(
                    "shows a secret that is not in it verbatim ({})",
                    rules()
                ))
            } else {
                Scrubbed::Clean { redactions }
            });
        }
        redactions += count;
    }
    unreachable!("the last round returns")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const KEY: &[u8] = b"k";

    fn redactor(values: &[&str], hosts: &[&str]) -> Redactor {
        let mut builder = Redactor::builder();
        for value in values.iter().filter(|value| distinctive(value)) {
            builder.secret(value, token("test.rule", &digest(KEY, value)));
        }
        for host in hosts {
            builder.any_case(host, label_token("endpoint"));
        }
        builder.build().unwrap()
    }

    #[test]
    fn a_secret_is_taken_out_wherever_and_however_it_is_written() {
        let secret = "sk-live-9f8e7d6c5b4a39281706f5e4d3c2b1a0";
        let redactor = redactor(&[secret], &[]);
        let token = token("test.rule", &digest(KEY, secret));
        let encoded = base64::engine::general_purpose::STANDARD.encode(format!("user:{secret}\n"));
        let mut document = json!({
            "plain": format!("export KEY={secret} and again {secret}"),
            "nested": [{"arguments": format!("{{\"key\":\"{secret}\"}}")}],
            "basic": format!("Authorization: Basic {encoded}"),
            "url": format!("https://x.test/?k={secret}"),
        });
        let count = redactor.document(&mut document);
        let text = document.to_string();
        assert!(!text.contains(secret), "{text}");
        assert!(
            !text.contains(&encoded),
            "the base64 of it is cut too: {text}"
        );
        assert_eq!(
            document["plain"],
            json!(format!("export KEY={token} and again {token}"))
        );
        assert!(count >= 5, "{count}");
    }

    #[test]
    fn escaped_and_encoded_forms_are_found() {
        let secret = "-----BEGIN KEY-----\nabc/def+ghi\"jkl\n-----END KEY-----";
        let redactor = redactor(&[secret], &[]);
        let inside = serde_json::to_string(&json!({"pem": secret})).unwrap();
        let twice = serde_json::to_string(&json!({"arguments": inside})).unwrap();
        let encoded = percent_encoding::utf8_percent_encode(secret, ENCODED).to_string();
        let mut document = json!({"a": inside, "b": twice, "c": encoded});
        redactor.document(&mut document);
        let text = document.to_string();
        assert!(!text.contains("abc/def"), "{text}");
        assert!(!text.contains("abc%2Fdef"), "{text}");
    }

    #[test]
    fn a_predictable_or_short_value_is_not_distinctive() {
        assert!(!distinctive("changeme"));
        assert!(!distinctive("aaaaaaaaaaaa"));
        assert!(!distinctive("s3cr3t"));
        assert!(distinctive("tvly-dev-A1b2C3d4E5"));
        assert!(distinctive("correct horse battery staple"));
        let redactor = redactor(&["password"], &[]);
        let mut document = json!("enter your password here");
        assert_eq!(redactor.document(&mut document), 0);
        assert!(redactor.is_empty());
    }

    #[test]
    fn hostnames_go_whatever_their_case() {
        let redactor = redactor(&[], &["api.internal.example"]);
        let mut document =
            json!("curl https://API.Internal.Example/v1/models and api.internal.example again");
        assert_eq!(redactor.document(&mut document), 2);
        assert_eq!(
            document,
            json!("curl https://[REDACTED:endpoint]/v1/models and [REDACTED:endpoint] again")
        );
    }

    #[test]
    fn plain_text_is_redacted_too() {
        let secret = "sk-live-9f8e7d6c5b4a39281706f5e4d3c2b1a0";
        let (out, count) =
            redactor(&[secret], &[]).text(&format!("upstream said: bad key {secret}"));
        assert_eq!(count, 1);
        assert!(
            out.starts_with("upstream said: bad key [REDACTED:test.rule:"),
            "{out}"
        );
    }

    #[test]
    fn a_token_is_not_a_secret() {
        let present: HashSet<String> = ["[REDACTED:generic-api-key:3fa9c1d2]".to_owned()].into();
        assert!(is_token(&present, "generic-api-key:3fa9c1d2"));
        assert!(is_token(
            &present,
            "KEY=[REDACTED:generic-api-key:3fa9c1d2]"
        ));
        assert!(!is_token(&present, "sk-live-9f8e7d6c5b4a"));
        assert_eq!(label_token("endpoint"), "[REDACTED:endpoint]");
        assert!(TOKEN.is_match(&label_token("endpoint")));
        assert!(TOKEN.is_match(&token("x", &digest(KEY, "y"))));
    }

    fn found(value: &str, shown: bool) -> Leftover {
        Leftover {
            value: value.into(),
            rule_id: "test.rule".into(),
            token: token("test.rule", &digest(KEY, value)),
            shown,
        }
    }

    /// A detector that finds each of `values` wherever it is still written.
    fn finds(values: &[&'static str]) -> impl FnMut(&Value) -> Result<Vec<Leftover>, ()> {
        move |document: &Value| {
            let text = document.to_string();
            Ok(values
                .iter()
                .filter(|value| text.contains(*value))
                .map(|value| found(value, true))
                .collect())
        }
    }

    #[test]
    fn what_a_rescan_finds_is_taken_out_and_the_document_passes() {
        let known = "sk-live-9f8e7d6c5b4a39281706f5e4d3c2b1a0";
        let mut document = json!({"m": format!("{known} and pw=hunter22 too")});
        let outcome = scrub(
            &mut document,
            &redactor(&[known], &[]),
            3,
            finds(&["hunter22"]),
        );
        assert_eq!(outcome, Ok(Scrubbed::Clean { redactions: 2 }));
        let text = document.to_string();
        assert!(
            !text.contains(known) && !text.contains("hunter22"),
            "{text}"
        );
    }

    #[test]
    fn a_secret_that_is_not_there_verbatim_holds_the_document_back() {
        let mut document = json!({"m": "nothing here"});
        let outcome = scrub(&mut document, &Redactor::default(), 3, |_: &Value| {
            Ok::<_, ()>(vec![found("assembled-elsewhere", true)])
        });
        assert!(
            matches!(&outcome, Ok(Scrubbed::Held(why)) if why.contains("not in it verbatim")),
            "{outcome:?}"
        );
        // Unless it is not shown.
        let outcome = scrub(&mut document, &Redactor::default(), 3, |_: &Value| {
            Ok::<_, ()>(vec![found("assembled-elsewhere", false)])
        });
        assert_eq!(outcome, Ok(Scrubbed::Clean { redactions: 0 }));
    }

    #[test]
    fn a_secret_that_keeps_coming_back_holds_the_document_back() {
        // Each round the scan finds one more.
        let mut document = json!({"m": "abcd-1 abcd-2 abcd-3 abcd-4"});
        let mut n = 0;
        let outcome = scrub(&mut document, &Redactor::default(), 2, |_: &Value| {
            n += 1;
            Ok::<_, ()>(vec![found(&format!("abcd-{n}"), true)])
        });
        assert!(
            matches!(&outcome, Ok(Scrubbed::Held(why)) if why.contains("after 2 rounds")),
            "{outcome:?}"
        );
    }

    #[test]
    fn a_failed_scan_is_an_error() {
        let mut document = json!({"m": "x"});
        let outcome = scrub(&mut document, &Redactor::default(), 3, |_: &Value| {
            Err::<Vec<Leftover>, _>("detector down")
        });
        assert_eq!(outcome, Err("detector down"));
    }

    #[test]
    fn a_token_found_by_the_rescan_is_left() {
        let known = "sk-live-9f8e7d6c5b4a39281706f5e4d3c2b1a0";
        let mut document = json!({"m": known});
        let outcome = scrub(
            &mut document,
            &redactor(&[known], &[]),
            3,
            |document: &Value| {
                // A generic rule reading the token as a key.
                let text = document["m"].as_str().unwrap().to_owned();
                Ok::<_, ()>(vec![found(text.trim_matches(['[', ']']), true)])
            },
        );
        assert_eq!(outcome, Ok(Scrubbed::Clean { redactions: 1 }));
    }
}
