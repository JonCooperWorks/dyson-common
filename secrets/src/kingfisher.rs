//! Kingfisher (MongoDB's secret scanner) embedded as a library, with its
//! built-in rules (provider formats from Betterleaks and Veles, generic
//! key/secret/password assignments with entropy thresholds, private keys,
//! JWTs, base64-wrapped secrets) and Betterleaks' generic API-key and
//! password rules beside them.
//!
//! Only Kingfisher's default features are built, which carry no
//! validation: no found key is ever presented to its issuer.
//!
//! Compiling the rules takes seconds, so build a [`Scanner`] once
//! ([`Scanner::shared`]) and scan many texts with it.

use std::collections::HashSet;
use std::sync::{Arc, LazyLock};

use kingfisher_rules::{Confidence, Rules, RulesDatabase, get_builtin_rules};
use kingfisher_scanner::ScannerConfig;

use crate::Error;
use crate::hit::{Dependency, Detection, Extra, Hit, KingfisherLocation};
use crate::text::{
    assigned_name, entropy, floor_boundary, line_col, same_secret, short_name, surroundings,
    unescape,
};

/// Named captures that name the variable a secret is assigned to.
const VARIABLE_CAPTURES: [&str; 4] = ["variable", "var", "key_name", "keyname"];

/// Betterleaks' generic rules, which Kingfisher's catalog omits and which
/// catch a secret known only by the name it is assigned to
/// (`DASHBOARD_ADMIN_KEY=…`), gated on entropy.
const GENERIC_RULES: &str = include_str!("../rules/betterleaks-generic.toml");

/// A compiled set of rules.
pub struct Scanner {
    inner: kingfisher_scanner::Scanner,
}

static SHARED: LazyLock<Result<Scanner, Error>> = LazyLock::new(Scanner::new);

/// One text to scan, and how to report what is in it.
#[derive(Debug, Clone, Copy)]
pub struct At<'a> {
    /// What is scanned. Its first `prefix` bytes were added to give a bare
    /// value a name (`field=`): locations and context are of the text after
    /// them.
    pub text: &'a str,
    pub prefix: usize,
    /// The path Kingfisher is told the text came from, which Betterleaks'
    /// path filters read (a lock file is skipped): a file's path for code,
    /// or a label such as `message/user`.
    pub path: &'a str,
    /// The line the text (after `prefix`) starts on, for a file read from
    /// partway through.
    pub first_line: usize,
    /// Look for card numbers and IBANs too (with the `payment` feature).
    pub payment: bool,
}

impl<'a> At<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            prefix: 0,
            path: "",
            first_line: 1,
            payment: true,
        }
    }

    pub fn path(self, path: &'a str) -> Self {
        Self { path, ..self }
    }

    pub fn prefix(self, prefix: usize) -> Self {
        Self { prefix, ..self }
    }

    pub fn first_line(self, first_line: usize) -> Self {
        Self { first_line, ..self }
    }

    pub fn payment(self, payment: bool) -> Self {
        Self { payment, ..self }
    }
}

impl Scanner {
    /// Kingfisher's built-in rules, all of them (the generic assignment and
    /// entropy rules are its low-confidence tier), and Betterleaks' generic
    /// rules. Kingfisher's own blob dedup is off: a shared scanner that
    /// skipped every text seen before would miss a secret sent twice.
    pub fn new() -> Result<Self, Error> {
        let mut rules = get_builtin_rules(Some(Confidence::Low))
            .map_err(|e| Error(format!("load Kingfisher rules: {e:#}")))?;
        rules.update(generic_rules()?);
        let db = RulesDatabase::from_rule_collection(rules)
            .map_err(|e| Error(format!("compile Kingfisher rules: {e:#}")))?;
        Ok(Self {
            inner: kingfisher_scanner::Scanner::with_config(
                Arc::new(db),
                ScannerConfig {
                    enable_dedup: false,
                    redact_secrets: false,
                    ..ScannerConfig::default()
                },
            ),
        })
    }

