//! Shared container-service runner. Products own persistence, authorization,
//! credential resolution, ingress routing, egress policy and scheduling.
//! Namespace infrastructure (RBAC, default-deny/egress and quotas) is installed
//! by the product; this crate applies only a workload's namespaced objects.

use anyhow::Context as _;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub mod kubernetes;
pub use kubernetes::{Cluster, Objects, Placement, objects};

pub const MANAGED: &str = "app.kubernetes.io/managed-by";
pub const APP: &str = "app";
pub const MAX_ENV: usize = 64;
pub const MAX_FILES: usize = 32;
pub const MAX_RENDERED: usize = 512 * 1024;
pub const MAX_VOLUME_GIB: u32 = 20;
/// The largest memory limit a Service pod may ask for. The pod requests half.
pub const MAX_MEMORY_MIB: u32 = 8192;
/// Capabilities a service may have back after all are dropped: the ones
/// Pod Security `baseline` allows, which an image running as root needs to
/// drop to its own user.
pub const ADDABLE: &[&str] = &[
    "CHOWN",
    "DAC_OVERRIDE",
    "FOWNER",
    "FSETID",
    "KILL",
    "NET_BIND_SERVICE",
    "SETGID",
    "SETUID",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Desired {
    #[default]
    Running,
    Stopped,
}

impl Desired {
    pub fn name(self) -> &'static str {
        match self {
            Desired::Running => "running",
            Desired::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvVar {
    pub name: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileTemplate {
    /// Absolute path inside the container.
    pub path: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub cpu_millis: u32,
    pub memory_mib: u32,
}

/// A directory the service keeps across restarts and changes: one claim
/// per service, made the first time it is asked for and deleted only with
/// the service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Volume {
    /// Absolute path inside the container.
    pub path: String,
    pub size_gib: u32,
}

impl Default for Resources {
    fn default() -> Self {
        Self {
            cpu_millis: 250,
            memory_mib: 256,
        }
    }
}

/// Configuration of the container itself. Product credentials and routing stay outside it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    #[serde(default)]
    pub env: Vec<EnvVar>,
    #[serde(default)]
    pub files: Vec<FileTemplate>,
    #[serde(default)]
    pub resources: Resources,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub add_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<Volume>,
}

/// A product's durable record projected onto what the runner needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workload {
    pub id: String,
    pub name: String,
    pub image: String,
    pub port: u16,
    pub spec: Spec,
    pub desired: Desired,
    pub restarted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub readiness: Readiness,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Readiness {
    #[default]
    Tcp,
    Http {
        path: String,
    },
}

impl Workload {
    pub fn object(&self) -> String {
        self.id.replace('_', "-")
    }
    pub fn hostname(&self) -> &str {
        match self.spec.hostname.trim() {
            "" => &self.name,
            hostname => hostname,
        }
    }
}

/// One pod of a service, as the worker last saw it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PodStatus {
    pub name: String,
    pub phase: String,
    pub ready: bool,
    pub restarts: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// Why it is not running, when the cluster says (`ImagePullBackOff`,
    /// `CrashLoopBackOff`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// From the metrics API, when it answers: millicores and MiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mib: Option<u64>,
}

/// What the worker last saw of a service.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// `running`, `starting`, `stopped`, `failing` or `unknown`.
    pub state: String,
    #[serde(default)]
    pub pods: Vec<PodStatus>,
    /// The digest of the objects last applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub observed_at: DateTime<Utc>,
}

impl Status {
    /// The state a service's pods add up to.
    pub fn summarise(desired: Desired, pods: &[PodStatus]) -> &'static str {
        if desired == Desired::Stopped {
            return if pods.is_empty() {
                "stopped"
            } else {
                "stopping"
            };
        }
        if pods.iter().any(|pod| pod.ready) {
            "running"
        } else if pods.iter().any(|pod| {
            pod.reason.as_deref().is_some_and(|reason| {
                reason.contains("BackOff")
                    || reason.contains("Err")
                    || reason == "InvalidImageName"
                    || reason == "CreateContainerConfigError"
            })
        }) {
            "failing"
        } else {
            "starting"
        }
    }
}

