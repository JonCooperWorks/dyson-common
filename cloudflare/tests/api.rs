//! The client against a stand-in for Cloudflare's API: paging, the error
//! envelope, 404s, rules kept by `ref` without disturbing their neighbours,
//! and reconciliation that never touches records it does not own.

use std::{
    collections::BTreeMap,
    net::IpAddr,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, put},
};
use dyson_cloudflare::{
    Client, Error, NewRecord, Token, TokenStatus,
    reconcile::{self, Owned},
};
use serde_json::{Value, json};

const TOKEN: &str = "cf-test-token-0001";
const ZONE: &str = "z-home";
const HOST: &str = "api.home.example.net";

#[derive(Default)]
struct Fake {
    /// zone id -> records
    records: BTreeMap<String, Vec<Value>>,
    /// (zone id, phase) -> rules
    rules: BTreeMap<(String, String), Vec<Value>>,
    location_headers: bool,
    next: usize,
    /// Every write, in order.
    writes: Vec<String>,
}

type Shared = Arc<Mutex<Fake>>;
type Answer = (StatusCode, Json<Value>);

fn refused(headers: &HeaderMap) -> Option<Answer> {
    (headers.get("authorization").and_then(|v| v.to_str().ok())
        != Some(&format!("Bearer {TOKEN}")))
    .then(|| {
        (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "errors": [{"code": 9109, "message": "Invalid access token"}]})),
        )
    })
}

fn ok(result: Value) -> Answer {
    (
        StatusCode::OK,
        Json(json!({"success": true, "errors": [], "result": result})),
    )
}

fn missing() -> Answer {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"success": false, "errors": [{"code": 7003, "message": "Could not route"}]})),
    )
}

async fn zones(Query(query): Query<BTreeMap<String, String>>, headers: HeaderMap) -> Answer {
    if let Some(no) = refused(&headers) {
        return no;
    }
    let page: u32 = query.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
    assert_eq!(query.get("per_page").map(String::as_str), Some("50"));
    let result = match page {
        1 => json!([
            {"id": "z-home", "name": "Home.example.net", "status": "active", "account": {"id": "acc1", "name": "Lab"}},
        ]),
        2 => json!([
            {"id": "z-bait", "name": "bait.example.org", "status": "pending", "account": {"name": "Lab"}},
        ]),
        _ => json!([]),
    };
    (
        StatusCode::OK,
        Json(json!({
            "success": true, "errors": [], "result": result,
            "result_info": {"page": page, "total_pages": 2},
        })),
    )
}

async fn user_verify(headers: HeaderMap) -> Answer {
    if let Some(no) = refused(&headers) {
        return no;
    }
    ok(json!({"id": "tok", "status": "active"}))
}

/// An account that does not own the token, as Cloudflare answers it.
async fn account_verify(Path(_account): Path<String>) -> Answer {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"success": false, "errors": [{"code": 1000, "message": "Invalid API Token"}]})),
    )
}

async fn records_list(
    State(fake): State<Shared>,
    Path(zone): Path<String>,
    Query(query): Query<BTreeMap<String, String>>,
    headers: HeaderMap,
) -> Answer {
    if let Some(no) = refused(&headers) {
        return no;
    }
    let fake = fake.lock().unwrap();
    let Some(rows) = fake.records.get(&zone) else {
        return missing();
    };
    let name = query.get("name").cloned().unwrap_or_default();
    ok(json!(
        rows.iter()
            .filter(|row| row["name"] == name)
            .cloned()
            .collect::<Vec<_>>()
    ))
}

async fn records_create(
    State(fake): State<Shared>,
    Path(zone): Path<String>,
    Json(mut body): Json<Value>,
) -> Answer {
    let mut fake = fake.lock().unwrap();
    let Some(rows) = fake.records.get(&zone) else {
        return missing();
    };
    // As Cloudflare does: an identical record is refused.
    if rows.iter().any(|row| {
        row["name"] == body["name"]
            && row["type"] == body["type"]
            && row["content"] == body["content"]
    }) {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({"success": false, "errors": [{"code": 81058, "message": "An identical record already exists."}]}),
            ),
        );
    }
    fake.next += 1;
    body["id"] = json!(format!("rec-{}", fake.next));
    fake.writes.push(format!(
        "create {} {} {}",
        body["type"].as_str().unwrap(),
        body["content"].as_str().unwrap(),
        body["proxied"]
    ));
    fake.records.get_mut(&zone).unwrap().push(body.clone());
    ok(body)
}

