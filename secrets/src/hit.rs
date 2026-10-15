//! What a detector reports: one secret in one piece of text, with where it
//! is, the lines around it and everything the detector said about it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A secret found in one piece of text.
#[derive(Debug, Clone)]
pub struct Hit {
    /// The rule, e.g. `betterleaks.github-pat`, `card.visa`, its name for a
    /// table and its full name (Betterleaks' is a sentence describing it).
    pub rule_id: String,
    pub label: String,
    pub rule_name: String,
    /// The detector's confidence in the rule: `high`, `medium` or `low`.
    pub severity: String,
    /// Byte offsets of the secret in the text (for a secret inside base64,
    /// of the encoded run).
    pub start: usize,
    pub end: usize,
    pub value: String,
    pub entropy: f64,
    /// Where it starts and ends in the text, lines from 1 and columns (in
    /// characters) from 1, the end being its last character.
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    /// The name it is assigned to, when the text says.
    pub variable: Option<String>,
    /// The line it is on, capped, and the lines around it.
    pub snippet: String,
    pub context: Vec<ContextLine>,
    pub detection: Detection,
    /// The rule is reported only as a component of another rule's finding
    /// (the secret half of an AWS key pair, a client id): it is paired with
    /// that finding (see [`crate::pair`]) and not listed on its own.
    pub visible: bool,
}

/// One line of a finding's surroundings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ContextLine {
    pub line: usize,
    /// Capped in length; long base64 runs show only their ends.
    pub text: String,
}

/// Everything the detector says about a finding and the rule behind it.
/// Validation and revocation are named, never run.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Detection {
    /// Kingfisher's own fingerprint of the match (it depends on the text it
    /// was found in), as a decimal string.
    pub fingerprint: String,
    /// The path the text was scanned as, which Betterleaks' path filters
    /// read: a file's path for code, or a label such as `message/user`.
    pub scanned_as: String,
    /// Found by decoding a base64 run; the location is the run's.
    pub base64: bool,
    /// Every named capture group of the match.
    pub captures: BTreeMap<String, String>,
    /// A card number's issuer: the bank its first six digits name, its
    /// country, and what kind of card it is; an IBAN's country.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<Issuer>,
    /// Kingfisher's location in the text it scanned: lines from 1, columns
    /// from 0, byte offsets.
    pub location: KingfisherLocation,
    /// The entropy the rule requires, in bits per character.
    pub min_entropy: f64,
    pub references: Vec<String>,
    /// How Kingfisher would check (`Http`, `AWS`, `Betterleaks`, `JWT`…) or
    /// revoke the secret, were it asked to. It never is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revocation: Option<String>,
    /// Rules this one needs a match of nearby, and the matches found for
    /// them (see [`crate::pair`]).
    pub depends_on: Vec<Dependency>,
    pub paired: Vec<Paired>,
    /// Character requirements the rule sets on a match.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Option<Object>))]
    pub pattern_requirements: Option<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct KingfisherLocation {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub start_offset: usize,
    pub end_offset: usize,
}

/// A rule another rule depends on (`depends_on_rule`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Dependency {
    pub rule_id: String,
    /// The name Kingfisher gives the dependency's value, e.g.
    /// `AWS_SECRET_ACCESS_KEY`.
    pub variable: String,
    pub optional: bool,
    /// How near it must be (`5L`: within five lines); none when the rule
    /// only uses it to validate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
}

/// A match of a dependency, found with the finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Paired {
    pub rule_id: String,
    pub variable: String,
    pub value: String,
    /// Its line in its own text.
    pub line: usize,
    /// It was in the same text as the finding, not elsewhere in the group.
    pub same_text: bool,
}

/// Who issued a card, by its first six digits. Of an IBAN, only the
/// country it names.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Issuer {
    /// The six digits.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bin: String,
    /// The scheme as the table names it: `VISA`, `MASTERCARD`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub scheme: String,
    /// `CREDIT`, `DEBIT` or `CHARGE CARD`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub funding: String,
    /// `CLASSIC`, `PLATINUM`, `PREPAID`, `BUSINESS` and the like.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub category: String,
    /// The issuing bank.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bank: String,
    /// The bank's country, as two letters.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub country: String,
}

/// A secret a detector other than Kingfisher found in a text (see
/// [`crate::external`]), before it is placed as a [`Hit`].
#[derive(Debug, Clone, PartialEq)]
pub struct Extra {
    /// `betterleaks.generic-api-key`, `trufflehog.jdbc`,
    /// `noseyparker.np.generic.1`.
    pub rule_id: String,
    pub label: String,
    pub severity: &'static str,
    /// Byte offsets of the value in the text; both zero when the detector
    /// found it in a decoding of the text and it is nowhere in it verbatim.
    pub start: usize,
    pub end: usize,
    pub value: String,
}

/// Attach to each hit the matches of the rules it depends on (an AWS key
/// id's secret key), nearest first, from the same text (`group`) when there
/// is one there, else from anywhere in `hits`. Kingfisher's library only
/// drops a Betterleaks finding whose component is missing; it does not say
/// which match completed it, and leaves its own rules' pairs to validation,
/// which is never run here.
pub fn pair(hits: &mut [(usize, &mut Hit)]) {
    let pool: Vec<(usize, String, String, usize)> = hits
        .iter()
        .map(|(group, h)| (*group, h.rule_id.clone(), h.value.clone(), h.line))
        .collect();
    for (group, hit) in hits.iter_mut() {
        let mut paired = Vec::new();
        for dep in &hit.detection.depends_on {
            let best = pool
                .iter()
                .filter(|(_, rule, value, _)| *rule == dep.rule_id && *value != hit.value)
                .min_by_key(|(g, _, _, line)| (*g != *group, line.abs_diff(hit.line)));
            if let Some((g, rule, value, line)) = best {
                paired.push(Paired {
                    rule_id: rule.clone(),
                    variable: dep.variable.clone(),
                    value: value.clone(),
                    line: *line,
                    same_text: g == group,
                });
            }
        }
        hit.detection.paired = paired;
    }
}