fn dns_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 40
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn env_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    value.len() <= 128
        && bytes
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl Workload {
    /// Validate container configuration before persisting or applying it.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            dns_label(self.name.trim()),
            "the name must be up to 40 lowercase letters, digits and dashes"
        );
        let image = self.image.trim();
        anyhow::ensure!(
            !image.is_empty() && image.len() <= 256 && !image.chars().any(char::is_whitespace),
            "the image must be a reference like nginx:1.27 or ghcr.io/acme/app:latest"
        );
        anyhow::ensure!(self.port > 0, "the port must be between 1 and 65535");
        let spec = &self.spec;
        anyhow::ensure!(spec.env.len() <= MAX_ENV, "at most {MAX_ENV} variables");
        anyhow::ensure!(spec.files.len() <= MAX_FILES, "at most {MAX_FILES} files");
        let mut seen = std::collections::HashSet::new();
        for var in &spec.env {
            anyhow::ensure!(
                env_name(&var.name),
                "{:?} is not a variable name: letters, digits and _, not starting with a digit",
                var.name
            );
            anyhow::ensure!(seen.insert(&var.name), "{} is set twice", var.name);
        }
        let mut paths = std::collections::HashSet::new();
        for file in &spec.files {
            let path = file.path.as_str();
            anyhow::ensure!(
                path.starts_with('/')
                    && !path.ends_with('/')
                    && path.len() <= 512
                    && !path.split('/').any(|part| part == ".." || part == "."),
                "{path:?} must be an absolute path to a file"
            );
            anyhow::ensure!(paths.insert(path), "{path} is written twice");
        }
        let hostname = spec.hostname.trim();
        anyhow::ensure!(
            hostname.is_empty() || (dns_label(hostname) && hostname.len() <= 40),
            "the machine name must be lowercase letters, digits and dashes"
        );
        for capability in &spec.add_capabilities {
            anyhow::ensure!(
                ADDABLE.contains(&capability.as_str()),
                "{capability} cannot be added; only {}",
                ADDABLE.join(", ")
            );
        }
        if let Some(volume) = &spec.volume {
            let path = volume.path.as_str();
            anyhow::ensure!(
                path.starts_with('/')
                    && path.len() > 1
                    && !path.ends_with('/')
                    && path.len() <= 512
                    && !path.split('/').any(|part| part == ".." || part == "."),
                "the volume's path {path:?} must be an absolute directory other than /"
            );
            anyhow::ensure!(
                (1..=MAX_VOLUME_GIB).contains(&volume.size_gib),
                "a volume is between 1 and {MAX_VOLUME_GIB} GiB"
            );
        }
        let resources = spec.resources;
        anyhow::ensure!(
            (50..=2000).contains(&resources.cpu_millis),
            "CPU is between 50 and 2000 millicores"
        );
        anyhow::ensure!(
            (64..=MAX_MEMORY_MIB).contains(&resources.memory_mib),
            "memory is between 64 and {MAX_MEMORY_MIB} MiB"
        );
        Ok(())
    }
}
/// A key as the templates are filled with it.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub name: String,
    pub key: String,
    /// The endpoint the product supplies for this credential.
    pub base_url: String,
}

/// The placeholders a template uses, trimmed, in order.
pub fn placeholders(template: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                found.push(after[..end].trim().to_owned());
                rest = &after[end + 2..];
            }
            None => break,
        }
    }
    found
}

/// Fill a template: `{{key}}` and `{{base_url}}` are the first key's,
/// `{{key.<name>}}` and `{{base_url.<name>}}` any key's. Anything else in
/// braces is refused rather than left for the box to show.
pub fn render(template: &str, keys: &[Rendered]) -> anyhow::Result<String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            return Ok(out);
        };
        let name = after[..end].trim();
        let (what, key) = match name.split_once('.') {
            Some((what, key)) => (what, keys.iter().find(|k| k.name == key)),
            None => (name, keys.first()),
        };
        let value = match (what, key) {
            ("key", Some(key)) => &key.key,
            ("base_url", Some(key)) => &key.base_url,
            ("key" | "base_url", None) => {
                anyhow::bail!("{{{{{name}}}}} names a key the service does not have")
            }
            _ => anyhow::bail!(
                "{{{{{name}}}}} is not a placeholder: use {{{{key}}}}, {{{{base_url}}}}, {{{{key.<name>}}}} or {{{{base_url.<name>}}}}"
            ),
        };
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// A service's environment and files, filled in, as its Secret holds them:
/// `env.<NAME>` and `file.<n>`.
pub fn secret_data(
    service: &Workload,
    keys: &[Rendered],
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut data = BTreeMap::new();
    for var in &service.spec.env {
        data.insert(
            format!("env.{}", var.name),
            render(&var.value, keys).with_context(|| format!("in {}", var.name))?,
        );
    }
    for (n, file) in service.spec.files.iter().enumerate() {
        data.insert(
            format!("file.{n}"),
            render(&file.content, keys).with_context(|| format!("in {}", file.path))?,
        );
    }
    Ok(data)
}

