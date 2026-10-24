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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigureRequest {
    pub api_version: u16,
    pub instance_id: String,
    pub tenant_id: String,
    pub workload_identity: String,
    pub recorder_identity: String,
    pub harness_profile: String,
    pub model_route: String,
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
    pub snapshot_id: String,
    pub destination_ref: String,
    pub deadline_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotResponse {
    pub api_version: u16,
    pub snapshot_id: String,
    pub logical_schema_version: u32,
    pub sha256: String,
    pub bytes: u64,
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
            harness_profile: HARNESS_PROFILE.into(),
            model_route: MODEL_ROUTE.into(),
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
    fn unknown_fields_fail_closed() {
        let value = r#"{"api_version":1,"instance_id":"i","tenant_id":"t","workload_identity":"w","recorder_identity":"r","harness_profile":"byo:glitch","model_route":"swarm-cyber","mcp_route":"p","public_recorder_ca_sha256":"h","compiled_scope_version_id":null,"compiled_scope_sha256":null,"expires_unix_ms":1,"provider_key":"secret"}"#;
        assert!(serde_json::from_str::<ConfigureRequest>(value).is_err());
    }

    #[test]
    fn glitch_profile_and_model_are_single_fixed_contract_values() {
        assert_eq!(HARNESS_PROFILE, "byo:glitch");
        assert_eq!(MODEL_ROUTE, "swarm-cyber");
    }
}
