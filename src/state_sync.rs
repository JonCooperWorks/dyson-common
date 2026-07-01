//! Dyson ↔ swarm durable-state sync contract.
//!
//! Dyson mirrors selected workspace/chat files to swarm (the durable
//! source of truth) over `POST /v1/internal/state-file`. Both halves of
//! that wire — the *which files are durable* predicate and the upload
//! envelope — used to be re-implemented on each side and drifted (dyson
//! matched `.md` case-sensitively, swarm case-insensitively, so a
//! `NOTES.MD` passed swarm's validator but was never pushed: silently
//! non-durable, lost on VM reset). This module is the single canonical
//! copy; dyson pushes exactly what swarm accepts.
//!
//! Env-var names for the sandbox contract live here too: swarm injects
//! them at hire time and dyson reads them at boot, and neither side can
//! import the other, so string drift was previously invisible to the
//! compiler.

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

/// State-file namespaces.
pub const STATE_NAMESPACE_WORKSPACE: &str = "workspace";
pub const STATE_NAMESPACE_CHATS: &str = "chats";

/// Directory under `workspace/` whose entire tree is durable.
pub const STATE_SKILLS_DIR: &str = "skills";

/// Per-chat transcript / title file names.
pub const STATE_CHAT_TRANSCRIPT_SUFFIX: &str = "/transcript.json";
pub const STATE_CHAT_TITLE_SUFFIX: &str = "/title.txt";

/// Env-var names of the sandbox ↔ swarm runtime contract. Swarm's
/// orchestrator injects these into every sandbox; dyson (and the
/// sidecar tooling) read them back.
pub const ENV_PROXY_URL: &str = "SWARM_PROXY_URL";
pub const ENV_PROXY_TOKEN: &str = "SWARM_PROXY_TOKEN";
pub const ENV_INSTANCE_ID: &str = "SWARM_INSTANCE_ID";
pub const ENV_STATE_SYNC_URL: &str = "SWARM_STATE_SYNC_URL";
pub const ENV_STATE_SYNC_TOKEN: &str = "SWARM_STATE_SYNC_TOKEN";

/// One mirrored file change on the wire. Serialized by dyson's state
/// sync worker, deserialized by swarm's internal-state route — one
/// struct so a field rename can never split the two sides.
///
/// `deleted`/`body_b64`/`mime` keep `#[serde(default)]` for
/// compatibility with older senders; `deleted: true` + `body_b64:
/// None` is a tombstone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateFileEnvelope {
    pub namespace: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_b64: Option<String>,
}

/// Is `path` (relative, within `namespace`) durable state that dyson
/// must push and swarm must accept?
///
/// Canonical policy:
/// - `chats/**` — everything (transcripts, titles, per-chat files).
/// - `workspace/*.md` — top-level markdown (case-insensitive).
/// - `workspace/memory/**.md` — memory notes (case-insensitive).
/// - `workspace/kb/**`, `workspace/skills/**` — whole trees.
/// - `workspace/channels/<ch>/*.md`, `.../_audit.jsonl`,
///   `.../memory/**.md` — per-channel state.
/// - Hidden/unclean components (dotfiles, `..`, non-UTF-8) are never
///   durable in either namespace.
pub fn is_durable_state_path(namespace: &str, path: &str) -> bool {
    let rel = Path::new(path);
    if path.is_empty() || has_hidden_or_unclean_component(rel) {
        return false;
    }
    match namespace {
        STATE_NAMESPACE_CHATS => true,
        STATE_NAMESPACE_WORKSPACE => should_sync_workspace_path(rel),
        _ => false,
    }
}

fn should_sync_workspace_path(rel: &Path) -> bool {
    let parts: Vec<&str> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();
    match parts.as_slice() {
        [file] => has_extension(file, "md"),
        ["memory", .., file] => has_extension(file, "md"),
        ["kb", ..] | [STATE_SKILLS_DIR, ..] => true,
        ["channels", _channel, rest @ ..] => should_sync_channel_workspace(rest),
        _ => false,
    }
}

fn should_sync_channel_workspace(parts: &[&str]) -> bool {
    match parts {
        [file] => has_extension(file, "md") || *file == "_audit.jsonl",
        ["memory", .., file] => has_extension(file, "md"),
        _ => false,
    }
}

/// Case-insensitive extension check (`NOTES.MD` is durable markdown).
fn has_extension(file_name: &str, expected: &str) -> bool {
    Path::new(file_name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(expected))
}

