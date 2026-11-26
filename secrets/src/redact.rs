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
        // Escapes may be written with lowercase hex (`%2f`) as well.
        let lower = lowercase_escapes(&encoded);
        if lower != encoded {
            out.push(lower);
        }
        out.push(encoded);
    }
    in_base64(value, &mut out);
    out
}

/// `encoded` with the hex digits of each `%XX` escape in lowercase; the
/// characters percent-encoding left alone keep their case.
fn lowercase_escapes(encoded: &str) -> String {
    let mut out = String::with_capacity(encoded.len());
    let mut hex = 0;
    for c in encoded.chars() {
        if hex > 0 {
            out.push(c.to_ascii_lowercase());
            hex -= 1;
        } else {
            if c == '%' {
                hex = 2;
            }
            out.push(c);
        }
    }
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

    /// Every string of a document, and every object key, redacted in
    /// place. How many were found. A key is taken out too: a header map or
    /// an environment written as an object may hold a secret as a name.
    pub fn document(&self, document: &mut Value) -> usize {
        let mut count = 0;
        strings_and_keys(document, &mut |text| self.change(text, &mut count));
        count
    }
}

/// Apply `change` to every string value of a document and to every object
/// key. A key renamed to one its object already has (two forms of one
/// secret, or a secret beside its own token) gets a `#n` suffix, so no
/// value is lost. The order of keys is kept.
pub fn strings_and_keys(value: &mut Value, change: &mut impl FnMut(&str) -> Option<String>) {
    match value {
        Value::String(text) => {
            if let Some(changed) = change(text) {
                *text = changed;
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| strings_and_keys(item, change)),
        Value::Object(fields) => {
            fields
                .values_mut()
                .for_each(|field| strings_and_keys(field, change));
            let renames: std::collections::HashMap<String, String> = fields
                .keys()
                .filter_map(|key| change(key).map(|renamed| (key.clone(), renamed)))
                .collect();
            if renames.is_empty() {
                return;
            }
            let old = std::mem::take(fields);
            let kept: HashSet<String> = old
                .keys()
                .filter(|key| !renames.contains_key(*key))
                .cloned()
                .collect();
            for (key, field) in old {
                let name = match renames.get(&key) {
                    None => key,
                    Some(renamed) => {
                        let taken = |name: &str| kept.contains(name) || fields.contains_key(name);
                        let mut name = renamed.clone();
                        let mut n = 2;
                        while taken(&name) {
                            name = format!("{renamed}#{n}");
                            n += 1;
                        }
                        name
                    }
                };
                fields.insert(name, field);
            }
        }
        _ => {}
    }
}

/// Apply `change` to every string value of a document. Keys are left; see
/// [`strings_and_keys`] for a walk that changes them too.
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

/// Every redaction token a document holds, in its strings or its keys.
pub fn tokens_in(document: &Value) -> HashSet<String> {
    let mut found = HashSet::new();
    each_text(document, &mut |text| {
        found.extend(TOKEN.find_iter(text).map(|token| token.as_str().to_owned()));
    });
    found
}

/// What may stand around a token in a value a detector read it as: an
/// assignment's name before it (`KEY=`, `"api_key": "`), and quotes,
/// brackets, separators and space anywhere.
static GLUE_BEFORE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r#"^[\s"'`\\(\[{<,;]*(?:[A-Za-z_][A-Za-z0-9_.-]{0,63}["'`]?\s*[:=]\s*)?[\s"'`\\(\[{<,;]*$"#,
    )
    .expect("glue before")
});
static GLUE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"^[\s"'`\\()\[\]{}<>,;:=]*$"#).expect("glue"));

/// Whether `value` is made of whole tokens with nothing between or around
/// them but [glue](GLUE_BEFORE). `starts_outside` says whether the text
/// before the first token is the value's own (and so may be a name).
fn tokens_and_glue(value: &str, starts_outside: bool) -> bool {
    let mut last = 0;
    let mut any = false;
    for token in TOKEN.find_iter(value) {
        let between = &value[last..token.start()];
        let glue = if last == 0 && starts_outside {
            GLUE_BEFORE.is_match(between)
        } else {
            GLUE.is_match(between)
        };
        if !glue {
            return false;
        }
        last = token.end();
        any = true;
    }
    any && GLUE.is_match(&value[last..])
}

/// Whether what a detector found is a redaction token, or part of one, or
/// one with only an assignment's name, quotes or separators around it:
/// nothing of a secret. `present` is what [`tokens_in`] found in the
/// document scanned.
///
/// Without the document this can only judge the value itself: a piece of
/// a token counts only when it crosses one of the token's own `:`s (a rule
/// name and its digest, or `REDACTED` and the rule), since a piece that
/// does not may as well be a secret that happens to be spelled like part
/// of a rule name. [`scrub`] judges by where the value is in the document
/// instead (see [`is_token_in`]), and uses this only for a value that is
/// in none of its strings verbatim.
pub fn is_token(present: &HashSet<String>, value: &str) -> bool {
    tokens_and_glue(value, true)
        || (value.contains(':') && present.iter().any(|token| token.contains(value)))
}

