use super::*;

/// Explicit installation policy: no product names, environment variables or
/// cluster addresses are inferred by the shared runner.
#[derive(Debug, Clone)]
pub struct Placement {
    pub namespace: String,
    pub manager: String,
    pub runtime_class: String,
    pub storage_class: String,
    pub annotation_prefix: String,
    /// Empty uses cluster DNS. A product may instead supply its egress resolver.
    pub dns_servers: Vec<String>,
    pub ingress_namespace: String,
    pub ingress_pod_labels: BTreeMap<String, String>,
}

/// The objects of one service, as server-side apply takes them.
#[derive(Debug, Clone)]
pub struct Objects {
    pub secret: Value,
    pub deployment: Value,
    pub service: Value,
    pub policy: Value,
    /// Its volume's claim, when it keeps one: made once, never applied
    /// again, so a bound claim is not changed under its pod.
    pub claim: Option<Value>,
    /// A digest of all four: what changed when it changes.
    pub digest: String,
}

fn labels(service: &Workload, placement: &Placement) -> Value {
    json!({MANAGED: placement.manager, APP: service.object()})
}

/// Everything the cluster runs for a service, from its spec and its
/// rendered Secret.
pub fn objects(
    service: &Workload,
    data: &BTreeMap<String, String>,
    placement: &Placement,
) -> Objects {
    use sha2::Digest as _;
    let name = service.object();
    let secret_digest = hex::encode(sha2::Sha256::digest(
        serde_json::to_vec(data).unwrap_or_default(),
    ));
    let secret = json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {"name": name, "namespace": placement.namespace, "labels": labels(service, placement)},
        "type": "Opaque",
        "stringData": data,
    });
    let env: Vec<Value> = service
        .spec
        .env
        .iter()
        .map(|var| {
            json!({"name": var.name, "valueFrom": {"secretKeyRef": {"name": name, "key": format!("env.{}", var.name)}}})
        })
        .collect();
    let mut mounts: Vec<Value> = service
        .spec
        .files
        .iter()
        .enumerate()
        .map(|(n, file)| {
            json!({"name": "files", "mountPath": file.path, "subPath": format!("file.{n}"), "readOnly": true})
        })
        .collect();
    let mut volumes = Vec::new();
    if !service.spec.files.is_empty() {
        volumes
            .push(json!({"name": "files", "secret": {"secretName": name, "defaultMode": 0o444}}));
    }
    let claim = service.spec.volume.as_ref().map(|volume| {
        mounts.push(json!({"name": "data", "mountPath": volume.path}));
        volumes.push(json!({"name": "data", "persistentVolumeClaim": {"claimName": name}}));
        json!({
            "apiVersion": "v1",
            "kind": "PersistentVolumeClaim",
            "metadata": {"name": name, "namespace": placement.namespace, "labels": labels(service, placement)},
            "spec": {
                "accessModes": ["ReadWriteOnce"],
                "storageClassName": placement.storage_class,
                "resources": {"requests": {"storage": format!("{}Gi", volume.size_gib)}},
            },
        })
    });
    let resources = service.spec.resources;
    let mut container = json!({
        "name": service.name,
        "image": service.image,
        "imagePullPolicy": "IfNotPresent",
        "ports": [{"containerPort": service.port, "protocol": "TCP"}],
        "readinessProbe": {
            "tcpSocket": {"port": service.port},
            "periodSeconds": 2,
            "timeoutSeconds": 1,
            "failureThreshold": 30,
        },
        "env": env,
        "volumeMounts": mounts,
        "resources": {
            "limits": {"cpu": format!("{}m", resources.cpu_millis), "memory": format!("{}Mi", resources.memory_mib)},
            "requests": {"cpu": format!("{}m", (resources.cpu_millis / 4).max(10)), "memory": format!("{}Mi", (resources.memory_mib / 2).max(32))},
        },
        "securityContext": {
            "allowPrivilegeEscalation": false,
            "privileged": false,
            "capabilities": {"drop": ["ALL"]},
            "seccompProfile": {"type": "RuntimeDefault"},
        },
    });
    if let Readiness::Http { path } = &service.readiness {
        container["readinessProbe"] = json!({
            "httpGet": {"path": path, "port": service.port},
            "periodSeconds": 2, "timeoutSeconds": 2, "failureThreshold": 30,
        });
    }
    if !service.spec.add_capabilities.is_empty() {
        container["securityContext"]["capabilities"]["add"] = json!(service.spec.add_capabilities);
    }
    let mut pod = json!({
        "automountServiceAccountToken": false,
        "enableServiceLinks": false,
        "hostname": service.hostname(),
        // Explicit resolvers have no cluster search domains. An empty list
        // opts into the cluster's ordinary resolver instead.
        "dnsPolicy": if placement.dns_servers.is_empty() { "ClusterFirst" } else { "None" },
        "dnsConfig": {"nameservers": placement.dns_servers, "searches": [], "options": [{"name": "ndots", "value": "1"}]},
        "terminationGracePeriodSeconds": 10,
        "containers": [container],
        "volumes": volumes,
    });
    if placement.dns_servers.is_empty() {
        pod.as_object_mut()
            .expect("a pod spec is an object")
            .remove("dnsConfig");
    }
    pod["runtimeClassName"] = json!(placement.runtime_class);
    let deployment = json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {"name": name, "namespace": placement.namespace, "labels": labels(service, placement)},
        "spec": {
            "replicas": if service.desired == Desired::Running { 1 } else { 0 },
            "revisionHistoryLimit": 2,
            "strategy": {"type": "Recreate"},
            "selector": {"matchLabels": {APP: name}},
            "template": {
                "metadata": {
                    "labels": labels(service, placement),
                    // A changed Secret rolls the pod: it reads its files once.
                    "annotations": {
                        (format!("{}/secret", placement.annotation_prefix)): &secret_digest[..16],
                        (format!("{}/restarted", placement.annotation_prefix)): service.restarted_at.map(|at| at.to_rfc3339()).unwrap_or_default(),
                    },
                },
                "spec": pod,
            },
        },
    });
    let kube_service = json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {"name": name, "namespace": placement.namespace, "labels": labels(service, placement)},
        "spec": {
            "type": "ClusterIP",
            "selector": {APP: name},
            "ports": [{"name": "http", "port": service.port, "targetPort": service.port, "protocol": "TCP"}],
        },
    });
    // The product installs namespace default-deny policy. This rule allows
    // its configured ingress pods to reach the workload's HTTP port.
    let policy = json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "NetworkPolicy",
        "metadata": {"name": name, "namespace": placement.namespace, "labels": labels(service, placement)},
        "spec": {
            "podSelector": {"matchLabels": {APP: name}},
            "policyTypes": ["Ingress"],
            "ingress": [{
                "from": [{
                    "namespaceSelector": {"matchLabels": {"kubernetes.io/metadata.name": placement.ingress_namespace}},
                    "podSelector": {"matchLabels": placement.ingress_pod_labels},
                }],
                "ports": [{"protocol": "TCP", "port": service.port}],
            }],
        },
    });
    let digest = hex::encode(sha2::Sha256::digest(
        serde_json::to_vec(&json!([secret, deployment, kube_service, policy])).unwrap_or_default(),
    ));
    Objects {
        secret,
        deployment,
        service: kube_service,
        policy,
        claim,
        digest: digest[..16].to_owned(),
    }
}

