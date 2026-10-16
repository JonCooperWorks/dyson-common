//! Where a secret is in a text and how it is shown: lines and columns, the
//! name it is assigned to, its line clipped for display, and a mask.

use std::collections::HashMap;

use crate::hit::ContextLine;

/// Lines either side of a finding kept as its context.
pub(crate) const CONTEXT_LINES: usize = 3;
/// Characters of a context line kept.
pub const LINE_CAP: usize = 200;
/// A run of base64 characters this long shows only its ends.
const RUN_CAP: usize = 64;
/// A secret longer than this shows only its ends in its snippet.
const SECRET_CAP: usize = 120;

/// Shannon entropy, in bits a character.
pub fn entropy(value: &str) -> f64 {
    let mut counts: HashMap<char, usize> = HashMap::new();
    let mut total = 0usize;
    for c in value.chars() {
        *counts.entry(c).or_default() += 1;
        total += 1;
    }
    if total == 0 {
        return 0.0;
    }
    counts
        .values()
        .map(|&n| {
            let p = n as f64 / total as f64;
            -p * p.log2()
        })
        .sum()
}

/// JSON string escapes (`\n`, `\"`, `\u…`) decoded, when there are any
/// and they decode.
pub fn unescape(text: &str) -> String {
    if !text.contains('\\') {
        return text.to_owned();
    }
    serde_json::from_str::<String>(&format!("\"{text}\""))
        .map(|decoded| decoded.trim().to_owned())
        .unwrap_or_else(|_| text.to_owned())
}

/// A rule's name for a table cell: its own when it is a name, else
/// (Betterleaks names its rules with a sentence) the rule id's last part,
/// `betterleaks.github-pat` read as `github pat`.
pub fn short_name(id: &str, name: &str) -> String {
    if name.len() <= 40 {
        return name.to_owned();
    }
    id.rsplit(['.', '/'])
        .next()
        .unwrap_or(id)
        .replace(['-', '_'], " ")
}

/// Whether two detectors' values are one secret read to different ends:
/// one begins with the other, or holds it and is no more than twice as
/// long. (A password that is a word found inside a private key is not the
/// key.)
pub fn same_secret(a: &str, b: &str) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    !short.is_empty()
        && (long.starts_with(short) || (long.contains(short) && short.len() * 2 >= long.len()))
}

/// The line (from 1) and column (in characters, from 1) of a byte offset.
pub fn line_col(text: &str, at: usize) -> (usize, usize) {
    let at = floor_boundary(text, at);
    let before = &text[..at];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (
        before.matches('\n').count() + 1,
        before[line_start..].chars().count() + 1,
    )
}

pub(crate) fn floor_boundary(text: &str, mut at: usize) -> usize {
    at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// The name a secret is assigned to, from the text before it on its line:
/// `DASHBOARD_ADMIN_KEY=`, `export X=`, `api_key: `, `"x": "`, `{\"x\": \"`,
/// `headers["X-Api-Key"] = "`. None when nothing is assigned (`Bearer `),
/// what precedes is a URL's user (`https://user:`), or a colon ends a phrase
/// (`the token for the API: `).
pub fn assigned_name(before: &str) -> Option<String> {
    let quote = |c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '\\');
    let rest = before.trim_end_matches(quote);
    let (op, rest) = ["=>", ":=", "?=", "+=", "=", ":", "->"]
        .iter()
        .find_map(|op| rest.strip_suffix(op).map(|rest| (*op, rest)))?;
    let rest = rest.trim_end_matches(|c: char| quote(c) || c == ']');
    let taken: Vec<char> = rest
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
        .collect();
    let preceding = rest.chars().rev().nth(taken.len());
    if matches!(preceding, Some('/' | '@')) {
        return None;
    }
    let head: String = rest.chars().rev().skip(taken.len()).collect();
    if op == ":"
        && head.starts_with(char::is_whitespace)
        && head.trim_start().starts_with(char::is_alphabetic)
    {
        return None;
    }
    let name: String = taken.into_iter().rev().collect();
    let name = name.trim_matches(['.', '-']);
    (name.chars().any(char::is_alphabetic)).then(|| name.to_owned())
}

fn base64ish(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-')
}

/// One line for display: long base64 runs shortened to their ends, a long
/// secret (`focus`, in characters) to its ends, and the whole cut to
/// [`LINE_CAP`] characters around the secret.
pub fn clip(line: &str, focus: Option<(usize, usize)>) -> String {
    let chars: Vec<char> = line.chars().collect();
    let in_focus = |k: usize| focus.is_some_and(|(a, b)| k >= a && k < b);
    let ends = |run: &[char], head: usize, tail: usize, out: &mut Vec<char>| {
        out.extend(&run[..head]);
        out.extend(format!("…[{} chars]…", run.len()).chars());
        out.extend(&run[run.len() - tail..]);
    };
    let mut out: Vec<char> = Vec::with_capacity(chars.len().min(LINE_CAP * 2));
    let mut focus_out = None;
    let mut i = 0;
    while i < chars.len() {
        if in_focus(i) {
            let b = focus.map_or(i, |(_, b)| b.min(chars.len()));
            let begin = out.len();
            let secret = &chars[i..b];
            if secret.len() > SECRET_CAP {
                ends(secret, 48, 24, &mut out);
            } else {
                out.extend(secret);
            }
            focus_out = Some((begin, out.len()));
            i = b;
            continue;
        }
        let run = chars[i..]
            .iter()
            .enumerate()
            .take_while(|&(k, c)| base64ish(*c) && !in_focus(i + k))
            .count();
        if run >= RUN_CAP {
            ends(&chars[i..i + run], 12, 6, &mut out);
            i += run;
            continue;
        }
        out.extend(&chars[i..i + run.max(1)]);
        i += run.max(1);
    }
    if out.len() <= LINE_CAP {
        return out.into_iter().collect();
    }
    let (a, b) = focus_out.unwrap_or((0, 0));
    let from = if b <= LINE_CAP {
        0
    } else {
        a.saturating_sub(40).min(out.len() - LINE_CAP)
    };
    let to = (from + LINE_CAP).min(out.len());
    format!(
        "{}{}{}",
        if from > 0 { "…" } else { "" },
        out[from..to].iter().collect::<String>(),
        if to < out.len() { "…" } else { "" }
    )
}

