//! Named wire contracts between Dyson agents and the Swarm control plane.
//!
//! This module intentionally contains small constants only. The contracts
//! remain implemented in their owning services; the constants keep repeated
//! route/header strings from drifting silently across the two independent
//! repos (the exact failure this crate exists to prevent — before it, one
//! side spelled the configure header `X-Swarm-Configure` and the other
//! `x-swarm-configure`, working only because HTTP headers are case-insensitive).

/// Header Dyson reads to verify the per-instance configure secret.
pub const DYSON_CONFIGURE_HEADER: &str = "X-Swarm-Configure";

/// Header Dyson's CSRF gate requires on state-changing `/api/*` calls.
pub const DYSON_CSRF_HEADER: &str = "X-Dyson-CSRF";

/// CSRF marker for Swarm-internal runtime/admin calls into Dyson.
pub const DYSON_INTERNAL_CSRF_VALUE: &str = "swarm-internal";

/// CSRF marker for Swarm webhook delivery calls into Dyson chats.
pub const DYSON_WEBHOOK_CSRF_VALUE: &str = "swarm-webhook";

/// Header carrying the swarm-minted audit id that correlates a proxied LLM
/// call between the swarm cost-proxy (which mints it) and the dyson agent
/// (which echoes it). Value is lowercase because dyson sets it as an
/// outbound request header where casing is preserved verbatim.
pub const SWARM_LLM_AUDIT_ID_HEADER: &str = "x-swarm-llm-audit-id";

pub const DYSON_ADMIN_CONFIGURE_ENDPOINT: &str = "configure";
pub const DYSON_ADMIN_SKILLS_ENDPOINT: &str = "skills";
pub const DYSON_ADMIN_SKILL_INSTALL_ENDPOINT: &str = "skills/install";
pub const DYSON_ADMIN_STATE_FILE_ENDPOINT: &str = "state/file";
pub const DYSON_ADMIN_IDLE_ENDPOINT: &str = "idle";
pub const DYSON_ADMIN_QUIESCE_ENDPOINT: &str = "quiesce";
pub const DYSON_ADMIN_UNQUIESCE_ENDPOINT: &str = "unquiesce";

/// Internal Swarm endpoint Dyson posts finalised artefacts to.
pub const SWARM_INTERNAL_ARTEFACT_INGEST_PATH: &str = "/v1/internal/ingest/artefact";

/// Internal Swarm endpoint Dyson posts durable state-file changes to.
pub const SWARM_INTERNAL_STATE_FILE_PATH: &str = "/v1/internal/state/file";

/// Same-origin escape route exposed on Dyson sandbox subdomains.
pub const SWARM_SHARE_MINT_PATH: &str = "/_swarm/share-mint";

/// Guest security telemetry: the Falco sensor inside a sandbox VM, the host
/// collector that reads it, and Swarm's ingest of what they report. The
/// sensor forwards Falco's own JSON records; these are the names both ends of
/// each hop rely on.
pub mod guest_security {
    /// AF_VSOCK port the in-VM sensor serves its record stream on.
    pub const VSOCK_PORT: u32 = 9473;

    /// The only vsock peer the sensor accepts: the hypervisor host
    /// (`VMADDR_CID_HOST`).
    pub const HOST_CID: u32 = 2;

    /// Falco rule tag that marks a telemetry record rather than a detection.
    pub const TELEMETRY_TAG: &str = "dyson_telemetry";

    /// Telemetry rules: every successful exec, process exit and TCP connect.
    pub const EXEC_RULE: &str = "dyson_exec";
    pub const EXIT_RULE: &str = "dyson_exit";
    pub const CONNECT_RULE: &str = "dyson_connect";

    /// Output fields the sensor adds to Falco's: the start (Falco clock, ns)
    /// of the process's thread-group leader, of ancestor `k`
    /// (`dyson.apid.ts[k]`), and whether argv was cut.
    pub const FIELD_PID_START: &str = "dyson.pid.ts";
    pub const FIELD_ANCESTOR_START_PREFIX: &str = "dyson.apid.ts[";
    pub const FIELD_ARGS_TRUNCATED: &str = "dyson.args_truncated";

    /// Process entity ids are `base64("<namespace>:<start ns>:<pid>")`.
    pub const ENTITY_NAMESPACE: &str = "guest";

