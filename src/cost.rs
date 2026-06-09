//! Cost-row wire DTO for the `/v1/internal/audit/calls` contract.
//!
//! `dyson-swarm` produces these from the audit store; `dyson` consumes them to
//! render per-call cost. This is the authoritative superset — readers that
//! only need a subset rely on serde defaults to ignore the rest.

use serde::{Deserialize, Serialize};

/// One recorded LLM call with its token + cost accounting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentCostCall {
    pub audit_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    pub instance_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub key_source: String,
    pub status_code: i64,
    pub occurred_at: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cost_usd: Option<f64>,
    pub cost_source: String,
    /// Latency to first streamed byte (ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<i64>,
    /// First→last byte streaming duration (ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_ms: Option<i64>,
    /// Output tokens per second. Prefers the OpenRouter-reconciled
    /// (`native_output_tokens / gen_time_ms`) figure when present, else the
    /// proxy-local (`output_tokens / stream_ms`) one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tok_per_sec: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_generation_id: Option<String>,
    /// OpenRouter authoritative generation time (ms), backfilled by swarmctl.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gen_time_ms: Option<i64>,
    /// OpenRouter native completion token count, backfilled by swarmctl.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_output_tokens: Option<i64>,
    /// When swarmctl reconciled this row against OpenRouter (`None` = local-only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciled_at: Option<i64>,
    /// The provider's raw `usage` block (parsed), as captured at request time.
    /// Carries the per-request token + cost breakdown the UI expands. `None`
    /// for rows with no captured usage block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_usage: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_row_round_trips_and_omits_empty_optionals() {
        let call = RecentCostCall {
            audit_id: 1,
            owner_id: None,
            instance_id: "i".into(),
            provider: "anthropic".into(),
            model: Some("claude".into()),
            key_source: "byo".into(),
            status_code: 200,
            occurred_at: 42,
            input_tokens: Some(10),
            output_tokens: Some(20),
            total_tokens: Some(30),
            cost_usd: Some(0.01),
            cost_source: "provider".into(),
            ttft_ms: None,
            stream_ms: None,
            tok_per_sec: None,
            upstream_generation_id: None,
            gen_time_ms: None,
            native_output_tokens: None,
            reconciled_at: None,
            provider_usage: None,
        };
        let json = serde_json::to_string(&call).unwrap();
        // Empty optionals are omitted from the wire (skip_serializing_if).
        assert!(!json.contains("owner_id"));
        assert!(!json.contains("reconciled_at"));
        let back: RecentCostCall = serde_json::from_str(&json).unwrap();
        assert_eq!(back, call);
    }

    #[test]
    fn deserializes_when_reconciliation_fields_absent() {
        // A producer that predates the OpenRouter columns still deserializes.
        let json = r#"{
            "audit_id": 7, "instance_id": "i", "provider": "openrouter",
            "model": null, "key_source": "shared", "status_code": 200,
            "occurred_at": 1, "input_tokens": null, "output_tokens": null,
            "total_tokens": null, "cost_usd": null, "cost_source": "estimate"
        }"#;
        let call: RecentCostCall = serde_json::from_str(json).unwrap();
        assert_eq!(call.audit_id, 7);
        assert!(call.reconciled_at.is_none());
    }
}