/// The Kubernetes API, for the services' namespace.
#[derive(Clone)]
pub struct Cluster {
    client: kube::Client,
    namespace: String,
    manager: String,
}

impl Cluster {
    /// Use a product-authenticated client, scoped to one namespace and manager.
    pub fn new(client: kube::Client, namespace: &str, manager: &str) -> Self {
        Self {
            client,
            namespace: namespace.into(),
            manager: manager.into(),
        }
    }

    /// An unauthenticated Kubernetes API, for local test fixtures.
    pub fn at(url: &str, namespace: &str, manager: &str) -> anyhow::Result<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut config = kube::Config::new(url.parse().context("the Kubernetes API address")?);
        config.default_namespace = namespace.into();
        Ok(Self::new(
            kube::Client::try_from(config).context("make the Kubernetes client")?,
            namespace,
            manager,
        ))
    }

    fn api<K>(&self) -> kube::Api<K>
    where
        K: kube::Resource<Scope = k8s_openapi::NamespaceResourceScope>,
        <K as kube::Resource>::DynamicType: Default,
    {
        kube::Api::namespaced(self.client.clone(), &self.namespace)
    }

    /// Apply a service's objects, server-side, as their one manager.
    pub async fn apply(&self, objects: &Objects) -> anyhow::Result<()> {
        use k8s_openapi::api::{
            apps::v1::Deployment,
            core::v1::{Secret, Service as KubeService},
            networking::v1::NetworkPolicy,
        };
        use kube::api::{Patch, PatchParams};
        let params = PatchParams::apply(&self.manager).force();
        let name = |object: &Value| object["metadata"]["name"].as_str().unwrap_or("").to_owned();
        if let Some(claim) = &objects.claim {
            use k8s_openapi::api::core::v1::PersistentVolumeClaim;
            let claims = self.api::<PersistentVolumeClaim>();
            // Made once: a bound claim's size cannot change on local-path,
            // and the one it has is the one its data is in.
            if claims
                .get_opt(&name(claim))
                .await
                .context("read its volume's claim")?
                .is_none()
            {
                claims
                    .patch(&name(claim), &params, &Patch::Apply(claim))
                    .await
                    .context("make its volume's claim")?;
            }
        }
        self.api::<Secret>()
            .patch(
                &name(&objects.secret),
                &params,
                &Patch::Apply(&objects.secret),
            )
            .await
            .context("apply its Secret")?;
        self.api::<KubeService>()
            .patch(
                &name(&objects.service),
                &params,
                &Patch::Apply(&objects.service),
            )
            .await
            .context("apply its Service")?;
        self.api::<NetworkPolicy>()
            .patch(
                &name(&objects.policy),
                &params,
                &Patch::Apply(&objects.policy),
            )
            .await
            .context("apply its NetworkPolicy")?;
        self.api::<Deployment>()
            .patch(
                &name(&objects.deployment),
                &params,
                &Patch::Apply(&objects.deployment),
            )
            .await
            .context("apply its Deployment")?;
        Ok(())
    }

    /// The names of the objects of each kind this application applied.
    pub async fn managed(&self) -> anyhow::Result<std::collections::BTreeSet<String>> {
        use k8s_openapi::api::{
            apps::v1::Deployment,
            core::v1::{PersistentVolumeClaim, Secret, Service as KubeService},
            networking::v1::NetworkPolicy,
        };
        let params =
            kube::api::ListParams::default().labels(&format!("{MANAGED}={}", self.manager));
        let mut names = std::collections::BTreeSet::new();
        let mut add = |items: Vec<Option<String>>| names.extend(items.into_iter().flatten());
        add(self
            .api::<Deployment>()
            .list(&params)
            .await?
            .items
            .into_iter()
            .map(|o| o.metadata.name)
            .collect());
        add(self
            .api::<Secret>()
            .list(&params)
            .await?
            .items
            .into_iter()
            .map(|o| o.metadata.name)
            .collect());
        add(self
            .api::<KubeService>()
            .list(&params)
            .await?
            .items
            .into_iter()
            .map(|o| o.metadata.name)
            .collect());
        add(self
            .api::<NetworkPolicy>()
            .list(&params)
            .await?
            .items
            .into_iter()
            .map(|o| o.metadata.name)
            .collect());
        add(self
            .api::<PersistentVolumeClaim>()
            .list(&params)
            .await?
            .items
            .into_iter()
            .map(|o| o.metadata.name)
            .collect());
        Ok(names)
    }

    /// Delete every object of this name, whichever exist: its service is
    /// gone, so its volume goes too.
    pub async fn remove(&self, name: &str) -> anyhow::Result<()> {
        use k8s_openapi::api::{
            apps::v1::Deployment,
            core::v1::{PersistentVolumeClaim, Secret, Service as KubeService},
            networking::v1::NetworkPolicy,
        };
        let params = kube::api::DeleteParams::background();
        let gone = |result: Result<_, kube::Error>| match result {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(status)) if status.code == 404 => Ok(()),
            Err(error) => Err(error),
        };
        gone(
            self.api::<Deployment>()
                .delete(name, &params)
                .await
                .map(|_| ()),
        )?;
        gone(
            self.api::<KubeService>()
                .delete(name, &params)
                .await
                .map(|_| ()),
        )?;
        gone(
            self.api::<NetworkPolicy>()
                .delete(name, &params)
                .await
                .map(|_| ()),
        )?;
        gone(self.api::<Secret>().delete(name, &params).await.map(|_| ()))?;
        gone(
            self.api::<PersistentVolumeClaim>()
                .delete(name, &params)
                .await
                .map(|_| ()),
        )?;
        Ok(())
    }

    /// Every service pod, by the object name of its service.
    pub async fn pods(&self) -> anyhow::Result<BTreeMap<String, Vec<PodStatus>>> {
        use k8s_openapi::api::core::v1::Pod;
        let params =
            kube::api::ListParams::default().labels(&format!("{MANAGED}={}", self.manager));
        let mut out: BTreeMap<String, Vec<PodStatus>> = BTreeMap::new();
        for pod in self.api::<Pod>().list(&params).await?.items {
            let Some(app) = pod
                .metadata
                .labels
                .as_ref()
                .and_then(|l| l.get(APP))
                .cloned()
            else {
                continue;
            };
            out.entry(app).or_default().push(pod_status(&pod));
        }
        Ok(out)
    }

    /// CPU and memory by pod name, from the metrics API; empty when it
    /// does not answer.
    pub async fn usage(&self) -> BTreeMap<String, (u64, u64)> {
        let request = match http::Request::get(format!(
            "/apis/metrics.k8s.io/v1beta1/namespaces/{}/pods?labelSelector={MANAGED}%3D{}",
            self.namespace, self.manager
        ))
        .body(Vec::new())
        {
            Ok(request) => request,
            Err(_) => return BTreeMap::new(),
        };
        let Ok(answer) = self.client.request::<Value>(request).await else {
            return BTreeMap::new();
        };
        let mut out = BTreeMap::new();
        for item in answer["items"].as_array().into_iter().flatten() {
            let Some(name) = item["metadata"]["name"].as_str() else {
                continue;
            };
            let (mut cpu, mut memory) = (0, 0);
            for container in item["containers"].as_array().into_iter().flatten() {
                cpu += parse_cpu(container["usage"]["cpu"].as_str().unwrap_or(""));
                memory += parse_memory(container["usage"]["memory"].as_str().unwrap_or(""));
            }
            out.insert(name.to_owned(), (cpu, memory));
        }
        out
    }

    /// A pod's log as it is written, from the last `tail` lines.
    pub async fn follow_log(
        &self,
        pod: &str,
        tail: i64,
    ) -> anyhow::Result<impl futures::Stream<Item = std::io::Result<String>> + use<>> {
        use futures::AsyncBufReadExt as _;
        use k8s_openapi::api::core::v1::Pod;
        let params = kube::api::LogParams {
            follow: true,
            tail_lines: Some(tail),
            timestamps: true,
            ..Default::default()
        };
        let reader = self.api::<Pod>().log_stream(pod, &params).await?;
        Ok(reader.lines())
    }
}