    /// One scanner for the process, compiled on first use.
    pub fn shared() -> Result<&'static Self, Error> {
        SHARED.as_ref().map_err(Clone::clone)
    }

    /// Every secret in `text`, in order, each once: where two rules match
    /// one secret, a named rule is kept over a generic one.
    pub fn scan(&self, text: &str) -> Result<Vec<Hit>, Error> {
        Ok(self
            .scan_at(At::new(text), &[])?
            .into_iter()
            .filter(|h| h.visible)
            .collect())
    }

    /// [`Scanner::scan`] with a path, a prefix and a first line (see
    /// [`At`]), and what other detectors found in the same text (see
    /// [`crate::external`]) merged in where Kingfisher has not the same
    /// secret already. Hidden component matches are kept, for
    /// [`crate::pair`]; drop those not `visible` before listing.
    ///
    /// An error is a text Kingfisher could not read, which is not a text
    /// with no secrets.
    pub fn scan_at(&self, at: At<'_>, extras: &[Extra]) -> Result<Vec<Hit>, Error> {
        let At {
            text: scanned,
            prefix,
            path,
            first_line,
            payment,
        } = at;
        let text = scanned.get(prefix..).unwrap_or(scanned);
        let mut seen = HashSet::new();
        let mut hits: Vec<Hit> = Vec::new();
        // Card numbers and IBANs first: a generic rule that reads one as a
        // key is the same value under a worse name.
        #[cfg(feature = "payment")]
        if payment {
            hits.extend(
                crate::payment::find(text)
                    .into_iter()
                    .filter(|found| seen.insert((found.value.clone(), true)))
                    .map(|found| payment_hit(text, path, first_line, found)),
            );
        }
        #[cfg(not(feature = "payment"))]
        let _ = payment;
        let blob = kingfisher_core::Blob::from_bytes(scanned.as_bytes().to_vec());
        let mut found = self
            .inner
            .scan_blob_at_path(&blob, path)
            .map_err(|error| Error(format!("kingfisher could not scan {path}: {error:#}")))?;
        found.sort_by_key(|f| (f.location.start_offset, f.rule_id.contains("generic")));
        hits.extend(found.into_iter().filter_map(|f| {
            // A secret inside JSON-escaped text (a PEM key in tool
            // arguments) reads the same as its decoded self.
            let value = unescape(f.secret.trim());
            let visible = f.rule.visible();
            if value.is_empty() || !seen.insert((value.clone(), visible)) {
                return None;
            }
            Some(hit(text, path, prefix, first_line, f, value))
        }));
        for extra in extras {
            let placed = extra.end > extra.start;
            if placed && extra.start < prefix {
                continue;
            }
            let value = unescape(&extra.value);
            let held = hits
                .iter()
                .any(|h: &Hit| h.visible && same_secret(&h.value, &value));
            if !held && seen.insert((value.clone(), true)) {
                let mut hit = extra_hit(text, path, prefix, first_line, extra);
                hit.value = value;
                hits.push(hit);
            }
        }
        Ok(hits)
    }
}

impl Scanner {
    /// Every secret in the strings of a JSON document, as
    /// [`crate::scrub`] takes them: each replaced with a token naming its
    /// rule and its digest under `key` (see [`crate::redact::digest`]).
    /// Card numbers and IBANs are included with the `payment` feature.
    /// `keep` is asked of each value found; one it says is not a secret (a
    /// false positive the host has marked) is left.
    ///
    /// ```ignore
    /// let scanner = Scanner::shared()?;
    /// let outcome = dyson_secrets::scrub(&mut document, &known, 3, |d| {
    ///     scanner.leftovers(d, key, |_| true)
    /// })?;
    /// ```
    pub fn leftovers(
        &self,
        document: &serde_json::Value,
        key: &[u8],
        keep: impl Fn(&Hit) -> bool,
    ) -> Result<Vec<crate::Leftover>, Error> {
        let mut texts = Vec::new();
        crate::redact::strings(&mut document.clone(), &mut |text| {
            texts.push(text.to_owned());
            None
        });
        let mut out: Vec<crate::Leftover> = Vec::new();
        for text in &texts {
            for hit in self.scan(text)? {
                if !keep(&hit) || out.iter().any(|left| left.value == hit.value) {
                    continue;
                }
                let digest = crate::redact::digest(key, hit.value.trim());
                out.push(crate::Leftover {
                    token: crate::redact::token(&hit.rule_id, &digest),
                    rule_id: hit.rule_id,
                    value: hit.value,
                    shown: true,
                });
            }
        }
        Ok(out)
    }
}

