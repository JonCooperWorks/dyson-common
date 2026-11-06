//! The extracted runner against a Kubernetes stand-in, independently of any
//! product database, scheduler, credentials or environment variables.
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use dyson_services::{Cluster, Desired, Placement, Readiness, Spec, Volume, Workload, objects};
use futures::StreamExt;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// What the stand-in holds: applied objects by kind and name, and every
/// write in order.
#[derive(Default)]
struct Fake {
    objects: BTreeMap<(String, String), Value>,
    writes: Vec<String>,
    selectors: Vec<String>,
}

type Shared = Arc<Mutex<Fake>>;

async fn apply(
    State(fake): State<Shared>,
    Path((kind, name)): Path<(String, String)>,
    body: String,
) -> Json<Value> {
    // Server-side apply sends the object as JSON (which is YAML).
    let object: Value = serde_json::from_str(&body).expect("an applied object is JSON");
    let mut fake = fake.lock().unwrap();
    fake.writes.push(format!("apply {kind}/{name}"));
    fake.objects.insert((kind, name), object.clone());
    Json(object)
}

async fn fetch(
    State(fake): State<Shared>,
    Path((kind, name)): Path<(String, String)>,
) -> (StatusCode, Json<Value>) {
    match fake.lock().unwrap().objects.get(&(kind, name.clone())) {
        Some(object) => (StatusCode::OK, Json(object.clone())),
        None => (
            StatusCode::NOT_FOUND,
            Json(
                json!({"apiVersion": "v1", "kind": "Status", "status": "Failure", "reason": "NotFound", "code": 404, "message": format!("{name} not found"), "metadata": {}}),
            ),
        ),
    }
}

async fn list(
    State(fake): State<Shared>,
    Path(kind): Path<String>,
    Query(params): Query<BTreeMap<String, String>>,
) -> Json<Value> {
    let mut fake = fake.lock().unwrap();
    fake.selectors
        .push(params.get("labelSelector").cloned().unwrap_or_default());
    let items: Vec<Value> = fake
        .objects
        .iter()
        .filter(|((k, _), _)| *k == kind)
        .map(|(_, object)| object.clone())
        .collect();
    Json(json!({"apiVersion": "v1", "kind": "List", "metadata": {}, "items": items}))
}

async fn remove(
    State(fake): State<Shared>,
    Path((kind, name)): Path<(String, String)>,
) -> (StatusCode, Json<Value>) {
    let mut fake = fake.lock().unwrap();
    fake.writes.push(format!("delete {kind}/{name}"));
    match fake.objects.remove(&(kind, name.clone())) {
        Some(_) => (
            StatusCode::OK,
            Json(
                json!({"apiVersion": "v1", "kind": "Status", "status": "Success", "metadata": {}}),
            ),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(
                json!({"apiVersion": "v1", "kind": "Status", "status": "Failure", "reason": "NotFound", "code": 404, "message": format!("{name} not found"), "metadata": {}}),
            ),
        ),
    }
}

/// One running pod for each Deployment scaled to one.
async fn pods(State(fake): State<Shared>) -> Json<Value> {
    let fake = fake.lock().unwrap();
    let items: Vec<Value> = fake
        .objects
        .iter()
        .filter(|((kind, _), d)| kind == "deployments" && d["spec"]["replicas"] == 1)
        .map(|((_, name), d)| {
            json!({
                "apiVersion": "v1", "kind": "Pod",
                "metadata": {"name": format!("{name}-7d9f"), "labels": d["spec"]["template"]["metadata"]["labels"]},
                "spec": {"containers": [{"name": "app", "image": "x"}], "nodeName": "node-1"},
                "status": {
                    "phase": "Running", "podIP": "10.42.0.77", "startTime": "2026-11-01T10:00:00Z",
                    "containerStatuses": [{"name": "app", "ready": true, "restartCount": 2, "image": "x", "imageID": "", "state": {"running": {}}}],
                },
            })
        })
        .collect();
    Json(json!({"apiVersion": "v1", "kind": "PodList", "metadata": {}, "items": items}))
}