/// The secret's line, clipped, and up to [`CONTEXT_LINES`] lines either
/// side of it (a secret over several lines, a PEM key, shows its first few).
/// Lines are numbered from `first_line`.
pub fn surroundings(
    text: &str,
    start: usize,
    end: usize,
    first_line: usize,
) -> (String, Vec<ContextLine>) {
    let lines: Vec<&str> = text.split('\n').collect();
    let (line, column) = line_col(text, start);
    let (end_line, _) = line_col(text, end.saturating_sub(1).max(start));
    let own = lines.get(line - 1).copied().unwrap_or_default();
    let length = text[floor_boundary(text, start)..floor_boundary(text, end)]
        .chars()
        .count();
    let focus = Some((column - 1, column - 1 + length));
    let snippet = clip(own.trim_end_matches('\r'), focus);
    let from = line.saturating_sub(CONTEXT_LINES).max(1);
    let to = (end_line.min(line + 5) + CONTEXT_LINES).min(lines.len());
    let context = (from..=to)
        .map(|n| ContextLine {
            line: n + first_line - 1,
            text: if n == line {
                snippet.clone()
            } else {
                clip(lines[n - 1].trim_end_matches('\r'), None)
            },
        })
        .collect();
    (snippet, context)
}

/// The first and last four characters, never the middle. A secret shorter
/// than 20 characters shows only its first four.
pub fn mask(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    match chars.len() {
        0..12 => "…".to_owned(),
        12..20 => format!("{}…", chars[..4].iter().collect::<String>()),
        n => format!(
            "{}…{}",
            chars[..4].iter().collect::<String>(),
            chars[n - 4..].iter().collect::<String>()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shifted(skip: usize, n: usize) -> String {
        let alphabet = "Q7wE9rT2yU4iO6pA8sD1fG3hJ5kL0zXcVbNm";
        alphabet.chars().cycle().skip(skip).take(n).collect()
    }

    #[test]
    fn one_secret_read_to_different_ends_is_one_secret() {
        let key = format!("sk-{}", shifted(3, 48));
        assert!(same_secret(&key, &format!("{key}\\")));
        assert!(same_secret(&format!("Bearer {key}"), &key));
        assert!(same_secret(&key, &key));
        // A word inside something far longer is not that thing.
        assert!(!same_secret(
            "password",
            &format!("-----BEGIN {} password", shifted(3, 200))
        ));
        let other: String = key.chars().rev().collect();
        assert!(!same_secret(&key, &other));
    }

    #[test]
    fn masks_all_but_the_ends() {
        assert_eq!(mask("sk-or-v1-abcdefghijklmnop"), "sk-o…mnop");
        assert_eq!(mask("abcdefghijklmn"), "abcd…");
        assert_eq!(mask("short"), "…");
    }

    #[test]
    fn names_the_variable_a_secret_is_assigned_to() {
        for (before, name) in [
            ("DASHBOARD_ADMIN_KEY=", Some("DASHBOARD_ADMIN_KEY")),
            ("  21: DASHBOARD_ADMIN_KEY=", Some("DASHBOARD_ADMIN_KEY")),
            ("export GH_TOKEN=", Some("GH_TOKEN")),
            ("  api_key: ", Some("api_key")),
            ("{\"openai_key\": \"", Some("openai_key")),
            ("{\\\"x\\\": \\\"", Some("x")),
            ("headers[\"X-Api-Key\"] = \"", Some("X-Api-Key")),
            ("password := '", Some("password")),
            ("--api-key=", Some("api-key")),
            ("Authorization: Bearer ", None),
            ("Use this token for the API: ", None),
            ("  21: api_key: ", Some("api_key")),
            ("https://deploy:", None),
            ("see ", None),
            ("", None),
        ] {
            assert_eq!(assigned_name(before).as_deref(), name, "{before}");
        }
    }

    #[test]
    fn clips_long_runs_and_long_secrets_to_their_ends() {
        let secret = shifted(1, 300);
        let clipped = clip(&format!("k={secret}"), Some((2, 302)));
        assert!(
            clipped.contains("…[300 chars]…") && clipped.len() < 120,
            "{clipped}"
        );
        assert_eq!(clip("short line", None), "short line");
        let blob = "QUJD".repeat(100);
        assert!(clip(&format!("blob = {blob}"), None).contains("…[400 chars]…"));
    }

    #[test]
    fn columns_count_characters() {
        assert_eq!(line_col("é → x", "é → ".len()), (1, 5));
        assert_eq!(line_col("a\nbc", 3), (2, 2));
    }

    #[test]
    fn entropy_is_bits_a_character() {
        assert_eq!(entropy(""), 0.0);
        assert_eq!(entropy("aaaa"), 0.0);
        assert!((entropy("abcd") - 2.0).abs() < 1e-9);
    }
}