impl Workload {
    /// Check template names and rendered size using product-resolved bindings.
    /// Use empty credential values for validation before credentials are minted.
    pub fn validate_templates(&self, keys: &[Rendered]) -> anyhow::Result<()> {
        let data = secret_data(self, keys)?;
        let size: usize = data.values().map(String::len).sum();
        anyhow::ensure!(
            size <= MAX_RENDERED,
            "the variables and files come to more than {} KiB",
            MAX_RENDERED / 1024
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn keys() -> Vec<Rendered> {
        vec![
            Rendered {
                name: "main".into(),
                key: "sk-one".into(),
                base_url: "https://a.example/v1".into(),
            },
            Rendered {
                name: "spare".into(),
                key: "sk-two".into(),
                base_url: "https://b.example/v1".into(),
            },
        ]
    }

    #[test]
    fn placeholders_fill_from_the_keys() {
        assert_eq!(
            render("OPENAI_API_KEY={{ key }}\nBASE={{base_url}}", &keys()).unwrap(),
            "OPENAI_API_KEY=sk-one\nBASE=https://a.example/v1"
        );
        assert_eq!(
            render("{{key.spare}}@{{base_url.spare}}", &keys()).unwrap(),
            "sk-two@https://b.example/v1"
        );
        assert_eq!(render("no braces", &keys()).unwrap(), "no braces");
        assert_eq!(render("half {{ open", &keys()).unwrap(), "half {{ open");
        assert!(render("{{key.missing}}", &keys()).is_err());
        assert!(render("{{password}}", &keys()).is_err());
        assert!(render("{{key}}", &[]).is_err());
    }

    #[test]
    fn templates_check_size_after_substitution() {
        let mut work = workload();
        work.spec.env = vec![EnvVar {
            name: "API_KEY".into(),
            value: "{{key}}".into(),
        }];
        work.validate().unwrap();
        work.validate_templates(&keys()).unwrap();
        assert!(work.validate_templates(&[]).is_err());
        work.spec.env[0].value = "x".repeat(MAX_RENDERED + 1);
        assert!(work.validate_templates(&[]).is_err());
    }

    pub(super) fn workload() -> Workload {
        Workload {
            id: "svc_example".into(),
            name: "example".into(),
            image: "nginx:1.27".into(),
            port: 8080,
            spec: Spec::default(),
            desired: Desired::Running,
            restarted_at: None,
            readiness: Readiness::Tcp,
        }
    }

    #[test]
    fn validation_preserves_resource_and_mount_limits() {
        let valid = workload();
        valid.validate().unwrap();
        for path in ["relative", "/app/../etc", "/"] {
            let mut bad = valid.clone();
            bad.spec.volume = Some(Volume {
                path: path.into(),
                size_gib: 1,
            });
            assert!(bad.validate().is_err());
        }
        let mut bad = valid.clone();
        bad.spec.add_capabilities = vec!["SYS_ADMIN".into()];
        assert!(bad.validate().is_err());
        bad = valid.clone();
        bad.spec.resources.memory_mib = 0;
        assert!(bad.validate().is_err());
        bad = valid.clone();
        bad.spec.resources.memory_mib = MAX_MEMORY_MIB + 1;
        assert!(bad.validate().is_err());
        let mut largest = valid.clone();
        largest.spec.resources.memory_mib = MAX_MEMORY_MIB;
        assert!(largest.validate().is_ok());
        bad = valid.clone();
        bad.spec.env = vec![
            EnvVar {
                name: "DUPLICATE".into(),
                value: String::new()
            };
            2
        ];
        assert!(bad.validate().is_err());
    }

    #[test]
    fn status_preserves_stop_and_failure_transitions() {
        let mut pod = PodStatus {
            ready: true,
            ..PodStatus::default()
        };
        assert_eq!(
            Status::summarise(Desired::Running, &[pod.clone()]),
            "running"
        );
        assert_eq!(
            Status::summarise(Desired::Stopped, &[pod.clone()]),
            "stopping"
        );
        assert_eq!(Status::summarise(Desired::Stopped, &[]), "stopped");
        pod.ready = false;
        pod.reason = Some("ImagePullBackOff".into());
        assert_eq!(Status::summarise(Desired::Running, &[pod]), "failing");
        assert_eq!(Status::summarise(Desired::Running, &[]), "starting");
    }
}
