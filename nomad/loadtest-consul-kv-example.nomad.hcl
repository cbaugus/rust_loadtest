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
  # constraint {
  #   attribute = "$${meta.purpose}"
  #   operator  = "="
  #   value     = "ops"
  # }

  constraint {
    attribute = "$${meta.purpose}"
    operator  = "!="
    value     = "traefik"
  }

  group "envoy-loadtest" {
    count = 3

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

      # gRPC inter-node Raft/coordination port (CLUSTER_BIND_ADDR)
      port "cluster" {
        static = 7000
        to     = 7000
      }

      # HTTP cluster health endpoint (CLUSTER_HEALTH_ADDR)
      port "health" {
        static = 8080
        to     = 8080
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

    # Cluster discovery service — nodes find each other via
    # loadtest-cluster.service.consul when DISCOVERY_MODE=consul.
    # The rust_loadtest process also registers/updates its own state tags
    # (forming/follower/leader) through the Consul HTTP API, so this service
    # block just ensures the port and health check are registered by Nomad.
    service {
      name = "loadtest-cluster"
      tags = ["loadtest-cluster"]
      port = "cluster"
      check {
        #name     = "cluster-health-http"
        type     = "http"
        path     = "/health/cluster"
        port     = "health"
        interval = "30s"
        timeout  = "90s"
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
        image      = "cbaugus/rust_loadtest:dev-f16afdb"
        force_pull = true
        ports = [
          "metrics",
          "cluster",
          "health",
        ]
      }

      template {
        destination = "secrets/config.env"
        env         = true
        data        = <<EOH
# ── Config auto-fetch from Consul KV (Issue #76) ─────────────────────────────
# The elected leader reads this key and distributes the YAML to all nodes.
# Upload the config before deploying:
#   consul kv put loadtest/config @nomad/consul-kv-config-example.yaml
CLUSTER_CONFIG_SOURCE=consul-kv
CONSUL_CONFIG_KEY=loadtest/config
CLUSTER_CONFIG_TIMEOUT_SECS=30

# ── Startup defaults (required by Config::from_env at process start) ──────────
# IMPORTANT: TARGET_URL, NUM_CONCURRENT_TASKS, LOAD_MODEL_TYPE, and TARGET_RPS
# must be set even when CLUSTER_CONFIG_SOURCE=consul-kv.  Config::from_env()
# runs at startup before the Raft cluster forms and before the leader can fetch
# from Consul KV.  The values here are replaced cluster-wide as soon as the
# leader commits the Consul KV config to the Raft log.
TARGET_URL=http://dialtone.service.consul:5678
REQUEST_TYPE=GET
SKIP_TLS_VERIFY=true
NUM_CONCURRENT_TASKS=25
TEST_DURATION=72h
LOAD_MODEL_TYPE=Rps
TARGET_RPS=200

# ── Cluster (enable to run as a coordinated multi-node cluster) ───────────────
CLUSTER_ENABLED=true

# This node's address as seen by peers — MUST match the entry Consul resolves
# for this node (Consul mode) or the value in CLUSTER_NODES (static mode).
# NOMAD_IP_cluster is set by Nomad to the host IP bound to the "cluster" port.
CLUSTER_SELF_ADDR={{ env "NOMAD_IP_cluster" }}:7000

# Node identity label used in metrics and logs (does NOT drive Raft node ID —
# that comes from CLUSTER_SELF_ADDR above). Defaults to $HOSTNAME (alloc ID).
# CLUSTER_NODE_ID=

# gRPC bind address for Raft transport. Must match the "cluster" port above.
CLUSTER_BIND_ADDR=0.0.0.0:7000

# HTTP health/state endpoint. Must match the "health" port above.
CLUSTER_HEALTH_ADDR=0.0.0.0:8080

# ── Discovery mode — choose ONE ──────────────────────────────────────────────

# Consul-based peer discovery (recommended for Nomad).
# CLUSTER_MIN_PEERS = total cluster size minus 1.
# The code does min = CLUSTER_MIN_PEERS + 1 (includes self).
# This job sets count=3 → CLUSTER_MIN_PEERS=2 → waits for 3 total.
DISCOVERY_MODE=consul
CONSUL_ADDR=http://consul.service.consul:8500
CONSUL_SERVICE_NAME=loadtest-cluster
CLUSTER_MIN_PEERS=2
EOH
      }

      resources {
        cpu    = 8000
        memory = 8192
      }
    }
  }
}

