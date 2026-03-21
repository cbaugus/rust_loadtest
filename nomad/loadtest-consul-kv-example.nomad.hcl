# Cluster load test — Consul KV config auto-fetch example (Issue #76)
#
# Each node runs 25 workers at 200 RPS.  For a 3-node cluster that gives
# 75 concurrent workers and 600 RPS aggregate across the cluster.
#
# ── Prerequisites ─────────────────────────────────────────────────────────────
#
# 1. Upload the test config to Consul KV before deploying this job:
#
#      consul kv put loadtest/config @nomad/consul-kv-config-example.yaml
#
#    Or inline:
#
#      consul kv put loadtest/config - <<'EOF'
#      version: "1.0"
#      config:
#        baseUrl: "http://your-service.service.consul:8080"
#        workers: 25
#        duration: "1h"
#        timeout: "30s"
#        skipTlsVerify: false
#      load:
#        model: "rps"
#        target: 200
#      scenarios:
#        - name: "Cluster load test"
#          weight: 100
#          steps:
#            - name: "GET /"
#              request:
#                method: "GET"
#                path: "/"
#              assertions:
#                - type: statusCode
#                  expected: 200
#      EOF
#
# 2. Verify the key is present:
#
#      consul kv get loadtest/config
#
# 3. Deploy this job:
#
#      nomad job run nomad/loadtest-consul-kv-example.nomad.hcl
#
# ── Notes ─────────────────────────────────────────────────────────────────────
#
# - CLUSTER_MIN_PEERS = total cluster size minus 1.
#   The code does min = CLUSTER_MIN_PEERS + 1 internally (to include self).
#   This job sets count=3, so CLUSTER_MIN_PEERS=2 → min=3 → wait for 3 total.
#
# - The elected leader logs:
#     INFO Leader committed auto-fetched config (Issue #76)
#   once the config is fetched from Consul and distributed to all nodes.
#
# - To push a new config without restarting, update the KV key and then
#   POST it to the current leader's /cluster/config endpoint:
#
#      LEADER=$(consul catalog nodes -service=loadtest-cluster -tag=leader \
#               -format=json | jq -r '.[0].Address')
#      curl -X POST http://${LEADER}:8080/cluster/config \
#           -H "Content-Type: application/x-yaml" \
#           --data-binary @nomad/consul-kv-config-example.yaml
#
# ─────────────────────────────────────────────────────────────────────────────

job "envoy-loadtest" {
  name        = "envoy-loadtest"
  datacenters = ["home"]
  type        = "service"
  namespace   = "thd"

  constraint {
    attribute = "$${meta.node-switcher}"
    value = "on"
  }
  constraint {
    attribute = "$${meta.purpose}"
    operator  = "="
    value     = "worker"
  }

  constraint {
    attribute = "$${meta.purpose}"
    operator  = "!="
    value     = "traefik"
  }

  group "envoy-loadtest" {
    count = 2

    scaling {
      enabled = true
      min     = 1
      max     = 20
    }

    update {
      max_parallel      = 1
      health_check      = "checks"
      min_healthy_time  = "10s"
      healthy_deadline  = "10m"
      progress_deadline = "15m"
      auto_revert       = true
    }

    network {
      mode = "host"

      port "metrics" {
        static = 9099
        to = 9090
      }

      # HTTP cluster health endpoint (CLUSTER_HEALTH_ADDR)
      port "health" {
        static = 8080
        to     = 8080
      }
    }

    service {
      name = "load-health"
      tags = ["load-health"]
      port = "health"
      # /ready is unauthenticated and always returns 200 — safe for Nomad probes
      # even when HEALTH_AUTH_ENABLED=true protects the full /health endpoint.
      check {
        type     = "http"
        path     = "/ready"
        port     = "health"
        interval = "10s"
        timeout  = "6s"
      }
    }

    # Prometheus metrics service (existing)
    service {
      name = "load-metrics"
      tags = ["load-metrics"]
      port = "metrics"
      check {
        type     = "tcp"
        port     = "metrics"
        interval = "10s"
        timeout  = "6s"
      }
    }

     task "envoy-loadtest" {
      driver = "docker"
      config {
        logging {
          type = "gelf"
          config {
            gelf-address = "udp://gelf.service.consul:12201"
          }
        }
        image      = "cbaugus/rust_loadtest:dev-c6d1e3e"
        force_pull = true
        ports = [
          "metrics",
          "health",
        ]
      }

      template {
        destination = "secrets/config.env"
        env         = true
        data        = <<EOH
# ── Startup defaults (required by Config::from_env at process start) ──────────
# IMPORTANT: TARGET_URL, NUM_CONCURRENT_TASKS, LOAD_MODEL_TYPE, and TARGET_RPS
# must be set even when CLUSTER_CONFIG_SOURCE=consul-kv.  Config::from_env()
# runs at startup before the Raft cluster forms and before the leader can fetch
# from Consul KV.  The values here are replaced cluster-wide as soon as the
# leader commits the Consul KV config to the Raft log.
RUST_LOG=rust_loadtest=warn,hyper=error,reqwest=error
# ── Logging format (Issue #101) ───────────────────────────────────────────────
# LOG_FORMAT=json emits one structured JSON object per line — required for
# FluentBit to unpack fields into GCP Cloud Logging jsonPayload.
# Omit or set to any other value for human-readable output (local dev).
LOG_FORMAT=json
TARGET_URL=http://dialtone.service.consul:5678
REQUEST_TYPE=GET
SKIP_TLS_VERIFY=true
NUM_CONCURRENT_TASKS=300
TEST_DURATION=2h
LOAD_MODEL_TYPE=Rps
TARGET_RPS=0

# ── Tenant label ─────────────────────────────────────────────────────────────
# Applied to all Prometheus metrics emitted by this node from startup.
# Overridden per-test by metadata.tenant in a POST /config YAML body.
# TENANT=acme

# ── Security (Issue #91 / #92) ────────────────────────────────────────────────
# Uncomment to protect POST /config and POST /stop with a bearer token:
# API_AUTH_TOKEN=your-secret-token-here
#
# Uncomment to also protect GET /health (GET /ready is always open for Nomad):
# HEALTH_AUTH_ENABLED=true

# ── Node auto-registration with web app (Issue #89) ──────────────────────────
# All three vars must be set to enable registration; missing any one is a no-op.
# NODE_REGISTRY_URL=https://loadtest-control.example.com
# AUTO_REGISTER_PSK=shared-secret-across-all-nodes
# NODE_BASE_URL=http://{{env "attr.unique.network.ip-address"}}:8080
# NODE_NAME={{env "node.unique.name"}}
# NODE_TAGS={"env":"staging","datacenter":"{{env "node.datacenter"}}"}

# ── Ephemeral node / GCP one-shot (Issue #98) ────────────────────────────
# EPHEMERAL=true starts the node in "ready" state (no startup workers).
# Workers only launch after POST /config is received from the control plane.
# On test completion the node transitions to "idle" (skips standby) and
# executes SELF_DESTRUCT_CMD to delete the instance.
# EPHEMERAL=true
# EPHEMERAL_FINAL_SCRAPE_DELAY=60s
# SELF_DESTRUCT_CMD=shutdown -h now

EOH
      }

      resources {
        cpu    = 11400
        memory = 20000
      }
    }
  }
}

