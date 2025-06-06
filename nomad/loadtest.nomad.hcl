job "envoy-loadtest" {
  name        = "envoy-loadtest"
  datacenters = ["home"]
  type        = "service"
  #namespace   =

  constraint {
    attribute = "$${meta.node-switcher}"
    value = "on"
  }
  constraint {
    attribute = "$${meta.purpose}"
    operator  = "!="
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
      max_parallel = 1
      health_check = "checks"
      min_healthy_time = "10s"
      healthy_deadline = "10m"
      progress_deadline = "15m"
      auto_revert = true
    }



    network {
      mode = "host"
      # TODO: interpolate port delivery mechanism
      port "metrics" {
        # container
        to = 9090
        # host
        #static = 9080
      }
    }

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
      # TODO: interpolate driver
      driver = "docker"
      config {
        logging {
          type = "gelf"
          config {
            gelf-address = "udp://gelf.service.consul:12201"
          }
        }
        # TODO: interpolate image/repo/tags
        image = "cbaugus/rust_loadtest:latest"
        #command = "/usr/local/bin/rust_loadtest"
        ports = [
          "metrics",
        ]
      }


      template {
        destination = "secrets/config.env"
        env         = true
        data = <<EOH
TARGET_URL=http://envoy.service.consul/any_endpoint
NUM_CONCURRENT_TASKS=1000
TEST_DURATION=90d
LOAD_MODEL_TYPE=DailyTraffic
#Possible Values: "Concurrent", "Rps", "RampRps", "DailyTraffic"
DAILY_MIN_RPS="1000"
DAILY_MID_RPS="4000"
DAILY_MAX_RPS="15000"
DAILY_CYCLE_DURATION="30m" # Simulate one day in 1 hour

SKIP_TLS_VERIFY=true


# Ratios for a 1-hour cycle (sum up to 1.0)
MORNING_RAMP_RATIO="0.2"   # 12 minutes (0 to MAX RPS)
PEAK_SUSTAIN_RATIO="0.1"   # 6 minutes (MAX RPS)
MID_DECLINE_RATIO="0.2"    # 12 minutes (MAX RPS to MID RPS)
MID_SUSTAIN_RATIO="0.1"    # 6 minutes (MID RPS)
EVENING_DECLINE_RATIO="0.2" # 12 minutes (MID to MIN RPS)
# Remaining 0.2 ratio is Night Sustain (12 minutes at MIN RPS)



# If LOAD_MODEL_TYPE is "Rps":
  # TARGET_RPS: The fixed total requests per second (RPS) that the load test will attempt to maintain across all concurrent tasks.
# If LOAD_MODEL_TYPE is "RampRps":
  # MIN_RPS: The starting requests per second for the ramp-up phase.
  # MAX_RPS: The peak requests per second to reach during the test.
  # RAMP_DURATION: The total duration of the ramp profile. The ramp will typically increase to MAX_RPS over the first 1/3 of this duration, sustain for the next 1/3, and ramp down over the last 1/3.
  # Format: Same duration format as TEST_DURATION (e.g., "30m", "1h").
  # Default: The value of TEST_DURATION.
# If LOAD_MODEL_TYPE is "DailyTraffic":
  # DAILY_MIN_RPS: The base (e.g., night-time) requests per second.
  # DAILY_MID_RPS: The mid-level (e.g., afternoon) requests per second.
  # DAILY_MAX_RPS: The peak (e.g., morning rush) requests per second.
  # DAILY_CYCLE_DURATION: The duration of one complete daily traffic pattern cycle.
  # Format: Same duration format as TEST_DURATION.
  # MORNING_RAMP_RATIO: The proportion of DAILY_CYCLE_DURATION spent ramping up from DAILY_MIN_RPS to DAILY_MAX_RPS. Default: "0.125"
  # PEAK_SUSTAIN_RATIO: The proportion of DAILY_CYCLE_DURATION spent holding at DAILY_MAX_RPS. Default: "0.167"
  # MID_DECLINE_RATIO: The proportion of DAILY_CYCLE_DURATION spent declining from DAILY_MAX_RPS to DAILY_MID_RPS. Default: "0.125"
  # MID_SUSTAIN_RATIO: The proportion of DAILY_CYCLE_DURATION spent holding at DAILY_MID_RPS. Default: "0.167"
  # EVENING_DECLINE_RATIO: The proportion of DAILY_CYCLE_DURATION spent declining from DAILY_MID_RPS to DAILY_MIN_RPS. Default: "0.167"


EOH
      }


      resources {
        #cores = 1
        cpu = 5000
        memory = 2048
      }
    }
  }
}
