# Cluster Config Auto-Fetch (Issue #76)

When `CLUSTER_ENABLED=true` and `CLUSTER_CONFIG_SOURCE` is set, the elected
Raft leader automatically retrieves the test configuration from an external
store and commits it to the Raft log.  All followers apply the same config
through the normal Raft state-machine path — no manual `POST /cluster/config`
is needed.

If `CLUSTER_CONFIG_SOURCE` is **not** set, auto-fetch is disabled and the
cluster waits for a manual `POST /cluster/config` submission (the existing
behaviour from Issue #78).

---

## Supported sources

| `CLUSTER_CONFIG_SOURCE` | Best for |
|-------------------------|----------|
| `consul-kv` | Nomad, local, and on-prem deployments |
| `gcs` | GCP / Cloud Run / GKE deployments |

---

## Consul KV (`consul-kv`)

### How it works

1. Elected leader calls `GET {CONSUL_ADDR}/v1/kv/{CONSUL_CONFIG_KEY}`.
2. Consul returns a JSON array `[{"Value": "<base64-encoded-yaml>"}]`.
3. Leader base64-decodes the value, commits the YAML to Raft, and all nodes
   reconfigure their worker pools.

The Consul agent is already running on every Nomad host for service discovery
(Issue #47).  The KV lookup reuses the same agent — no extra connection needed.

### Setup

```bash
# Write (or update) the test config before starting (or while the cluster runs).
consul kv put loadtest/config @my-test-config.yaml

# Verify it was stored.
consul kv get loadtest/config
```

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CLUSTER_CONFIG_SOURCE` | (unset) | Set to `consul-kv` to enable |
| `CONSUL_ADDR` | `http://127.0.0.1:8500` | Consul agent HTTP address |
| `CONSUL_CONFIG_KEY` | `loadtest/config` | KV path to read |
| `CLUSTER_CONFIG_TIMEOUT_SECS` | `30` | Abort fetch after this many seconds |

### Nomad template snippet

```hcl
template {
  destination = "secrets/config.env"
  env         = true
  data        = <<EOH
CLUSTER_ENABLED=true
CLUSTER_CONFIG_SOURCE=consul-kv
CONSUL_ADDR=http://127.0.0.1:8500
CONSUL_CONFIG_KEY=loadtest/config
CLUSTER_CONFIG_TIMEOUT_SECS=30

DISCOVERY_MODE=consul
CONSUL_SERVICE_NAME=loadtest-cluster
CLUSTER_MIN_PEERS=2
CLUSTER_SELF_ADDR={{ env "NOMAD_IP_cluster" }}:7000
EOH
}
```

### Updating config while the cluster is running

Overwrite the Consul KV key at any time:

```bash
consul kv put loadtest/config @new-config.yaml
```

The running cluster will **not** automatically re-fetch — the auto-fetch fires
only when a node first becomes leader.  To apply a new config without
restarting, use `POST /cluster/config` against the leader, or trigger a
re-election (e.g. rolling restart of one node).

---

## GCS Bucket (`gcs`)

### How it works

1. Elected leader calls the GCE instance metadata service to obtain a
   short-lived OAuth 2.0 access token for the node's service account.
2. Leader fetches the config object from the GCS JSON API using that token.
3. Leader commits the YAML to Raft, and all nodes reconfigure.

No GCP SDK or credentials file is needed — the metadata server at
`http://metadata.google.internal` is always available on GCE VMs, GKE nodes,
Cloud Run instances, and Compute Engine-backed Nomad clients.

### IAM prerequisite

Grant the node's service account `roles/storage.objectViewer` on the bucket:

```bash
gcloud storage buckets add-iam-policy-binding gs://my-loadtest-configs \
  --member="serviceAccount:my-node-sa@my-project.iam.gserviceaccount.com" \
  --role="roles/storage.objectViewer"
```

For GKE with Workload Identity, annotate the Kubernetes service account
instead.  For Cloud Run, use the revision's service account.

### Uploading a config

```bash
# Initial upload.
gsutil cp my-test-config.yaml gs://my-loadtest-configs/configs/prod-test.yaml

# Update in place (takes effect on the next leader election).
gsutil cp new-config.yaml gs://my-loadtest-configs/configs/prod-test.yaml
```

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CLUSTER_CONFIG_SOURCE` | (unset) | Set to `gcs` to enable |
| `GCS_CONFIG_BUCKET` | (required) | Bucket name, e.g. `my-loadtest-configs` |
| `GCS_CONFIG_OBJECT` | (required) | Object path, e.g. `configs/prod-test.yaml` |
| `CLUSTER_CONFIG_TIMEOUT_SECS` | `30` | Abort fetch after this many seconds |

### Docker / Cloud Run snippet

```bash
docker run \
  -e CLUSTER_ENABLED=true \
  -e CLUSTER_CONFIG_SOURCE=gcs \
  -e GCS_CONFIG_BUCKET=my-loadtest-configs \
  -e GCS_CONFIG_OBJECT=configs/prod-test.yaml \
  -e CLUSTER_CONFIG_TIMEOUT_SECS=30 \
  cbaugus/rust_loadtest:latest
```

### Object naming tips

- Slashes in object paths are percent-encoded automatically (`configs/prod.yaml`
  becomes `configs%2Fprod.yaml` in the request URL).  No manual escaping needed.
- Avoid spaces in object names; they encode to `%20` and work, but are harder
  to type in `gsutil` commands.

---

## Observability

On successful auto-fetch the leader logs:

```
INFO Leader committed auto-fetched config (Issue #76)
```

On failure:

```
ERROR Config fetch from external source failed  error="..."
ERROR Config fetch timed out                    timeout_secs=30
ERROR Failed to commit fetched config           error="..."
```

Set `RUST_LOG=rust_loadtest=debug` to see the individual HTTP requests made
during the fetch (token endpoint, GCS URL, Consul KV URL).

---

## Disabling auto-fetch

Leave `CLUSTER_CONFIG_SOURCE` unset (or remove it).  The cluster will still
form and elect a leader; config must be pushed manually:

```bash
curl -X POST http://<leader-ip>:9090/cluster/config \
  -H "Content-Type: application/x-yaml" \
  --data-binary @my-test-config.yaml
```