async fn records_put(
    State(fake): State<Shared>,
    Path((zone, id)): Path<(String, String)>,
    Json(mut body): Json<Value>,
) -> Answer {
    let mut fake = fake.lock().unwrap();
    fake.writes
        .push(format!("update {id} {}", body["content"].as_str().unwrap()));
    let Some(row) = fake
        .records
        .get_mut(&zone)
        .and_then(|rows| rows.iter_mut().find(|row| row["id"] == id.as_str()))
    else {
        return missing();
    };
    body["id"] = json!(id);
    *row = body.clone();
    ok(body)
}

async fn records_delete(
    State(fake): State<Shared>,
    Path((zone, id)): Path<(String, String)>,
) -> Answer {
    let mut fake = fake.lock().unwrap();
    fake.writes.push(format!("delete {id}"));
    if let Some(rows) = fake.records.get_mut(&zone) {
        rows.retain(|row| row["id"] != id.as_str());
    }
    ok(json!({"id": id}))
}

async fn ruleset_get(
    State(fake): State<Shared>,
    Path((zone, phase)): Path<(String, String)>,
) -> Answer {
    let fake = fake.lock().unwrap();
    match fake.rules.get(&(zone, phase)) {
        Some(rules) => ok(json!({"rules": rules})),
        None => missing(),
    }
}

async fn ruleset_put(
    State(fake): State<Shared>,
    Path((zone, phase)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Answer {
    let mut fake = fake.lock().unwrap();
    let rules = body["rules"].as_array().cloned().unwrap_or_default();
    // As Cloudflare does: what it writes itself is refused coming back.
    if rules
        .iter()
        .any(|rule| rule.get("version").is_some() || rule.get("last_updated").is_some())
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "errors": [{"message": "unknown field"}]})),
        );
    }
    let stored: Vec<Value> = rules
        .into_iter()
        .map(|mut rule| {
            rule["version"] = json!("1");
            rule["last_updated"] = json!("2026-11-06T00:00:00Z");
            rule
        })
        .collect();
    fake.writes.push(format!("rules {phase}"));
    fake.rules.insert((zone, phase), stored.clone());
    ok(json!({"rules": stored}))
}

async fn managed_get(State(fake): State<Shared>) -> Answer {
    let on = fake.lock().unwrap().location_headers;
    ok(json!({"managed_request_headers": [{"id": "add_visitor_location_headers", "enabled": on}]}))
}

async fn managed_patch(State(fake): State<Shared>, Json(body): Json<Value>) -> Answer {
    let mut fake = fake.lock().unwrap();
    fake.location_headers = body["managed_request_headers"][0]["enabled"] == true;
    fake.writes.push("location headers".into());
    ok(body)
}

async fn bots() -> Answer {
    ok(json!({"fight_mode": true}))
}

/// A 200 whose envelope says it failed.
async fn quiet_failure() -> Answer {
    (
        StatusCode::OK,
        Json(
            json!({"success": false, "errors": [{"code": 10000, "message": "Authentication error"}]}),
        ),
    )
}

async fn cloudflare() -> (Client, Shared) {
    let fake = Shared::default();
    fake.lock().unwrap().records.insert(ZONE.into(), Vec::new());
    let app = Router::new()
        .route("/zones", get(zones))
        .route("/user/tokens/verify", get(user_verify))
        .route("/accounts/{account}/tokens/verify", get(account_verify))
        .route(
            "/zones/{zone}/dns_records",
            get(records_list).post(records_create),
        )
        .route(
            "/zones/{zone}/dns_records/{id}",
            put(records_put).delete(records_delete),
        )
        .route(
            "/zones/{zone}/rulesets/phases/{phase}/entrypoint",
            get(ruleset_get).put(ruleset_put),
        )
        .route(
            "/zones/{zone}/managed_headers",
            get(managed_get).patch(managed_patch),
        )
        .route("/zones/{zone}/bot_management", get(bots))
        .route("/quiet/zones", get(quiet_failure))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (Client::with_endpoint(&base, "dyson-cloudflare-test"), fake)
}

fn token() -> Token {
    Token::new(TOKEN).unwrap()
}

fn seed(fake: &Shared, rows: Vec<Value>) {
    fake.lock().unwrap().records.insert(ZONE.into(), rows);
}