/// Kingfisher's importer reads Betterleaks rules from a file, so the
/// embedded copy is written out once.
fn generic_rules() -> Result<Rules, Error> {
    let path =
        std::env::temp_dir().join(format!("dyson-secrets-rules-{}.toml", std::process::id()));
    std::fs::write(&path, GENERIC_RULES)
        .map_err(|e| Error(format!("write the generic secret rules: {e}")))?;
    let rules = Rules::from_toml_file(&path, Confidence::Low);
    let _ = std::fs::remove_file(&path);
    rules.map_err(|e| Error(format!("load the generic secret rules: {e:#}")))
}

/// A card number or IBAN as a hit.
#[cfg(feature = "payment")]
fn payment_hit(text: &str, path: &str, first_line: usize, found: crate::payment::Found) -> Hit {
    let (line, column) = line_col(text, found.start);
    let (end_line, end_column) = line_col(text, found.end.saturating_sub(1).max(found.start));
    let (snippet, context) = surroundings(text, found.start, found.end, first_line);
    let line_start = text[..found.start].rfind('\n').map_or(0, |i| i + 1);
    Hit {
        rule_id: found.rule_id,
        rule_name: found.label.clone(),
        label: found.label,
        severity: found.severity.to_owned(),
        start: found.start,
        end: found.end,
        entropy: 0.0,
        line: line + first_line - 1,
        column,
        end_line: end_line + first_line - 1,
        end_column,
        variable: assigned_name(&text[line_start..found.start]),
        snippet,
        context,
        detection: Detection {
            scanned_as: path.to_owned(),
            issuer: found.issuer,
            ..Detection::default()
        },
        visible: true,
        value: found.value,
    }
}

/// A secret another detector found, as a hit.
fn extra_hit(text: &str, path: &str, prefix: usize, first_line: usize, extra: &Extra) -> Hit {
    let start = floor_boundary(text, extra.start.saturating_sub(prefix));
    let end = floor_boundary(text, extra.end.saturating_sub(prefix)).max(start);
    let (line, column) = line_col(text, start);
    let (end_line, end_column) = line_col(text, end.saturating_sub(1).max(start));
    let (snippet, context) = surroundings(text, start, end, first_line);
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    Hit {
        rule_id: extra.rule_id.clone(),
        label: short_name(&extra.rule_id, &extra.label),
        rule_name: extra.label.clone(),
        severity: extra.severity.to_owned(),
        start,
        end,
        value: extra.value.clone(),
        entropy: entropy(&extra.value),
        line: line + first_line - 1,
        column,
        end_line: end_line + first_line - 1,
        end_column,
        variable: (end > start)
            .then(|| assigned_name(&text[line_start..start]))
            .flatten(),
        snippet,
        context,
        detection: Detection {
            scanned_as: path.to_owned(),
            ..Detection::default()
        },
        visible: true,
    }
}

