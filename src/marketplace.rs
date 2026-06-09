//! Skill-marketplace catalog wire DTOs.
//!
//! `dyson-swarm` publishes the catalog; the `dyson` agent's marketplace client
//! consumes it. These are the shared, serializable contract types. (Swarm's
//! `CatalogListing` is intentionally *not* here — it embeds a swarm-internal
//! source-config view; swarm keeps that locally and builds it from these.)
//!
//! Derives both `Serialize` and `Deserialize`: swarm only serialized them, the
//! dyson client only deserialized — sharing one definition with both halves
//! removes the drift that previously crept in (`license`/`min_dyson_version`/
//! `author` that the client silently dropped).

use serde::{Deserialize, Serialize};

/// How a package's `skill.md` is delivered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SkillPackageContent {
    Inline { skill_md: String },
    Url { url: String },
}

/// A skill as it appears in a published catalog listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogSkill {
    pub marketplace_id: String,
    pub marketplace_name: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    // Optional metadata — `serde(default)` so a listing that omits these
    // (rather than sending null) still deserializes.
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub min_dyson_version: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    pub content_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<CatalogSkillAuthor>,
}

/// Attribution for a catalog skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogSkillAuthor {
    pub name: String,
    pub instance_id: String,
    pub href: String,
}

/// A marketplace source that failed to list, surfaced alongside the skills
/// that did resolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogError {
    pub marketplace_id: String,
    pub error: String,
}

/// Detail view for a single package: the catalog entry plus a preview and the
/// server-computed content hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPackageDetail {
    pub skill: CatalogSkill,
    pub preview: String,
    pub computed_sha256: String,
}

/// The full package body delivered on install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPackageBody {
    pub marketplace_id: String,
    pub marketplace_name: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub declared_sha256: Option<String>,
    pub computed_sha256: String,
    pub skill_md: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_skill_round_trips_with_and_without_author() {
        let skill = CatalogSkill {
            marketplace_id: "m1".into(),
            marketplace_name: "Main".into(),
            name: "linter".into(),
            version: "1.2.0".into(),
            description: "lints".into(),
            tags: vec!["dev".into()],
            license: Some("MIT".into()),
            min_dyson_version: Some("0.2.0".into()),
            sha256: Some("abc".into()),
            content_type: "text/markdown".into(),
            author: None,
        };
        let json = serde_json::to_string(&skill).unwrap();
        assert!(!json.contains("\"author\"")); // omitted when None
        assert_eq!(serde_json::from_str::<CatalogSkill>(&json).unwrap(), skill);

        let with_author = CatalogSkill {
            author: Some(CatalogSkillAuthor {
                name: "ada".into(),
                instance_id: "i".into(),
                href: "https://x".into(),
            }),
            ..skill
        };
        let json = serde_json::to_string(&with_author).unwrap();
        assert_eq!(
            serde_json::from_str::<CatalogSkill>(&json).unwrap(),
            with_author
        );
    }

    #[test]
    fn package_content_is_externally_tagged_lowercase() {
        let inline = SkillPackageContent::Inline {
            skill_md: "# hi".into(),
        };
        assert_eq!(
            serde_json::to_value(&inline).unwrap(),
            serde_json::json!({ "type": "inline", "skill_md": "# hi" })
        );
        let url = SkillPackageContent::Url {
            url: "https://x".into(),
        };
        assert_eq!(
            serde_json::to_value(&url).unwrap(),
            serde_json::json!({ "type": "url", "url": "https://x" })
        );
    }
}