fn writes(fake: &Shared) -> Vec<String> {
    std::mem::take(&mut fake.lock().unwrap().writes)
}

fn contents(fake: &Shared) -> Vec<String> {
    let fake = fake.lock().unwrap();
    let mut out: Vec<String> = fake.records[ZONE]
        .iter()
        .filter(|row| row["name"] == HOST)
        .map(|row| {
            format!(
                "{} {}",
                row["type"].as_str().unwrap(),
                row["content"].as_str().unwrap()
            )
        })
        .collect();
    out.sort();
    out
}

fn ips(list: &[&str]) -> Vec<IpAddr> {
    list.iter().map(|ip| ip.parse().unwrap()).collect()
}

#[tokio::test]
async fn zones_are_read_across_pages_and_a_bad_token_is_refused() {
    let (client, _) = cloudflare().await;
    let zones = client.zones(&token()).await.unwrap();
    assert_eq!(
        zones.iter().map(|z| z.name.as_str()).collect::<Vec<_>>(),
        ["home.example.net", "bait.example.org"]
    );
    assert_eq!(zones[0].account_id.as_deref(), Some("acc1"));
    assert_eq!(zones[1].status, "pending");

    let wrong = Token::new("cf-wrong-token").unwrap();
    match client.zones(&wrong).await {
        Err(Error::Api {
            status,
            codes,
            messages,
        }) => {
            assert_eq!(status, 403);
            assert_eq!(codes, [9109]);
            assert_eq!(messages, ["Invalid access token"]);
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn verify_falls_back_from_the_account_to_the_user_endpoint() {
    let (client, _) = cloudflare().await;
    assert_eq!(
        client.verify(&token(), Some("acc1")).await.unwrap(),
        TokenStatus::Active
    );
    assert_eq!(
        client.verify(&token(), None).await.unwrap(),
        TokenStatus::Active
    );
    let wrong = Token::new("cf-wrong-token").unwrap();
    assert!(matches!(
        client.verify(&wrong, Some("acc1")).await,
        Err(Error::Api { status: 403, .. })
    ));
    assert!(matches!(
        client.verify(&token(), Some("../x")).await,
        Err(Error::Invalid(_))
    ));
}

#[tokio::test]
async fn a_404_is_not_found_and_a_quiet_failure_is_an_error() {
    let (client, _) = cloudflare().await;
    assert!(matches!(
        client.records(&token(), "z-nowhere", HOST).await,
        Err(Error::NotFound)
    ));
    assert!(matches!(
        client
            .create_record(
                &token(),
                "z-nowhere",
                &NewRecord::cname(HOST, "x.example", true)
            )
            .await,
        Err(Error::NotFound)
    ));
    // A 200 with `success: false` is still refused.
    let quiet = Client::with_endpoint(&format!("{}/quiet", client.endpoint()), "t");
    assert!(matches!(
        quiet.zones(&token()).await,
        Err(Error::Api { status: 200, .. })
    ));
}

#[tokio::test]
async fn a_name_with_records_is_taken() {
    let (client, fake) = cloudflare().await;
    client.ensure_free(&token(), ZONE, HOST).await.unwrap();
    seed(
        &fake,
        vec![
            json!({"id": "theirs", "type": "TXT", "name": HOST, "content": "hello", "proxied": false}),
        ],
    );
    match client.ensure_free(&token(), ZONE, HOST).await {
        Err(Error::Taken(why)) => assert_eq!(
            why,
            "api.home.example.net already has 1 record at Cloudflare (TXT hello); pick another name"
        ),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_rule_is_kept_by_ref_without_disturbing_its_neighbours() {
    let (client, fake) = cloudflare().await;
    let phase = "http_config_settings";
    fake.lock().unwrap().rules.insert(
        (ZONE.into(), phase.into()),
        vec![
            json!({"id": "x", "ref": "theirs", "expression": "true", "action": "set_config",
                    "action_parameters": {"ssl": "strict"}, "enabled": true, "version": "4"}),
        ],
    );
    let rule = json!({
        "ref": "ours", "description": "ours",
        "expression": "(http.host in {\"a.example.net\"})",
        "action": "set_config", "action_parameters": {"ssl": "full"}, "enabled": true,
    });
    assert!(
        client
            .ensure_rule(&token(), ZONE, phase, "ours", Some(rule.clone()))
            .await
            .unwrap()
    );
    // Asked again, nothing is written.
    assert!(
        !client
            .ensure_rule(&token(), ZONE, phase, "ours", Some(rule.clone()))
            .await
            .unwrap()
    );
    assert_eq!(writes(&fake), [format!("rules {phase}")]);
    {
        let fake = fake.lock().unwrap();
        let rules = &fake.rules[&(ZONE.to_owned(), phase.to_owned())];
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0]["ref"], "theirs");
        assert_eq!(rules[0]["action_parameters"]["ssl"], "strict");
    }
    // Removed: only ours goes.
    assert!(
        client
            .ensure_rule(&token(), ZONE, phase, "ours", None)
            .await
            .unwrap()
    );
    assert!(
        !client
            .ensure_rule(&token(), ZONE, phase, "ours", None)
            .await
            .unwrap()
    );
    let fake_rules = fake.lock().unwrap().rules[&(ZONE.to_owned(), phase.to_owned())].clone();
    assert_eq!(fake_rules.len(), 1);
    assert_eq!(fake_rules[0]["ref"], "theirs");

    // A missing entry point is created by the first write.
    assert!(
        client
            .ensure_rule(
                &token(),
                ZONE,
                "http_request_late_transform",
                "ours",
                Some(rule.clone())
            )
            .await
            .unwrap()
    );
    // A rule whose ref disagrees is refused before Cloudflare is asked.
    assert!(matches!(
        client
            .ensure_rule(&token(), ZONE, phase, "other", Some(rule))
            .await,
        Err(Error::Invalid(_))
    ));
}

#[tokio::test]
async fn zone_settings_are_read_and_a_header_is_turned_on_once() {
    let (client, fake) = cloudflare().await;
    let id = "add_visitor_location_headers";
    assert!(
        client
            .enable_managed_header(&token(), ZONE, id)
            .await
            .unwrap()
    );
    assert!(
        !client
            .enable_managed_header(&token(), ZONE, id)
            .await
            .unwrap()
    );
    assert_eq!(writes(&fake), ["location headers"]);
    assert!(client.managed_headers(&token(), ZONE).await.unwrap()[0].enabled);
    assert_eq!(
        client.bot_fight_mode(&token(), ZONE).await.unwrap(),
        Some(true)
    );
}

#[tokio::test]
async fn addresses_follow_the_origin_without_leaving_the_name_unanswered() {
    let (client, fake) = cloudflare().await;
    let t = token();

    let made = reconcile::addresses(
        &client,
        &t,
        ZONE,
        HOST,
        &ips(&["8.8.8.8", "2001:4860::8888"]),
        true,
        Owned::All,
    )
    .await
    .unwrap();
    assert!(made.changed);
    assert_eq!(made.records.len(), 2);
    assert_eq!(contents(&fake), ["A 8.8.8.8", "AAAA 2001:4860::8888"]);
    writes(&fake);

    // Same again: nothing written.
    let again = reconcile::addresses(
        &client,
        &t,
        ZONE,
        HOST,
        &ips(&["2001:4860::8888", "8.8.8.8"]),
        true,
        Owned::All,
    )
    .await
    .unwrap();
    assert!(!again.changed);
    assert!(writes(&fake).is_empty());

    // The origin moves: the new record is made before the old one goes.
    let moved = reconcile::addresses(
        &client,
        &t,
        ZONE,
        HOST,
        &ips(&["9.9.9.9"]),
        true,
        Owned::All,
    )
    .await
    .unwrap();
    assert!(moved.changed);
    let log = writes(&fake);
    assert!(log[0].starts_with("create A 9.9.9.9"), "{log:?}");
    assert!(log[1..].iter().all(|w| w.starts_with("delete")), "{log:?}");
    assert_eq!(contents(&fake), ["A 9.9.9.9"]);

    // Same address, other proxying: the identical record must go first.
    reconcile::addresses(
        &client,
        &t,
        ZONE,
        HOST,
        &ips(&["9.9.9.9"]),
        false,
        Owned::All,
    )
    .await
    .unwrap();
    let log = writes(&fake);
    assert!(log[0].starts_with("delete"), "{log:?}");
    assert_eq!(log[1], "create A 9.9.9.9 false");

    // Nothing wanted: everything owned goes.
    reconcile::addresses(&client, &t, ZONE, HOST, &[], false, Owned::All)
        .await
        .unwrap();
    assert!(contents(&fake).is_empty());
}

#[tokio::test]
async fn records_a_product_does_not_own_are_never_touched() {
    let (client, fake) = cloudflare().await;
    let t = token();
    seed(
        &fake,
        vec![
            json!({"id": "theirs", "type": "A", "name": HOST, "content": "1.1.1.1", "proxied": false}),
            json!({"id": "ours-old", "type": "A", "name": HOST, "content": "8.8.4.4", "proxied": true}),
            json!({"id": "txt", "type": "TXT", "name": HOST, "content": "v=spf1", "proxied": false}),
        ],
    );
    let owned = vec!["ours-old".to_owned()];
    let out = reconcile::addresses(
        &client,
        &t,
        ZONE,
        HOST,
        &ips(&["8.8.8.8"]),
        true,
        Owned::Ids(&owned),
    )
    .await
    .unwrap();
    assert!(out.changed);
    assert_eq!(out.records.len(), 1);
    assert_eq!(out.records[0].content, "8.8.8.8");
    let log = writes(&fake);
    assert!(
        !log.iter()
            .any(|w| w == "delete theirs" || w == "delete txt"),
        "{log:?}"
    );
    assert!(log.iter().any(|w| w == "delete ours-old"), "{log:?}");
    assert_eq!(contents(&fake), ["A 1.1.1.1", "A 8.8.8.8", "TXT v=spf1"]);

    // With every record owned, the stranger's A record goes too.
    reconcile::addresses(
        &client,
        &t,
        ZONE,
        HOST,
        &ips(&["8.8.8.8"]),
        true,
        Owned::All,
    )
    .await
    .unwrap();
    assert_eq!(contents(&fake), ["A 8.8.8.8", "TXT v=spf1"]);
}

#[tokio::test]
async fn a_cname_is_created_kept_and_changed_in_place() {
    let (client, fake) = cloudflare().await;
    let t = token();
    let none: Vec<String> = Vec::new();

    let made = reconcile::cname(
        &client,
        &t,
        ZONE,
        HOST,
        "Swarm.Example.com.",
        true,
        Owned::Ids(&none),
    )
    .await
    .unwrap();
    assert!(made.changed);
    assert_eq!(made.records[0].content, "swarm.example.com");
    let ours = vec![made.records[0].id.clone()];
    writes(&fake);

    let kept = reconcile::cname(
        &client,
        &t,
        ZONE,
        HOST,
        "swarm.example.com",
        true,
        Owned::Ids(&ours),
    )
    .await
    .unwrap();
    assert!(!kept.changed);
    assert!(writes(&fake).is_empty());

    let moved = reconcile::cname(
        &client,
        &t,
        ZONE,
        HOST,
        "edge.example.com",
        true,
        Owned::Ids(&ours),
    )
    .await
    .unwrap();
    assert!(moved.changed);
    assert_eq!(moved.records[0].id, ours[0], "changed in place");
    assert_eq!(
        writes(&fake),
        [format!("update {} edge.example.com", ours[0])]
    );
    assert_eq!(contents(&fake), ["CNAME edge.example.com"]);

    // A CNAME we do not own is left alone; making ours then fails at
    // Cloudflare rather than replacing theirs.
    seed(
        &fake,
        vec![
            json!({"id": "theirs", "type": "CNAME", "name": HOST, "content": "edge.example.com", "proxied": true}),
        ],
    );
    let err = reconcile::cname(
        &client,
        &t,
        ZONE,
        HOST,
        "edge.example.com",
        true,
        Owned::Ids(&none),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, Error::Api { status: 400, .. }), "{err:?}");
    assert_eq!(contents(&fake), ["CNAME edge.example.com"]);
}

/// Real TLS to api.cloudflare.com with the bundled roots: a made-up token
/// must come back as Cloudflare's refusal, not a transport error. Run with
/// `cargo test -p dyson-cloudflare -- --ignored`.
#[tokio::test]
#[ignore = "talks to api.cloudflare.com"]
async fn the_real_api_answers_over_tls() {
    let client = Client::new("dyson-cloudflare-test");
    let fake = Token::new("not-a-real-cloudflare-token-0000000000").unwrap();
    match client.zones(&fake).await {
        Err(Error::Api { status, .. }) => assert!(matches!(status, 400 | 401 | 403), "{status}"),
        other => panic!("{other:?}"),
    }
}
