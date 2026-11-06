//! The REST client: one method per Cloudflare call this crate makes.

use std::{sync::Arc, time::Duration};

use reqwest::{Method, StatusCode};
use serde_json::{Value, json};

use crate::{
    Changed, Error, ManagedHeader, NewRecord, Record, Result, Token, TokenStatus, Zone, path_part,
    same_rule,
};

/// Cloudflare's v4 API.
pub const DEFAULT_API: &str = "https://api.cloudflare.com/client/v4";
const TIMEOUT: Duration = Duration::from_secs(20);
/// Zones are listed this many to a page, up to this many pages.
const ZONES_PER_PAGE: u32 = 50;
const MAX_ZONE_PAGES: u32 = 20;
const RECORDS_PER_PAGE: u32 = 100;

/// A Cloudflare API client. Cheap to clone; every call takes the token it
/// acts with, so one client serves every key a product holds.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    api: Arc<str>,
}

fn crypto_provider() -> rustls::crypto::CryptoProvider {
    #[cfg(feature = "aws-lc-rs")]
    {
        rustls::crypto::aws_lc_rs::default_provider()
    }
    #[cfg(all(feature = "ring", not(feature = "aws-lc-rs")))]
    {
        rustls::crypto::ring::default_provider()
    }
    #[cfg(not(any(feature = "ring", feature = "aws-lc-rs")))]
    compile_error!("dyson-cloudflare needs the `ring` or `aws-lc-rs` feature");
}