    /// Bounds on a record's argv (`proc.exe` + `proc.args[N]`).
    pub const MAX_ARGUMENTS: usize = 64;
    pub const MAX_ARGUMENT_BYTES: usize = 4096;

    /// Swarm's internal ingest for guest security events and findings.
    pub const SWARM_INGEST_PATH: &str = "/v1/internal/ingest/security-events";

    /// Finding `kind` for a Falco detection.
    pub const FINDING_KIND_FALCO: &str = "falco";

    /// Falco priority (lowercase) -> Swarm finding severity. `debug` is never
    /// a finding (telemetry runs at that priority).
    pub const PRIORITY_SEVERITY: [(&str, Option<&str>); 8] = [
        ("emergency", Some("high")),
        ("alert", Some("high")),
        ("critical", Some("high")),
        ("error", Some("high")),
        ("warning", Some("medium")),
        ("notice", Some("low")),
        ("informational", Some("low")),
        ("debug", None),
    ];

    /// Swarm severity for a Falco priority (any case); `None` for `debug` or
    /// an unknown priority.
    #[must_use]
    pub fn severity(priority: &str) -> Option<&'static str> {
        PRIORITY_SEVERITY
            .iter()
            .find(|(p, _)| p.eq_ignore_ascii_case(priority))
            .and_then(|(_, s)| *s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dyson_admin_contract_constants_are_stable() {
        assert_eq!(DYSON_CONFIGURE_HEADER, "X-Swarm-Configure");
        assert_eq!(DYSON_CSRF_HEADER, "X-Dyson-CSRF");
        assert_eq!(DYSON_INTERNAL_CSRF_VALUE, "swarm-internal");
        assert_eq!(DYSON_WEBHOOK_CSRF_VALUE, "swarm-webhook");
        assert_eq!(SWARM_LLM_AUDIT_ID_HEADER, "x-swarm-llm-audit-id");
        assert_eq!(DYSON_ADMIN_CONFIGURE_ENDPOINT, "configure");
        assert_eq!(DYSON_ADMIN_STATE_FILE_ENDPOINT, "state/file");
        assert_eq!(DYSON_ADMIN_SKILL_INSTALL_ENDPOINT, "skills/install");
    }

    #[test]
    fn swarm_internal_route_contract_constants_are_stable() {
        assert_eq!(
            SWARM_INTERNAL_ARTEFACT_INGEST_PATH,
            "/v1/internal/ingest/artefact"
        );
        assert_eq!(SWARM_INTERNAL_STATE_FILE_PATH, "/v1/internal/state/file");
        assert_eq!(SWARM_SHARE_MINT_PATH, "/_swarm/share-mint");
    }

    #[test]
    fn guest_security_contract_constants_are_stable() {
        use super::guest_security::*;
        assert_eq!((VSOCK_PORT, HOST_CID), (9473, 2));
        assert_eq!(TELEMETRY_TAG, "dyson_telemetry");
        assert_eq!(
            (EXEC_RULE, EXIT_RULE, CONNECT_RULE),
            ("dyson_exec", "dyson_exit", "dyson_connect")
        );
        assert_eq!(FIELD_PID_START, "dyson.pid.ts");
        assert_eq!(FIELD_ANCESTOR_START_PREFIX, "dyson.apid.ts[");
        assert_eq!(FIELD_ARGS_TRUNCATED, "dyson.args_truncated");
        assert_eq!(ENTITY_NAMESPACE, "guest");
        assert_eq!((MAX_ARGUMENTS, MAX_ARGUMENT_BYTES), (64, 4096));
        assert_eq!(SWARM_INGEST_PATH, "/v1/internal/ingest/security-events");
        assert_eq!(FINDING_KIND_FALCO, "falco");
    }

    #[test]
    fn falco_priorities_map_to_swarm_severities() {
        use super::guest_security::severity;
        for p in [
            "emergency",
            "alert",
            "critical",
            "error",
            "Critical",
            "ERROR",
        ] {
            assert_eq!(severity(p), Some("high"), "{p}");
        }
        assert_eq!(severity("warning"), Some("medium"));
        assert_eq!(severity("Notice"), Some("low"));
        assert_eq!(severity("informational"), Some("low"));
        assert_eq!(severity("debug"), None);
        assert_eq!(severity("Debug"), None);
        assert_eq!(severity("loud"), None);
    }
}