fn pod_status(pod: &k8s_openapi::api::core::v1::Pod) -> PodStatus {
    let status = pod.status.clone().unwrap_or_default();
    let containers = status.container_statuses.clone().unwrap_or_default();
    let waiting = containers.iter().find_map(|c| {
        c.state
            .as_ref()
            .and_then(|s| s.waiting.clone())
            .or_else(|| {
                c.state
                    .as_ref()
                    .and_then(|s| s.terminated.as_ref())
                    .map(|t| k8s_openapi::api::core::v1::ContainerStateWaiting {
                        reason: t.reason.clone(),
                        message: t.message.clone(),
                    })
            })
    });
    let unscheduled = status
        .conditions
        .clone()
        .unwrap_or_default()
        .into_iter()
        .find(|c| c.type_ == "PodScheduled" && c.status == "False");
    PodStatus {
        name: pod.metadata.name.clone().unwrap_or_default(),
        phase: status.phase.clone().unwrap_or_else(|| "Unknown".into()),
        ready: !containers.is_empty() && containers.iter().all(|c| c.ready),
        restarts: containers.iter().map(|c| i64::from(c.restart_count)).sum(),
        ip: status.pod_ip.clone(),
        node: pod.spec.as_ref().and_then(|s| s.node_name.clone()),
        started_at: status.start_time.as_ref().map(|t| t.0.to_string()),
        reason: waiting
            .as_ref()
            .and_then(|w| w.reason.clone())
            .or_else(|| unscheduled.as_ref().and_then(|c| c.reason.clone())),
        message: waiting
            .and_then(|w| w.message)
            .or_else(|| unscheduled.and_then(|c| c.message)),
        cpu_millis: None,
        memory_mib: None,
    }
}