async fn log() -> &'static str {
    "2026-11-01T10:00:01Z listening on :8080\n2026-11-01T10:00:02Z GET / 200\n"
}

async fn kubernetes() -> (String, Shared) {
    let fake = Shared::default();
    let objects = |prefix: &str| {
        Router::new()
            .route(&format!("{prefix}/{{kind}}"), get(list))
            .route(
                &format!("{prefix}/{{kind}}/{{name}}"),
                get(fetch).patch(apply).delete(remove),
            )
    };
    let app = Router::new()
        .route("/api/v1/namespaces/isolated/pods", get(pods))
        .route("/api/v1/namespaces/isolated/pods/{name}/log", get(log))
        .merge(objects("/api/v1/namespaces/isolated"))
        .merge(objects("/apis/apps/v1/namespaces/isolated"))
        .merge(objects("/apis/networking.k8s.io/v1/namespaces/isolated"))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, fake)
}

#[tokio::test]
async fn lifecycle_and_logs_use_the_supplied_namespace_and_manager() {
    let (base, fake) = kubernetes().await;
    let cluster = Cluster::at(&base, "isolated", "test-runner").unwrap();
    let work = Workload {
        id: "svc_example".into(),
        name: "example".into(),
        image: "nginx:1.27".into(),
        port: 8080,
        desired: Desired::Running,
        restarted_at: None,
        readiness: Readiness::Tcp,
        spec: Spec {
            volume: Some(Volume {
                path: "/data".into(),
                size_gib: 1,
            }),
            ..Spec::default()
        },
    };
    let placement = Placement {
        namespace: "isolated".into(),
        manager: "test-runner".into(),
        runtime_class: "kata-test".into(),
        storage_class: "local-path".into(),
        annotation_prefix: "example.test".into(),
        dns_servers: vec!["10.43.0.53".into()],
        ingress_namespace: "gateway".into(),
        ingress_pod_labels: BTreeMap::from([("role".into(), "gateway".into())]),
    };
    let rendered = objects(&work, &BTreeMap::new(), &placement);
    cluster.apply(&rendered).await.unwrap();
    cluster.apply(&rendered).await.unwrap();
    assert_eq!(
        fake.lock()
            .unwrap()
            .writes
            .iter()
            .filter(|w| w.starts_with("apply persistentvolumeclaims/"))
            .count(),
        1
    );
    assert!(cluster.managed().await.unwrap().contains("svc-example"));
    assert!(
        fake.lock()
            .unwrap()
            .selectors
            .iter()
            .all(|s| s == "app.kubernetes.io/managed-by=test-runner")
    );
    let pods = cluster.pods().await.unwrap();
    assert!(pods["svc-example"][0].ready);
    assert_eq!(pods["svc-example"][0].restarts, 2);
    let lines = cluster.follow_log("svc-example-7d9f", 10).await.unwrap();
    futures::pin_mut!(lines);
    assert!(
        lines
            .next()
            .await
            .unwrap()
            .unwrap()
            .ends_with("listening on :8080")
    );
    // A missing metrics API must not break service operation.
    assert!(cluster.usage().await.is_empty());
    let mut stopped = work;
    stopped.desired = Desired::Stopped;
    stopped.spec.volume = None;
    cluster
        .apply(&objects(&stopped, &BTreeMap::new(), &placement))
        .await
        .unwrap();
    assert!(cluster.pods().await.unwrap().is_empty());
    assert!(
        fake.lock()
            .unwrap()
            .objects
            .contains_key(&("persistentvolumeclaims".into(), "svc-example".into()))
    );
    cluster.remove("svc-example").await.unwrap();
    cluster.remove("svc-example").await.unwrap();
    assert!(cluster.managed().await.unwrap().is_empty());
    assert!(fake.lock().unwrap().objects.is_empty());
}
