# dyson-services

The container-service runner extracted from `dyson-inference-honeypot`.
It contains no product database, scheduler, authentication, environment-variable
lookup, planted-key minting, capture pipeline, or egress gate.

- `Workload` / `Spec`: container image and port, env/file templates, resource
  limits, hostname, allowed capabilities, optional persistent volume, desired
  running/stopped state, restart timestamp, and TCP or HTTP readiness.
- `validate`, `validate_templates`, `render`, `secret_data`: the established
  validation limits and credential template grammar (`{{key}}`, `{{base_url}}`,
  and named forms). Products resolve credentials and endpoints into `Rendered`.
- `Placement` / `objects`: explicit namespace, object manager, runtime/storage
  classes, annotation prefix, DNS servers and ingress pod selectors. No
  installation defaults are inferred. A changed secret or restart timestamp
  rolls the Deployment; stopping scales it to zero.
- `Cluster`: a product-authenticated kube client scoped to a namespace and
  manager; apply and remove objects, enumerate managed objects, read pod status,
  best-effort resource metrics, and follow logs. A PVC is created once and kept
  across restarts, stops, and removing the volume from the spec; deleting the
  service deletes its PVC. `at` constructs an unauthenticated local test client.
- `PodStatus` / `Status`: the existing desired/observed status contract.

Products own durable service IDs/records, ownership and authorization, domain
routing, credential resolution, scheduling/reconciliation, and installation
infrastructure. They install namespaces, RBAC, quotas, RuntimeClasses, storage
classes and default-deny/egress NetworkPolicies. The generated NetworkPolicy
only adds the supplied ingress rule. NetworkPolicies are additive, so products
must also control other policies in the workload namespace.

Pass validated workloads and trusted installation configuration to `objects`;
it is a pure renderer, not a policy admission boundary. In particular a product
must supply a supported isolated runtime class, rather than an empty class.

The honeypot adapter preserves its `apps` namespace, `honeypot` manager,
Kata Dragonball runtime, gate-only DNS, inference-only ingress, and Open WebUI
HTTP readiness exception. Planted credentials and captures remain in that
application. No Swarm integration is included in this extraction.

Run `cargo test -p dyson-services` and
`cargo clippy -p dyson-services --all-targets -- -D warnings` from the workspace.
Tests exercise template validation, state derivation and explicit installation
policy, plus Kubernetes apply/stop/PVC lifetime/delete/log operations against a
local API stand-in. They require localhost socket access, no real cluster.


Transport defaults remain `ring` for the honeypot. Applications using AWS-LC can
select `default-features = false, features = ["aws-lc-rs"]`, keeping one Rustls
crypto provider. `Cluster::infer(namespace, manager)` uses the application's
Kubernetes credentials without requiring it to share this crate's kube version.
The product still supplies namespace, manager and every Placement setting.