/// `250m`, `1`, `12345678n` as millicores.
fn parse_cpu(value: &str) -> u64 {
    let value = value.trim();
    if let Some(n) = value.strip_suffix('n') {
        n.parse::<u64>().map_or(0, |n| n / 1_000_000)
    } else if let Some(u) = value.strip_suffix('u') {
        u.parse::<u64>().map_or(0, |u| u / 1_000)
    } else if let Some(m) = value.strip_suffix('m') {
        m.parse().unwrap_or(0)
    } else {
        value
            .parse::<f64>()
            .map_or(0, |cores| (cores * 1000.0) as u64)
    }
}

/// `123456Ki`, `64Mi`, `1Gi`, bytes as MiB.
fn parse_memory(value: &str) -> u64 {
    let value = value.trim();
    let (number, scale) = [("Ki", 1u64 << 10), ("Mi", 1 << 20), ("Gi", 1 << 30)]
        .into_iter()
        .find_map(|(suffix, scale)| value.strip_suffix(suffix).map(|n| (n, scale)))
        .unwrap_or((value, 1));
    number.parse::<u64>().map_or(0, |n| n * scale / (1 << 20))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usage_is_read_in_millicores_and_mebibytes() {
        assert_eq!(parse_cpu("250m"), 250);
        assert_eq!(parse_cpu("12000000n"), 12);
        assert_eq!(parse_cpu("2"), 2000);
        assert_eq!(parse_memory("65536Ki"), 64);
        assert_eq!(parse_memory("1Gi"), 1024);
    }
    #[test]
    fn installation_policy_has_no_honeypot_defaults() {
        let work = crate::tests::workload();
        let placement = Placement {
            namespace: "tenant-a".into(),
            manager: "test-runner".into(),
            runtime_class: "kata-test".into(),
            storage_class: "test-disk".into(),
            annotation_prefix: "example.test".into(),
            dns_servers: vec![],
            ingress_namespace: "gateway".into(),
            ingress_pod_labels: BTreeMap::from([("role".into(), "gateway".into())]),
        };
        let rendered = objects(&work, &BTreeMap::new(), &placement);
        let pod = &rendered.deployment["spec"]["template"]["spec"];
        assert_eq!(pod["dnsPolicy"], "ClusterFirst");
        assert!(pod.get("dnsConfig").is_none());
        assert_eq!(pod["runtimeClassName"], "kata-test");
        for object in [
            &rendered.secret,
            &rendered.deployment,
            &rendered.service,
            &rendered.policy,
        ] {
            assert_eq!(object["metadata"]["namespace"], "tenant-a");
            assert_eq!(object["metadata"]["labels"][MANAGED], "test-runner");
        }
        assert!(
            rendered.deployment["spec"]["template"]["metadata"]["annotations"]
                .get("example.test/secret")
                .is_some()
        );
        let from = &rendered.policy["spec"]["ingress"][0]["from"][0];
        assert_eq!(
            from["namespaceSelector"]["matchLabels"]["kubernetes.io/metadata.name"],
            "gateway"
        );
        assert_eq!(from["podSelector"]["matchLabels"]["role"], "gateway");
    }
}
