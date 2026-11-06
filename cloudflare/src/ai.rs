//! Workers AI account URLs: one parser for every form the dashboard shows,
//! so Swarm and the honeypot agree on the account, gateway and catalogue.
//! Pure functions; products send the requests with their own clients.
//!
//! An account is registered as
//! `https://api.cloudflare.com/client/v4/accounts/<id>`, optionally followed
//! by `/ai`, `/ai/run` or `/ai/v1`, all naming the same account. An optional
//! `?gateway=<name>` picks the AI Gateway (default: `default`).

const API_HOST: &str = "api.cloudflare.com";
pub const DEFAULT_GATEWAY: &str = "default";
/// Picks the AI Gateway a request goes through.
pub const GATEWAY_HEADER: &str = "cf-aig-gateway-id";
/// JSON object of attribution that AI Gateway logs and spend rules key on.
pub const METADATA_HEADER: &str = "cf-aig-metadata";
/// Verifies a user-owned token (an account-owned one: [`Account::token_verify_url`]).
pub const USER_TOKEN_VERIFY_URL: &str = "https://api.cloudflare.com/client/v4/user/tokens/verify";

/// A Workers AI account, from any of its registered URL forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// The account id.
    pub id: String,
    /// `https://api.cloudflare.com/client/v4/accounts/<id>`.
    pub base: String,
    /// The AI Gateway name.
    pub gateway: String,
}

impl Account {
    /// Recognise a Cloudflare account URL; `None` for anything else.
    pub fn parse(url: &str) -> Option<Self> {
        let parsed = url::Url::parse(url.trim()).ok()?;
        if parsed.scheme() != "https" || parsed.host_str()? != API_HOST {
            return None;
        }
        let segments: Vec<&str> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
        let ["client", "v4", "accounts", id, rest @ ..] = segments.as_slice() else {
            return None;
        };
        let valid_id = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric());
        let valid_rest = matches!(rest, [] | ["ai"] | ["ai", "run"] | ["ai", "v1"]);
        if !valid_id || !valid_rest {
            return None;
        }
        let gateway = parsed
            .query_pairs()
            .find(|(k, _)| k == "gateway")
            .map(|(_, v)| v.into_owned())
            .filter(|v| {
                !v.is_empty()
                    && v.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            })
            .unwrap_or_else(|| DEFAULT_GATEWAY.to_owned());
        Some(Self {
            id: (*id).to_owned(),
            base: format!("https://{API_HOST}/client/v4/accounts/{id}"),
            gateway,
        })
    }

    /// Where a proxied request goes: `v1/...` to the OpenAI-compatible
    /// `/ai/v1/...` paths, anything else under `/ai/`.
    pub fn request_url(&self, rest: &str, query: Option<&str>) -> String {
        let path = match rest.strip_prefix("v1/") {
            Some(tail) => format!("{}/ai/v1/{tail}", self.base),
            None => format!("{}/ai/{rest}", self.base),
        };
        match query {
            Some(query) => format!("{path}?{query}"),
            None => path,
        }
    }

    /// The OpenRouter-shaped model catalogue, one page of it.
    pub fn catalog_url(&self, page: u32, per_page: u32) -> String {
        format!(
            "{}/ai/models/search?format=openrouter&hide_experimental=true&per_page={per_page}&page={page}",
            self.base
        )
    }

    /// Verifies an account-owned token.
    pub fn token_verify_url(&self) -> String {
        format!("{}/tokens/verify", self.base)
    }

    /// Unified Billing credits, in cents (the token needs AI Gateway Read).
    pub fn credit_balance_url(&self) -> String {
        format!("{}/ai-gateway/billing/credit-balance", self.base)
    }
}

/// The [`METADATA_HEADER`] value for these attribution pairs, leaving out
/// empty values; `None` when nothing is left.
pub fn metadata(pairs: &[(&str, &str)]) -> Option<String> {
    let map: serde_json::Map<String, serde_json::Value> = pairs
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| ((*key).to_owned(), (*value).into()))
        .collect();
    (!map.is_empty()).then(|| serde_json::Value::Object(map).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT: &str =
        "https://api.cloudflare.com/client/v4/accounts/cbf3fc52da36e389943cf1c80f0d0d29";

    #[test]
    fn every_dashboard_form_resolves_to_the_account() {
        for suffix in ["", "/", "/ai", "/ai/run", "/ai/v1", "/ai/v1/"] {
            let account = Account::parse(&format!("{ACCOUNT}{suffix}")).expect(suffix);
            assert_eq!(account.base, ACCOUNT);
            assert_eq!(account.id, "cbf3fc52da36e389943cf1c80f0d0d29");
            assert_eq!(account.gateway, DEFAULT_GATEWAY);
            assert_eq!(
                account.request_url("v1/chat/completions", None),
                format!("{ACCOUNT}/ai/v1/chat/completions")
            );
        }
    }

    #[test]
    fn non_cloudflare_or_other_paths_are_not_claimed() {
        assert!(Account::parse("https://openrouter.ai/api").is_none());
        assert!(Account::parse(&format!("{ACCOUNT}/dns_records")).is_none());
        assert!(Account::parse("http://api.cloudflare.com/client/v4/accounts/x/ai").is_none());
        assert!(Account::parse("https://api.cloudflare.com.example/ai/v1").is_none());
        assert!(Account::parse("https://api.cloudflare.com/client/v4/accounts/a-b/ai").is_none());
    }

    #[test]
    fn the_gateway_comes_from_the_query_when_it_is_a_plain_name() {
        let account = Account::parse(&format!("{ACCOUNT}/ai/v1?gateway=swarm")).unwrap();
        assert_eq!(account.gateway, "swarm");
        let account = Account::parse(&format!("{ACCOUNT}/ai/v1?gateway=a%20b")).unwrap();
        assert_eq!(account.gateway, DEFAULT_GATEWAY);
    }

    #[test]
    fn urls_match_what_swarm_and_the_honeypot_call() {
        let account = Account::parse(&format!("{ACCOUNT}/ai/v1")).unwrap();
        assert_eq!(
            account.catalog_url(2, 200),
            format!(
                "{ACCOUNT}/ai/models/search?format=openrouter&hide_experimental=true&per_page=200&page=2"
            )
        );
        assert_eq!(
            account.token_verify_url(),
            format!("{ACCOUNT}/tokens/verify")
        );
        assert_eq!(
            account.credit_balance_url(),
            format!("{ACCOUNT}/ai-gateway/billing/credit-balance")
        );
        assert_eq!(
            account.request_url("run/@cf/meta/llama", Some("a=1")),
            format!("{ACCOUNT}/ai/run/@cf/meta/llama?a=1")
        );
    }

    #[test]
    fn metadata_leaves_out_empty_values() {
        let value = metadata(&[("swarm_owner", "u1"), ("swarm_instance", "")]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&value).unwrap();
        assert_eq!(parsed, serde_json::json!({"swarm_owner": "u1"}));
        assert!(metadata(&[("swarm_owner", "")]).is_none());
    }
}