fn tls() -> rustls::ClientConfig {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    rustls::ClientConfig::builder_with_provider(Arc::new(crypto_provider()))
        .with_safe_default_protocol_versions()
        .expect("the provider supports the default protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}

impl Client {
    /// Cloudflare's API, identifying as `user_agent`.
    pub fn new(user_agent: &str) -> Self {
        Self::with_endpoint(DEFAULT_API, user_agent)
    }

    /// Another API root: a local stand-in in tests. Products should only
    /// honour an override like this in debug builds.
    pub fn with_endpoint(api: &str, user_agent: &str) -> Self {
        let http = reqwest::Client::builder()
            .tls_backend_preconfigured(tls())
            .redirect(reqwest::redirect::Policy::none())
            .timeout(TIMEOUT)
            .user_agent(user_agent)
            .build()
            .expect("static reqwest configuration");
        Self {
            http,
            api: api.trim_end_matches('/').into(),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.api
    }

    /// One call. `NotFound` for a 404; `Api` for any other refusal,
    /// including a 2xx whose envelope says `success: false`.
    async fn call(
        &self,
        token: &Token,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&Value>,
    ) -> Result<Value> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.api))
            .bearer_auth(token.expose());
        if !query.is_empty() {
            request = request.query(query);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request
            .send()
            .await
            .map_err(|error| Error::Unreachable(without_url(&error)))?;
        let status = response.status();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        if status == StatusCode::NOT_FOUND {
            return Err(Error::NotFound);
        }
        if status.is_success() && body["success"].as_bool() != Some(false) {
            return Ok(body);
        }
        let errors = body["errors"].as_array().cloned().unwrap_or_default();
        Err(Error::Api {
            status: status.as_u16(),
            codes: errors.iter().filter_map(|e| e["code"].as_i64()).collect(),
            messages: errors
                .iter()
                .filter_map(|e| e["message"].as_str().map(str::to_owned))
                .collect(),
        })
    }

    async fn get(&self, token: &Token, path: &str, query: &[(&str, String)]) -> Result<Value> {
        self.call(token, Method::GET, path, query, None).await
    }

    // ------------------------------------------------------------- tokens

    /// Whether the token is active. With an account id, the account-owned
    /// token endpoint is asked first and the user-owned one after it, as a
    /// token can be either.
    pub async fn verify(&self, token: &Token, account_id: Option<&str>) -> Result<TokenStatus> {
        let mut paths = Vec::new();
        if let Some(account) = account_id {
            paths.push(format!(
                "/accounts/{}/tokens/verify",
                path_part("account", account)?
            ));
        }
        paths.push("/user/tokens/verify".to_owned());
        let mut last = Error::NotFound;
        for path in paths {
            match self.get(token, &path, &[]).await {
                Ok(body) => {
                    return match body.pointer("/result/status").and_then(Value::as_str) {
                        Some("active") => Ok(TokenStatus::Active),
                        Some(other) => Ok(TokenStatus::Other(other.to_owned())),
                        None => Err(Error::Malformed("token status missing".into())),
                    };
                }
                Err(error @ Error::Unreachable(_)) => return Err(error),
                Err(error) => last = error,
            }
        }
        Err(last)
    }

    // -------------------------------------------------------------- zones

    /// Every zone the token reaches, in Cloudflare's order.
    pub async fn zones(&self, token: &Token) -> Result<Vec<Zone>> {
        let mut zones = Vec::new();
        for page in 1..=MAX_ZONE_PAGES {
            let body = self
                .get(
                    token,
                    "/zones",
                    &[
                        ("per_page", ZONES_PER_PAGE.to_string()),
                        ("page", page.to_string()),
                    ],
                )
                .await?;
            let rows = body["result"].as_array().cloned().unwrap_or_default();
            for row in &rows {
                if let (Some(id), Some(name)) = (row["id"].as_str(), row["name"].as_str()) {
                    zones.push(Zone {
                        id: id.to_owned(),
                        name: name.to_ascii_lowercase(),
                        status: row["status"].as_str().unwrap_or("unknown").to_owned(),
                        account_id: row["account"]["id"].as_str().map(str::to_owned),
                        account_name: row["account"]["name"].as_str().map(str::to_owned),
                    });
                }
            }
            let pages = body["result_info"]["total_pages"].as_u64().unwrap_or(1);
            if rows.is_empty() || u64::from(page) >= pages {
                break;
            }
        }
        Ok(zones)
    }

    // ------------------------------------------------------------ records

    /// The records at exactly `name` in the zone, of every type.
    pub async fn records(&self, token: &Token, zone_id: &str, name: &str) -> Result<Vec<Record>> {
        let zone_id = path_part("zone", zone_id)?;
        let body = self
            .get(
                token,
                &format!("/zones/{zone_id}/dns_records"),
                &[
                    ("name", name.to_owned()),
                    ("per_page", RECORDS_PER_PAGE.to_string()),
                ],
            )
            .await?;
        Ok(body["result"]
            .as_array()
            .map(|rows| rows.iter().filter_map(record).collect())
            .unwrap_or_default())
    }

    /// `Taken` when the name has any record already, naming the first.
    pub async fn ensure_free(&self, token: &Token, zone_id: &str, name: &str) -> Result<()> {
        let there = self.records(token, zone_id, name).await?;
        match there.first() {
            None => Ok(()),
            Some(record) => Err(Error::Taken(format!(
                "{name} already has {} record{} at Cloudflare ({} {}); pick another name",
                there.len(),
                if there.len() == 1 { "" } else { "s" },
                record.kind,
                record.content
            ))),
        }
    }

    pub async fn create_record(
        &self,
        token: &Token,
        zone_id: &str,
        record: &NewRecord,
    ) -> Result<Record> {
        let zone_id = path_part("zone", zone_id)?;
        let body = self
            .call(
                token,
                Method::POST,
                &format!("/zones/{zone_id}/dns_records"),
                &[],
                Some(&serde_json::to_value(record).expect("a record serializes")),
            )
            .await?;
        made(&body, record)
    }

    /// Replace a record's type, name, content, proxying and TTL.
    pub async fn update_record(
        &self,
        token: &Token,
        zone_id: &str,
        record_id: &str,
        record: &NewRecord,
    ) -> Result<Record> {
        let zone_id = path_part("zone", zone_id)?;
        let record_id = path_part("record", record_id)?;
        let body = self
            .call(
                token,
                Method::PUT,
                &format!("/zones/{zone_id}/dns_records/{record_id}"),
                &[],
                Some(&serde_json::to_value(record).expect("a record serializes")),
            )
            .await?;
        made(&body, record)
    }

    pub async fn delete_record(&self, token: &Token, zone_id: &str, record_id: &str) -> Result<()> {
        let zone_id = path_part("zone", zone_id)?;
        let record_id = path_part("record", record_id)?;
        self.call(
            token,
            Method::DELETE,
            &format!("/zones/{zone_id}/dns_records/{record_id}"),
            &[],
            None,
        )
        .await
        .map(|_| ())
    }

    // -------------------------------------------------------------- rules

    /// Bring the rule with `rule_ref` in the zone's `phase` entry point to
    /// `wanted`, or remove it with `None`. Every other rule in the phase is
    /// kept as it is. Nothing is written when the stored rule already says
    /// the same (see [`crate::same_rule`]). A missing entry point counts as
    /// empty; the write creates it.
    pub async fn ensure_rule(
        &self,
        token: &Token,
        zone_id: &str,
        phase: &str,
        rule_ref: &str,
        wanted: Option<Value>,
    ) -> Result<Changed> {
        let zone_id = path_part("zone", zone_id)?;
        let phase = path_part("phase", phase)?;
        if let Some(rule) = &wanted
            && rule["ref"].as_str() != Some(rule_ref)
        {
            return Err(Error::Invalid(format!("the rule's ref must be {rule_ref}")));
        }
        let path = format!("/zones/{zone_id}/rulesets/phases/{phase}/entrypoint");
        let rules: Vec<Value> = match self.get(token, &path, &[]).await {
            Ok(body) => body["result"]["rules"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            Err(Error::NotFound) => Vec::new(),
            Err(error) => return Err(error),
        };
        let have = rules
            .iter()
            .find(|rule| rule["ref"].as_str() == Some(rule_ref));
        match (&wanted, have) {
            (None, None) => return Ok(false),
            (Some(wanted), Some(have)) if same_rule(have, wanted) => return Ok(false),
            _ => {}
        }
        let mut next: Vec<Value> = rules
            .into_iter()
            .filter(|rule| rule["ref"].as_str() != Some(rule_ref))
            .map(|mut rule| {
                // What Cloudflare writes itself is refused coming back.
                if let Some(fields) = rule.as_object_mut() {
                    fields.remove("version");
                    fields.remove("last_updated");
                }
                rule
            })
            .collect();
        next.extend(wanted);
        self.call(
            token,
            Method::PUT,
            &path,
            &[],
            Some(&json!({ "rules": next })),
        )
        .await?;
        Ok(true)
    }

    // ------------------------------------------------------ zone settings

    /// The zone's managed request headers.
    pub async fn managed_headers(
        &self,
        token: &Token,
        zone_id: &str,
    ) -> Result<Vec<ManagedHeader>> {
        let zone_id = path_part("zone", zone_id)?;
        let body = self
            .get(token, &format!("/zones/{zone_id}/managed_headers"), &[])
            .await?;
        Ok(body["result"]["managed_request_headers"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        Some(ManagedHeader {
                            id: row["id"].as_str()?.to_owned(),
                            enabled: row["enabled"].as_bool().unwrap_or(false),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Turn on one managed request header (for example
    /// `add_visitor_location_headers`), unless it is on already.
    pub async fn enable_managed_header(
        &self,
        token: &Token,
        zone_id: &str,
        header_id: &str,
    ) -> Result<Changed> {
        let on = self
            .managed_headers(token, zone_id)
            .await?
            .iter()
            .any(|header| header.id == header_id && header.enabled);
        if on {
            return Ok(false);
        }
        let zone_id = path_part("zone", zone_id)?;
        let header_id = path_part("header", header_id)?;
        self.call(
            token,
            Method::PATCH,
            &format!("/zones/{zone_id}/managed_headers"),
            &[],
            Some(&json!({
                "managed_request_headers": [{"id": header_id, "enabled": true}],
                "managed_response_headers": [],
            })),
        )
        .await?;
        Ok(true)
    }

    /// Whether Bot Fight Mode is on for the zone; `None` when Cloudflare
    /// does not say.
    pub async fn bot_fight_mode(&self, token: &Token, zone_id: &str) -> Result<Option<bool>> {
        let zone_id = path_part("zone", zone_id)?;
        match self
            .get(token, &format!("/zones/{zone_id}/bot_management"), &[])
            .await
        {
            Ok(body) => Ok(body["result"]["fight_mode"].as_bool()),
            Err(Error::NotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

fn record(row: &Value) -> Option<Record> {
    Some(Record {
        id: row["id"].as_str()?.to_owned(),
        kind: row["type"].as_str()?.to_owned(),
        name: row["name"]
            .as_str()
            .unwrap_or_default()
            .to_ascii_lowercase(),
        content: row["content"].as_str().unwrap_or_default().to_owned(),
        proxied: row["proxied"].as_bool().unwrap_or(false),
        ttl: row["ttl"]
            .as_u64()
            .and_then(|ttl| u32::try_from(ttl).ok())
            .unwrap_or(1),
    })
}

/// The record Cloudflare says it made, falling back to what was asked for
/// where the answer leaves a field out.
fn made(body: &Value, asked: &NewRecord) -> Result<Record> {
    let id = body["result"]["id"]
        .as_str()
        .ok_or_else(|| Error::Malformed("the record was made without an id".into()))?;
    let mut out = record(&body["result"]).unwrap_or(Record {
        id: id.to_owned(),
        kind: asked.kind.as_str().to_owned(),
        name: asked.name.clone(),
        content: asked.content.clone(),
        proxied: asked.proxied,
        ttl: asked.ttl,
    });
    if out.name.is_empty() {
        out.name = asked.name.clone();
    }
    if out.content.is_empty() {
        out.content = asked.content.clone();
    }
    Ok(out)
}

/// A transport error without its URL: query strings may carry names the
/// product would rather not log, and the host is always the API's.
fn without_url(error: &reqwest::Error) -> String {
    let kind = if error.is_timeout() {
        "timed out"
    } else if error.is_connect() {
        "could not connect"
    } else {
        "request failed"
    };
    let mut source = std::error::Error::source(error);
    let mut detail = String::new();
    while let Some(cause) = source {
        detail = cause.to_string();
        source = cause.source();
    }
    if detail.is_empty() {
        kind.to_owned()
    } else {
        format!("{kind}: {detail}")
    }
}