/// Any dotfile, empty, non-UTF-8, or non-`Normal` (`..`, root, prefix)
/// component disqualifies a path.
pub fn has_hidden_or_unclean_component(path: &Path) -> bool {
    let mut saw_component = false;
    for component in path.components() {
        saw_component = true;
        match component {
            Component::Normal(part) => {
                let Some(s) = part.to_str() else {
                    return true;
                };
                if s.is_empty() || s.starts_with('.') {
                    return true;
                }
            }
            _ => return true,
        }
    }
    !saw_component
}

/// A zero-byte `transcript.json` is a torn write, never valid chat
/// state (a real empty transcript serializes as `[]`). Both sides drop
/// it: dyson must not push it, swarm must not let it clobber a good
/// sealed copy.
pub fn is_zero_byte_chat_transcript(namespace: &str, path: &str, body: &[u8]) -> bool {
    namespace == STATE_NAMESPACE_CHATS
        && path.ends_with(STATE_CHAT_TRANSCRIPT_SUFFIX)
        && body.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_markdown_is_durable_case_insensitively() {
        for p in ["IDENTITY.md", "NOTES.MD", "Readme.Md"] {
            assert!(
                is_durable_state_path(STATE_NAMESPACE_WORKSPACE, p),
                "{p} must be durable"
            );
        }
        assert!(!is_durable_state_path(
            STATE_NAMESPACE_WORKSPACE,
            "notes.txt"
        ));
    }

    #[test]
    fn memory_kb_skills_and_channels_policies() {
        let ws = STATE_NAMESPACE_WORKSPACE;
        assert!(is_durable_state_path(ws, "memory/deep/note.md"));
        assert!(is_durable_state_path(ws, "memory/NOTE.MD"));
        assert!(!is_durable_state_path(ws, "memory/data.json"));
        assert!(is_durable_state_path(ws, "kb/anything/at-all.bin"));
        assert!(is_durable_state_path(ws, "skills/foo/SKILL.md"));
        assert!(is_durable_state_path(ws, "channels/tg/README.md"));
        assert!(is_durable_state_path(ws, "channels/tg/_audit.jsonl"));
        assert!(is_durable_state_path(ws, "channels/tg/memory/x.md"));
        assert!(!is_durable_state_path(ws, "channels/tg/data.json"));
        assert!(!is_durable_state_path(ws, "src/main.rs"));
    }

    #[test]
    fn chats_namespace_takes_everything_clean() {
        assert!(is_durable_state_path(
            STATE_NAMESPACE_CHATS,
            "c1/transcript.json"
        ));
        assert!(!is_durable_state_path(
            STATE_NAMESPACE_CHATS,
            ".hidden/t.json"
        ));
    }

    #[test]
    fn unclean_components_are_rejected() {
        for p in ["", ".git/config", "a/../b.md", "a/.hidden.md", "/abs.md"] {
            assert!(
                !is_durable_state_path(STATE_NAMESPACE_WORKSPACE, p),
                "{p:?} must be rejected"
            );
        }
    }

    #[test]
    fn zero_byte_transcript_detection() {
        assert!(is_zero_byte_chat_transcript(
            STATE_NAMESPACE_CHATS,
            "c1/transcript.json",
            b""
        ));
        assert!(!is_zero_byte_chat_transcript(
            STATE_NAMESPACE_CHATS,
            "c1/transcript.json",
            b"[]"
        ));
        assert!(!is_zero_byte_chat_transcript(
            STATE_NAMESPACE_CHATS,
            "c1/title.txt",
            b""
        ));
    }

    #[test]
    fn envelope_round_trips_and_tolerates_missing_optionals() {
        let e = StateFileEnvelope {
            namespace: STATE_NAMESPACE_CHATS.into(),
            path: "c1/transcript.json".into(),
            mime: Some("application/json".into()),
            updated_at: 123,
            deleted: false,
            body_b64: Some("W10=".into()),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: StateFileEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.path, e.path);
        assert_eq!(back.body_b64, e.body_b64);

        let tombstone: StateFileEnvelope = serde_json::from_str(
            r#"{"namespace":"chats","path":"c1/transcript.json","updated_at":1,"deleted":true}"#,
        )
        .unwrap();
        assert!(tombstone.deleted);
        assert!(tombstone.body_b64.is_none());
    }
}