/// The name of an enum variant from its debug form: `Http(…)` is `Http`.
fn variant(debug: String) -> String {
    debug
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn hit(
    text: &str,
    path: &str,
    prefix: usize,
    first_line: usize,
    f: kingfisher_scanner::Finding,
    value: String,
) -> Hit {
    let start = f
        .location
        .start_offset
        .saturating_sub(prefix)
        .min(text.len());
    let end = f
        .location
        .end_offset
        .saturating_sub(prefix)
        .clamp(start, text.len());
    let (line, column) = line_col(text, start);
    let (end_line, end_column) = line_col(text, end.saturating_sub(1).max(start));
    let (snippet, context) = surroundings(text, start, end, first_line);
    let rule = f.rule.syntax();
    let variable = f
        .captures
        .iter()
        .find(|(name, v)| {
            VARIABLE_CAPTURES.contains(&name.to_ascii_lowercase().as_str()) && !v.is_empty()
        })
        .map(|(_, v)| v.clone())
        .or_else(|| {
            let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
            assigned_name(&text[line_start..start])
        });
    Hit {
        label: short_name(&f.rule_id, &f.rule_name),
        rule_name: f.rule_name.clone(),
        rule_id: f.rule_id.clone(),
        severity: format!("{:?}", f.confidence).to_ascii_lowercase(),
        start,
        end,
        value,
        entropy: f64::from(f.entropy),
        line: line + first_line - 1,
        column,
        end_line: end_line + first_line - 1,
        end_column,
        variable,
        snippet,
        context,
        visible: f.rule.visible(),
        detection: Detection {
            fingerprint: f.fingerprint.to_string(),
            scanned_as: path.to_owned(),
            base64: f.is_base64_encoded,
            captures: f.captures.into_iter().collect(),
            issuer: None,
            location: KingfisherLocation {
                line: f.location.line,
                column: f.location.column,
                end_line: f.location.end_line,
                end_column: f.location.end_column,
                start_offset: f.location.start_offset,
                end_offset: f.location.end_offset,
            },
            min_entropy: (f64::from(rule.min_entropy) * 100.0).round() / 100.0,
            references: rule.references.clone(),
            validation: rule.validation.as_ref().map(|v| variant(format!("{v:?}"))),
            revocation: rule.revocation.as_ref().map(|v| variant(format!("{v:?}"))),
            depends_on: rule
                .depends_on_rule
                .iter()
                .flatten()
                .map(|d| Dependency {
                    rule_id: d.rule_id.clone(),
                    variable: d.variable.clone(),
                    optional: d.optional,
                    within: d.within.clone(),
                })
                .collect(),
            paired: Vec::new(),
            pattern_requirements: rule
                .pattern_requirements
                .as_ref()
                .and_then(|p| serde_json::to_value(p).ok()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{LINE_CAP, clip};
    use serde_json::json;

    fn scanner() -> &'static Scanner {
        Scanner::shared().expect("the rules compile")
    }

    fn scan(text: &str) -> Vec<Hit> {
        scanner().scan(text).unwrap()
    }

    fn rule_ids(text: &str) -> Vec<String> {
        scan(text).into_iter().map(|h| h.rule_id).collect()
    }

    // Test secrets are assembled at run time so the source holds no string a
    // repository scanner would flag.
    fn hex64() -> String {
        "0123456789abcdef".repeat(4)
    }
    fn mixed(n: usize) -> String {
        shifted(3, n)
    }
    fn shifted(skip: usize, n: usize) -> String {
        let alphabet = "Q7wE9rT2yU4iO6pA8sD1fG3hJ5kL0zXcVbNm";
        alphabet.chars().cycle().skip(skip).take(n).collect()
    }
    /// A GitHub token, a different one for each `k`.
    fn token(k: usize) -> String {
        format!("ghp_{}", shifted(k, 36))
    }

    #[test]
    fn finds_named_and_generic_secrets() {
        // A key known only by the name it is assigned to, as a .env file
        // read by a tool shows it: the generic rule, on entropy.
        let admin = format!("ksadm-{}", mixed(31));
        let listing =
            format!("  20: # dashboard admin key\\n  21: DASHBOARD_ADMIN_KEY={admin}\\n  22: ");
        let hits = scan(&listing);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(
            (hits[0].rule_id.as_str(), hits[0].value.as_str()),
            ("custom.generic-api-key", admin.as_str())
        );
        // Named provider formats.
        assert_eq!(
            rule_ids(&format!("token: {}", token(1))),
            ["betterleaks.github-pat"]
        );
        assert_eq!(
            rule_ids(&format!("STRIPE=sk_live_{}", mixed(28))),
            ["betterleaks.stripe-access-token"]
        );
        assert_eq!(
            rule_ids(&format!("sk-or-v1-{}", hex64())),
            ["veles.secrets/openrouter"]
        );
        // A private key inside JSON-escaped tool arguments reads as the key.
        let body = (0..4)
            .map(|i| shifted(i, 64))
            .collect::<Vec<_>>()
            .join("\n");
        let pem = format!(
            "-----BEGIN OPENSSH PRIVATE KEY-----\n{body}\n-----END OPENSSH PRIVATE KEY-----"
        );
        let escaped = serde_json::to_string(&json!({"content": pem})).unwrap();
        let hits = scan(&escaped);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].value, pem);
        // Prose, placeholders and low-entropy values are not secrets.
        assert!(rule_ids("the quick brown fox jumps over the lazy dog").is_empty());
        assert!(rule_ids("api_key = \"your-api-key-here\"").is_empty());
        assert!(rule_ids("password = aaaaaaaaaaaa").is_empty());
    }

    #[test]
    fn names_the_variable_from_the_text() {
        let gh = token(3);
        for (text, name) in [
            (format!("# ci\nGITHUB_TOKEN={gh}\n"), "GITHUB_TOKEN"),
            (format!("ci:\n  github_token: {gh}\n"), "github_token"),
            (format!("{{\"gh_token\": \"{gh}\"}}"), "gh_token"),
            (format!("export GH_PAT={gh}"), "GH_PAT"),
            // Tool arguments as a request body carries them, escaped.
            (
                serde_json::to_string(&json!({"x": gh}).to_string()).unwrap(),
                "x",
            ),
        ] {
            let hits = scan(&text);
            assert_eq!(hits[0].variable.as_deref(), Some(name), "{text}");
        }
    }

    #[test]
    fn maps_lines_and_columns() {
        let gh = token(4);
        let text = format!("first\nsecond line\n  key = {gh} # here\nlast");
        let h = &scan(&text)[0];
        assert_eq!(
            (h.line, h.column, h.end_line, h.end_column),
            (3, 9, 3, 8 + gh.len())
        );
        // Kingfisher's own: columns from 0, and byte offsets.
        assert_eq!(h.detection.location.line, 3);
        assert_eq!(h.detection.location.column, 8);
        assert_eq!(
            &text[h.detection.location.start_offset..h.detection.location.end_offset],
            gh
        );
        // Columns count characters, not bytes.
        let h = &scan(&format!("é → {gh}"))[0];
        assert_eq!(h.column, 5);
        // A private key spans lines.
        let body = (0..3)
            .map(|i| shifted(i, 64))
            .collect::<Vec<_>>()
            .join("\n");
        let pem = format!(
            "-----BEGIN OPENSSH PRIVATE KEY-----\n{body}\n-----END OPENSSH PRIVATE KEY-----"
        );
        let h = &scan(&format!("# deploy key\n{pem}\n"))[0];
        assert_eq!((h.line, h.end_line), (2, 6));
        // A file read from line 40 numbers its lines from there.
        let file = format!("import os\n\nAPI_TOKEN = \"{gh}\"\nprint(API_TOKEN)");
        let h = &scanner()
            .scan_at(At::new(&file).path("/r/app.py").first_line(40), &[])
            .unwrap()[0];
        assert_eq!(h.line, 42);
        assert_eq!(h.variable.as_deref(), Some("API_TOKEN"));
        assert_eq!(h.detection.scanned_as, "/r/app.py");
        let around: Vec<usize> = h.context.iter().map(|l| l.line).collect();
        assert_eq!(around, [40, 41, 42, 43]);
        assert_eq!(h.context[2].text, h.snippet);
    }

    #[test]
    fn caps_the_context() {
        let gh = token(5);
        let blob = "QUJD".repeat(100);
        let long = "word ".repeat(80);
        let text = format!("a\nb\nc\nd\nblob = {blob}\n{long}token = {gh} then {blob}\ne\nf\ng\nh");
        let h = &scan(&text)[0];
        assert_eq!(h.value, gh);
        assert_eq!(h.line, 6);
        let around: Vec<usize> = h.context.iter().map(|l| l.line).collect();
        assert_eq!(around, (3..=9).collect::<Vec<_>>());
        // The line is cut around the secret, which it shows whole.
        assert!(h.snippet.chars().count() <= LINE_CAP + 2, "{}", h.snippet);
        assert!(
            h.snippet.starts_with('…') && h.snippet.contains(&gh),
            "{}",
            h.snippet
        );
        // No long base64 run is shown whole.
        assert!(h.context.iter().all(|l| !l.text.contains(&blob)));
        assert!(
            h.context[2].text.contains("…[400 chars]…"),
            "{:?}",
            h.context
        );
        assert_eq!(clip("short line", None), "short line");
    }

    #[test]
    fn finds_a_secret_inside_base64() {
        use base64::Engine;
        let gh = format!("ghp_{}", shifted(2, 36));
        let blob = base64::engine::general_purpose::STANDARD.encode(format!("GH_TOKEN={gh}\n"));
        let text = format!("line one\nconfig: {blob}\n");
        let hits = scan(&text);
        let h = hits
            .iter()
            .find(|h| h.value == gh)
            .expect("the decoded token");
        assert!(h.detection.base64);
        assert_eq!(h.rule_id, "betterleaks.github-pat");
        // Where the encoded run is, and the name it is assigned to.
        assert_eq!((h.line, h.column), (2, 9));
        assert_eq!(h.variable.as_deref(), Some("config"));
        assert_eq!(h.snippet, format!("config: {blob}"));
    }

    #[test]
    fn pairs_components_and_keeps_kingfisher_detail() {
        let id = format!(
            "AKIA{}",
            "QWERTYUPASDFGHJKLZXCVBNM234567"
                .chars()
                .cycle()
                .skip(3)
                .take(16)
                .collect::<String>()
        );
        let secret: String = "Q7wE9rT2yU4iO6pA8sD1fG3hJ5kL0zXcVbNm+/"
            .chars()
            .cycle()
            .skip(5)
            .take(40)
            .collect();
        let credentials =
            format!("[default]\naws_access_key_id = {id}\naws_secret_access_key = {secret}\n");
        let mut hits = scanner()
            .scan_at(At::new(&credentials).path("message/user"), &[])
            .unwrap();
        let mut grouped: Vec<(usize, &mut Hit)> = hits.iter_mut().map(|h| (0, h)).collect();
        crate::pair(&mut grouped);
        let key = hits
            .iter()
            .find(|h| h.rule_id == "betterleaks.aws-access-token")
            .unwrap_or_else(|| panic!("{hits:?}"));
        assert_eq!(key.value, id);
        assert_eq!(key.variable.as_deref(), Some("aws_access_key_id"));
        let dep = &key.detection.depends_on[0];
        assert_eq!(
            (dep.rule_id.as_str(), dep.variable.as_str()),
            ("betterleaks.aws-secret-access-key", "AWS_SECRET_ACCESS_KEY")
        );
        let paired = &key.detection.paired[0];
        assert_eq!((paired.value.as_str(), paired.line), (secret.as_str(), 3));
        assert!(paired.same_text);
        // The component is a hidden hit, paired with its key; the rule is
        // named and its validator recorded, never run.
        assert!(
            hits.iter()
                .filter(|h| h.rule_id == "betterleaks.aws-secret-access-key")
                .all(|h| !h.visible)
        );
        assert!(
            key.rule_name.contains("AWS access key"),
            "{}",
            key.rule_name
        );
        assert!(key.detection.validation.is_some());
        assert!(key.detection.fingerprint.parse::<u64>().is_ok());
        assert_eq!(key.detection.scanned_as, "message/user");
    }

    #[test]
    fn betterleaks_path_filters_read_the_path() {
        let text = format!("token: {}", token(6));
        let rules = |path: &str| -> Vec<String> {
            scanner()
                .scan_at(At::new(&text).path(path), &[])
                .unwrap()
                .into_iter()
                .map(|h| h.rule_id)
                .collect()
        };
        assert_eq!(rules("src/app.py"), ["betterleaks.github-pat"]);
        // A lock file is generated, not written: Betterleaks skips it.
        assert!(!rules("package-lock.json").contains(&"betterleaks.github-pat".to_owned()));
    }

    #[test]
    fn another_detectors_find_is_merged_unless_kingfisher_has_it() {
        let gh = token(7);
        let odd = format!("zq{}", mixed(30));
        let text = format!("a = {gh}\nb = {odd}\n");
        let extras = [
            Extra {
                rule_id: "trufflehog.github".into(),
                label: "Github".into(),
                severity: "low",
                start: 4,
                end: 4 + gh.len(),
                value: gh.clone(),
            },
            Extra {
                rule_id: "noseyparker.np.generic.1".into(),
                label: "Generic Secret".into(),
                severity: "low",
                start: text.find(&odd).unwrap(),
                end: text.find(&odd).unwrap() + odd.len(),
                value: odd.clone(),
            },
        ];
        let hits = scanner().scan_at(At::new(&text), &extras).unwrap();
        let of = |value: &str| -> Vec<&str> {
            hits.iter()
                .filter(|h| h.visible && h.value == value)
                .map(|h| h.rule_id.as_str())
                .collect()
        };
        assert_eq!(of(&gh), ["betterleaks.github-pat"]);
        assert!(of(&odd).contains(&"noseyparker.np.generic.1"), "{hits:?}");
        let h = hits
            .iter()
            .find(|h| h.rule_id.starts_with("noseyparker"))
            .unwrap();
        assert_eq!((h.line, h.variable.as_deref()), (2, Some("b")));
    }

    #[test]
    fn a_document_is_scrubbed_of_what_the_scanner_finds() {
        let gh = token(8);
        let stripe = format!("sk_live_{}", mixed(28));
        let encoded = serde_json::to_string(&json!({"key": stripe})).unwrap();
        let mut document = json!({
            "messages": [
                {"role": "user", "content": format!("deploy with GITHUB_TOKEN={gh}")},
                {"role": "tool", "content": encoded},
                {"role": "assistant", "content": format!("done; {gh} works")},
            ],
        });
        let outcome = crate::scrub(&mut document, &crate::Redactor::default(), 3, |d| {
            scanner().leftovers(d, b"k", |_| true)
        })
        .unwrap();
        let text = document.to_string();
        assert!(!text.contains(&gh) && !text.contains(&stripe), "{text}");
        assert!(text.contains("[REDACTED:betterleaks.github-pat:"), "{text}");
        assert!(matches!(outcome, crate::Scrubbed::Clean { redactions } if redactions >= 3));
        // A value the host says is not a secret is left.
        let mut document = json!(format!("GITHUB_TOKEN={gh}"));
        crate::scrub(&mut document, &crate::Redactor::default(), 3, |d| {
            scanner().leftovers(d, b"k", |hit| hit.value != gh)
        })
        .unwrap();
        assert!(document.as_str().unwrap().contains(&gh));
    }

    #[cfg(feature = "payment")]
    #[test]
    fn a_card_number_is_named_as_one_not_as_a_generic_key() {
        let hits = scan("card_number = 4242424242424242");
        assert_eq!(hits[0].rule_id, "card.visa");
        assert_eq!(hits[0].variable.as_deref(), Some("card_number"));
        assert!(hits[0].detection.issuer.is_some());
        let hits = scanner()
            .scan_at(
                At::new("card_number = 4242424242424242").payment(false),
                &[],
            )
            .unwrap();
        assert!(hits.iter().all(|h| h.rule_id != "card.visa"));
    }
}
