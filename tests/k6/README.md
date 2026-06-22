# MoxUI — k6 Load Tests

This directory contains k6 performance and load-testing scripts for MoxUI.

## Prerequisites

### Install k6

**macOS (Homebrew):**
```bash
brew install k6
```

**Linux (Debian/Ubuntu):**
```bash
sudo gpg -k
sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update && sudo apt-get install k6
```

**Docker:**
```bash
docker pull grafana/k6
```

**Windows (Chocolatey):**
```powershell
choco install k6
```

**Other:** See the [official installation guide](https://k6.io/docs/getting-started/installation/).

## Test Scripts

| Script | Description | Default VUs | Duration |
|---|---|---|---|
| `smoke_test.js` | Quick smoke test after deploy — validates basic endpoints | 5 | 30s |
| `load_test.js` | Realistic load test — simulates user flows (health → login → VMs → dashboard) | 500 | 10 min |
| `stress_test.js` | Step-load stress test — ramps up to find the breaking point | 1000 (peak) | ~35 min |

## Running Tests

### Local k6

```bash
# Smoke test (point at your MoxUI instance)
K6_BASE_URL=http://localhost:8080 k6 run tests/k6/smoke_test.js

# Load test with defaults (500 VUs, 10 min)
K6_BASE_URL=http://localhost:8080 k6 run tests/k6/load_test.js

# Load test with custom parameters
K6_BASE_URL=http://localhost:8080 \
  K6_VUS=100 \
  K6_DURATION=5m \
  K6_AUTH=true \
  K6_USER=admin \
  K6_PASS=secret \
  k6 run tests/k6/load_test.js

# Stress test
K6_BASE_URL=http://localhost:8080 k6 run tests/k6/stress_test.js
```

### Docker

```bash
# Smoke test
docker run --rm -i \
  -e K6_BASE_URL=http://host.docker.internal:8080 \
  grafana/k6 run - <tests/k6/smoke_test.js

# Load test
docker run --rm -i \
  -e K6_BASE_URL=http://host.docker.internal:8080 \
  -e K6_VUS=500 \
  -e K6_DURATION=10m \
  grafana/k6 run - <tests/k6/load_test.js

# Stress test
docker run --rm -i \
  -e K6_BASE_URL=http://host.docker.internal:8080 \
  grafana/k6 run - <tests/k6/stress_test.js
```

### Docker with mounted volume (for HTML reports)

```bash
docker run --rm -i \
  -e K6_BASE_URL=http://host.docker.internal:8080 \
  -v /tmp/k6-results:/results \
  grafana/k6 run --summary-export=/results/summary.json \
    - <tests/k6/load_test.js
```

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `K6_BASE_URL` | `http://localhost:8080` | Target MoxUI instance URL |
| `K6_VUS` | `500` | Target virtual users (load_test.js only) |
| `K6_DURATION` | `10m` | Hold duration at peak VUs (load_test.js only) |
| `K6_AUTH` | `true` | Enable/disable login + authenticated requests |
| `K6_USER` | `admin` | Username for login |
| `K6_PASS` | `admin123` | Password for login |

## Interpreting Results

Key metrics to watch:

- **http_req_duration** — Request latency. p(95) < 2s is healthy for load; < 500ms for smoke.
- **http_req_failed** — Error rate. Should be < 1% for load tests, < 5% under stress.
- **login_failures** (load test) — Rate of failed login attempts. Should be < 5%.
- **api_errors** (load test) — Rate of non-2xx API responses. Should be < 1%.

## CI/CD Integration

```bash
# Run smoke test in CI pipeline
k6 run --quiet tests/k6/smoke_test.js

# Run load test with JSON output for analysis
k6 run --summary-export=/tmp/k6-results.json tests/k6/load_test.js
```