/// Whether every place `value` is written in `document` is inside a
/// redaction token, or is a token with only [glue](GLUE_BEFORE) around it.
/// `None` when it is written nowhere (a value a detector stitched together
/// or decoded): see [`is_token`] for that.
pub fn is_token_in(document: &Value, value: &str) -> Option<bool> {
    if value.is_empty() {
        return None;
    }
    let mut seen = false;
    let mut all = true;
    each_text(document, &mut |text| {
        if !all || !text.contains(value) {
            return;
        }
        let spans: Vec<(usize, usize)> = TOKEN
            .find_iter(text)
            .map(|token| (token.start(), token.end()))
            .collect();
        // Every occurrence, overlapping ones too.
        let mut from = 0;
        while let Some(at) = text.get(from..).and_then(|rest| rest.find(value)) {
            let (start, end) = (from + at, from + at + value.len());
            seen = true;
            if !inside_tokens(text, &spans, start, end) {
                all = false;
                return;
            }
            from = start + text[start..].chars().next().map_or(1, char::len_utf8);
        }
    });
    seen.then_some(all)
}

/// Every string value and object key of a document, read in place
/// (what [`Redactor::document`] redacts).
pub fn each_text(value: &Value, read: &mut impl FnMut(&str)) {
    match value {
        Value::String(text) => read(text),
        Value::Array(items) => items.iter().for_each(|item| each_text(item, read)),
        Value::Object(fields) => fields.iter().for_each(|(key, field)| {
            read(key);
            each_text(field, read);
        }),
        _ => {}
    }
}

