//! Versioned Swarm ↔ security-tester control-plane wire contract.
//!
//! These types deliberately carry lifecycle/configuration references and
//! summarized status only. Target credentials, provider credentials, raw
//! evidence, and interception-CA private keys are never valid members.

use serde::{Deserialize, Serialize};

pub const API_VERSION: u16 = 1;
pub const INSTANCE_KIND: &str = "security-tester.v1";
pub const CAPABILITY_FLAG: &str = "security_tester_v1";
pub const HARNESS_PROFILE: &str = "byo:glitch";
pub const MODEL_ROUTE: &str = "swarm-cyber";
pub const CONTROL_MEDIA_TYPE: &str = "application/vnd.security-tester.control.v1+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessProfile {
    #[serde(rename = "byo:glitch")]
    Glitch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelRoute {
    #[serde(rename = "swarm-cyber")]
    SwarmCyber,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigureRequest {
    pub api_version: u16,
    pub instance_id: String,
    pub tenant_id: String,
    pub workload_identity: String,
    pub recorder_identity: String,
    pub harness_profile: HarnessProfile,
    pub model_route: ModelRoute,
    pub mcp_route: String,
    pub public_recorder_ca_sha256: String,
    pub compiled_scope_version_id: Option<String>,
    pub compiled_scope_sha256: Option<String>,
    pub expires_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessState {
    Disabled,
    Configuring,
    ObserveReady,
    ActiveReady,
    Paused,
    Unready,
    Killed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthResponse {
    pub api_version: u16,
    pub state: ReadinessState,
    pub recorder_ready: bool,
    pub evidence_sink_writable: bool,
    pub nonterminal_flow_reservations: u64,
    pub scope_version_id: Option<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlAction {
    Pause,
    ResumeObserve,
    StartActive,
    Cancel,
    Kill,
    TightenScope,
    RevokeTokens,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    pub api_version: u16,
    pub action_id: String,
    pub action: ControlAction,
    pub actor_id: String,
    pub reason: String,
    pub scope_version_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionResponse {
    pub api_version: u16,
    pub action_id: String,
    pub accepted: bool,
    pub state: ReadinessState,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotRequest {
    pub api_version: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotResponse {
    pub api_version: u16,
    pub envelope: SnapshotEnvelope,
}

/// Authenticated product-owned snapshot data returned to the trusted control
/// plane for storage. The logical payload stays opaque to Swarm; its schema is
/// versioned and validated by the product during restore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotEnvelope {
    pub sha256: String,
    pub key_id: String,
    pub source_instance_id: String,
    pub source_tenant_id: String,
    pub generation: u64,
    pub hmac_sha256: String,
    pub snapshot: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreRequest {
    pub api_version: u16,
    pub envelope: SnapshotEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceStatus {
    pub api_version: u16,
    pub instance_id: String,
    pub state: ReadinessState,
    pub active_engagements: u32,
    pub pending_approvals: u32,
    pub runnable_intents: u32,
    pub budget_stop: bool,
    pub expires_unix_ms: u64,
    /// What this Glitch build says it is. Absent from releases that predate
    /// self-reporting, so Swarm must treat `None` as "unknown", never as a
    /// match.
    #[serde(default)]
    pub release: Option<ReleaseIdentity>,
}

/// A Glitch build's self-reported identity. `version` and `commit` are baked
/// in at image build time. An image cannot contain its own content digest, so
/// `image_digest` is the digest the control plane rendered into the workload
/// (`GLITCH_IMAGE_DIGEST`) echoed back; the control plane cross-checks it
/// against the kubelet-reported image ID rather than trusting it alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseIdentity {
    pub version: String,
    pub commit: String,
    pub image_digest: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_round_trip_and_no_secret_fields() {
        let request = ConfigureRequest {
            api_version: API_VERSION,
            instance_id: "instance".into(),
            tenant_id: "tenant".into(),
            workload_identity: "workload".into(),
            recorder_identity: "recorder".into(),
            harness_profile: HarnessProfile::Glitch,
            model_route: ModelRoute::SwarmCyber,
            mcp_route: "mcp-route".into(),
            public_recorder_ca_sha256: "00".repeat(32),
            compiled_scope_version_id: None,
            compiled_scope_sha256: None,
            expires_unix_ms: 1,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(
            serde_json::from_str::<ConfigureRequest>(&json).unwrap(),
            request
        );
        for forbidden in ["credential", "private_key", "target_secret", "provider_key"] {
            assert!(!json.contains(forbidden));
        }
    }

    #[test]
    fn status_release_is_optional_and_closed() {
        let old = r#"{"api_version":1,"instance_id":"i","state":"observe_ready","active_engagements":0,"pending_approvals":0,"runnable_intents":0,"budget_stop":false,"expires_unix_ms":1}"#;
        assert_eq!(
            serde_json::from_str::<InstanceStatus>(old).unwrap().release,
            None
        );
        let new = r#"{"api_version":1,"instance_id":"i","state":"observe_ready","active_engagements":0,"pending_approvals":0,"runnable_intents":0,"budget_stop":false,"expires_unix_ms":1,"release":{"version":"0.1.0","commit":"abc1234","image_digest":"sha256:00"}}"#;
        let status = serde_json::from_str::<InstanceStatus>(new).unwrap();
        assert_eq!(status.release.unwrap().commit, "abc1234");
        let extra = r#"{"api_version":1,"instance_id":"i","state":"observe_ready","active_engagements":0,"pending_approvals":0,"runnable_intents":0,"budget_stop":false,"expires_unix_ms":1,"release":{"version":"0.1.0","commit":"abc1234","image_digest":null,"signed":true}}"#;
        assert!(serde_json::from_str::<InstanceStatus>(extra).is_err());
    }

    #[test]
    fn unknown_fields_fail_closed() {
        let value = r#"{"api_version":1,"instance_id":"i","tenant_id":"t","workload_identity":"w","recorder_identity":"r","harness_profile":"byo:glitch","model_route":"swarm-cyber","mcp_route":"p","public_recorder_ca_sha256":"h","compiled_scope_version_id":null,"compiled_scope_sha256":null,"expires_unix_ms":1,"provider_key":"secret"}"#;
        assert!(serde_json::from_str::<ConfigureRequest>(value).is_err());
    }

    #[test]
    fn glitch_profile_and_model_are_single_fixed_contract_values() {
        assert_eq!(HARNESS_PROFILE, "byo:glitch");
        assert_eq!(MODEL_ROUTE, "swarm-cyber");
        assert_eq!(
            serde_json::to_string(&HarnessProfile::Glitch).unwrap(),
            r#""byo:glitch""#
        );
        assert_eq!(
            serde_json::to_string(&ModelRoute::SwarmCyber).unwrap(),
            r#""swarm-cyber""#
        );
        assert!(serde_json::from_str::<HarnessProfile>(r#""byo:other""#).is_err());
        assert!(serde_json::from_str::<ModelRoute>(r#""other-model""#).is_err());
    }

    #[test]
    fn snapshot_contract_keeps_storage_destination_out_of_guest_request() {
        let request = serde_json::to_value(SnapshotRequest {
            api_version: API_VERSION,
        })
        .unwrap();
        assert_eq!(request, serde_json::json!({"api_version": API_VERSION}));

        let envelope = SnapshotEnvelope {
            sha256: "00".repeat(32),
            key_id: "snapshot-key-v1".into(),
            source_instance_id: "instance".into(),
            source_tenant_id: "tenant".into(),
            generation: 1,
            hmac_sha256: "11".repeat(32),
            snapshot: serde_json::json!({"schema_version": 4}),
        };
        let response = SnapshotResponse {
            api_version: API_VERSION,
            envelope: envelope.clone(),
        };
        assert_eq!(
            serde_json::from_value::<SnapshotResponse>(serde_json::to_value(&response).unwrap())
                .unwrap(),
            response
        );
        assert_eq!(
            serde_json::from_value::<RestoreRequest>(
                serde_json::to_value(RestoreRequest {
                    api_version: API_VERSION,
                    envelope,
                })
                .unwrap()
            )
            .unwrap()
            .api_version,
            API_VERSION
        );
        let json = serde_json::to_string(&request).unwrap();
        for forbidden in ["destination", "credential", "bucket", "object_store"] {
            assert!(!json.contains(forbidden));
        }
    }
}
