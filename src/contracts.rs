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
}