/// Whether `text[start..end]` is inside one token, or is whole tokens with
/// only glue between and around them (a piece of a token at either end
/// counts as part of it).
fn inside_tokens(text: &str, spans: &[(usize, usize)], start: usize, end: usize) -> bool {
    if spans.iter().any(|&(s, e)| s <= start && end <= e) {
        return true;
    }
    // Widen the occurrence to the whole of any token it cuts into, then
    // judge what is left around the tokens.
    let (mut from, mut to) = (start, end);
    for &(s, e) in spans {
        if s < start && start < e {
            from = s;
        }
        if s < end && end < e {
            to = e;
        }
    }
    tokens_and_glue(&text[from..to], from == start)
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
            .filter(|found| {
                let value = found.value.trim();
                !is_token_in(document, value).unwrap_or_else(|| is_token(&present, value))
            })
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
        assert!(is_token(
            &present,
            "\"api_key\": \"[REDACTED:generic-api-key:3fa9c1d2]\","
        ));
        assert_eq!(label_token("endpoint"), "[REDACTED:endpoint]");
        assert!(TOKEN.is_match(&label_token("endpoint")));
        assert!(TOKEN.is_match(&token("x", &digest(KEY, "y"))));
    }

    #[test]
    fn a_secret_beside_or_spelled_like_part_of_a_token_is_not_a_token() {
        let present: HashSet<String> = ["[REDACTED:generic-api-key:3fa9c1d2]".to_owned()].into();
        // Spelled like part of the rule name.
        assert!(!is_token(&present, "generic-api"));
        assert!(!is_token(&present, "api-key"));
        // A secret with a token beside it is still a secret.
        assert!(!is_token(
            &present,
            "sk-live-9f8e7d6c5b4a39281706[REDACTED:generic-api-key:3fa9c1d2]"
        ));
        assert!(!is_token(
            &present,
            "[REDACTED:generic-api-key:3fa9c1d2] sk-live-9f8e7d6c5b4a39281706"
        ));
        assert!(!is_token(
            &present,
            "[REDACTED:generic-api-key:3fa9c1d2]sk-live-9f8e7d6c5b4a"
        ));
    }

    #[test]
    fn a_rescan_finding_spelled_like_part_of_a_token_is_still_taken_out() {
        // The secret is also a piece of a token's rule name: the old rule
        // dropped it from the rescan because the token "contains" it.
        let mut document = json!({
            "m": "[REDACTED:generic-api-key:3fa9c1d2] and the password is generic-api"
        });
        let outcome = scrub(
            &mut document,
            &Redactor::default(),
            3,
            finds(&["generic-api"]),
        );
        assert!(
            matches!(outcome, Ok(Scrubbed::Clean { redactions }) if redactions >= 1),
            "{outcome:?}"
        );
        let text = document.to_string();
        assert!(!text.contains("password is generic-api"), "{text}");

        // And one with a token glued to it is taken out whole.
        const GLUED: &str = concat!(
            "sk-live-9f8e7d6c5b4a39281706",
            "[REDACTED:generic-api-key:3fa9c1d2]"
        );
        let secret = "sk-live-9f8e7d6c5b4a39281706";
        let mut document = json!({ "m": GLUED });
        let outcome = scrub(&mut document, &Redactor::default(), 3, finds(&[GLUED]));
        assert!(
            matches!(outcome, Ok(Scrubbed::Clean { redactions: 1 })),
            "{outcome:?}"
        );
        assert!(!document.to_string().contains(secret), "{document}");
    }

    #[test]
    fn a_rescan_finding_inside_a_token_or_named_by_one_is_left() {
        // A detector reading a token's digest, its rule and digest, or an
        // assignment of it: all inside tokens, nothing taken out.
        for value in [
            "3fa9c1d2",
            "generic-api-key:3fa9c1d2",
            "REDACTED:generic-api-key",
            "api_key=[REDACTED:generic-api-key:3fa9c1d2]",
            "api_key=[REDACTED:generic-api-key",
        ] {
            let mut document = json!({
                "m": "export api_key=[REDACTED:generic-api-key:3fa9c1d2]; done"
            });
            let before = document.clone();
            let outcome = scrub(&mut document, &Redactor::default(), 3, |_: &Value| {
                Ok::<_, ()>(vec![found(value, true)])
            });
            assert_eq!(outcome, Ok(Scrubbed::Clean { redactions: 0 }), "{value}");
            assert_eq!(document, before, "{value}");
        }
        // Written both inside a token and outside one: outside wins.
        let document = json!({"a": "[REDACTED:generic-api-key:3fa9c1d2]", "b": "3fa9c1d2"});
        assert_eq!(is_token_in(&document, "3fa9c1d2"), Some(false));
        assert_eq!(is_token_in(&document, "nowhere"), None);
    }

    /// SEC-21: a secret used as an object key leaked through `document`.
    #[test]
    fn a_secret_written_as_a_key_is_taken_out() {
        let secret = "sk-live-9f8e7d6c5b4a39281706f5e4d3c2b1a0";
        let redactor = redactor(&[secret], &[]);
        let mut document = json!({
            "headers": { secret: "value", "plain": secret },
            "list": [{ format!("Bearer {secret}"): 1 }],
        });
        let count = redactor.document(&mut document);
        let text = document.to_string();
        assert!(!text.contains(secret), "{text}");
        assert_eq!(count, 3, "{text}");
        // The values under renamed keys are kept.
        let headers = document["headers"].as_object().unwrap();
        assert_eq!(headers.len(), 2, "{text}");
        assert!(headers.values().any(|value| value == "value"), "{text}");
    }

    /// Two keys that redact to the same token keep both values.
    #[test]
    fn keys_that_redact_alike_are_both_kept() {
        let secret = "sk-live-9f8e7d6c5b4a39281706f5e4d3c2b1a0";
        let redactor = redactor(&[secret], &[]);
        let token = token("test.rule", &digest(KEY, secret));
        let mut document = json!({ secret: 1, token.clone(): 2 });
        redactor.document(&mut document);
        let fields = document.as_object().unwrap();
        assert_eq!(fields.len(), 2, "{document}");
        assert!(!document.to_string().contains(secret), "{document}");
    }

    /// SEC-21: percent-encoding may use lowercase hex digits.
    #[test]
    fn lowercase_percent_encoding_is_found() {
        let secret = "abc/def+ghi=jkl/mno";
        let redactor = redactor(&[secret], &[]);
        let lower = "abc%2fdef%2bghi%3djkl%2fmno";
        let mut document = json!({ "url": format!("https://x.test/?k={lower}") });
        assert_eq!(redactor.document(&mut document), 1, "{document}");
        assert!(!document.to_string().contains("abc%2fdef"), "{document}");
    }

    /// A token in a key counts as present, and a rescan finding in a key
    /// is judged by where it is written.
    #[test]
    fn keys_are_read_for_tokens() {
        let document = json!({ "[REDACTED:generic-api-key:3fa9c1d2]": "x" });
        assert!(tokens_in(&document).contains("[REDACTED:generic-api-key:3fa9c1d2]"));
        assert_eq!(is_token_in(&document, "3fa9c1d2"), Some(true));
        let document = json!({ "pw-hunter22": "x" });
        assert_eq!(is_token_in(&document, "hunter22"), Some(false));
    }

    /// A rescan that finds a secret in a key takes it out rather than
    /// holding the document back as "not in it verbatim".
    #[test]
    fn a_rescan_finding_in_a_key_is_taken_out() {
        let mut document = json!({ "pw=hunter22": "x" });
        let outcome = scrub(&mut document, &Redactor::default(), 3, finds(&["hunter22"]));
        assert_eq!(outcome, Ok(Scrubbed::Clean { redactions: 1 }));
        assert!(!document.to_string().contains("hunter22"), "{document}");
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
