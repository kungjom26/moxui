// MoxUI — Load Test
//
// Simulates realistic user behaviour: health check → login → list VMs → dashboard
// Configurable via environment variables.
//
// Usage:
//   K6_BASE_URL=http://localhost:8080 k6 run tests/k6/load_test.js
//   K6_BASE_URL=http://localhost:8080 K6_USER=admin K6_PASS=secret k6 run tests/k6/load_test.js

import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// ── Custom metrics ──────────────────────────────────────────────────────
const loginFailures = new Rate('login_failures');
const apiErrors = new Rate('api_errors');
const healthTrend = new Trend('health_duration_ms');
const loginTrend = new Trend('login_duration_ms');
const vmsTrend = new Trend('vms_list_duration_ms');
const dashTrend = new Trend('dashboard_duration_ms');

// ── Configuration ───────────────────────────────────────────────────────
const BASE_URL = __ENV.K6_BASE_URL || 'http://localhost:8080';
const DEFAULT_VUS = __ENV.K6_VUS ? parseInt(__ENV.K6_VUS, 10) : 500;
const DEFAULT_DURATION = __ENV.K6_DURATION || '10m';
const AUTH_ENABLED = (__ENV.K6_AUTH || 'true') === 'true';
const USERNAME = __ENV.K6_USER || 'admin';
const PASSWORD = __ENV.K6_PASS || 'admin123';

export const options = {
  stages: [
    // Ramp up gradually to target VUs
    { duration: '2m', target: Math.ceil(DEFAULT_VUS * 0.25) },
    { duration: '2m', target: Math.ceil(DEFAULT_VUS * 0.50) },
    { duration: '2m', target: Math.ceil(DEFAULT_VUS * 0.75) },
    { duration: '2m', target: DEFAULT_VUS },
    // Hold at target
    { duration: DEFAULT_DURATION, target: DEFAULT_VUS },
    // Ramp down
    { duration: '2m', target: 0 },
  ],

  thresholds: {
    http_req_duration: ['p(95)<2000', 'p(99)<5000'],
    http_req_failed: ['rate<0.01'],
    login_failures: ['rate<0.05'],
    api_errors: ['rate<0.01'],
  },
};

// ── Helpers ─────────────────────────────────────────────────────────────
function randomSleep(base, jitter) {
  sleep(base + Math.random() * jitter);
}

// ── Setup (runs once per VU) ────────────────────────────────────────────
export function setup() {
  // Probe the base URL before starting VUs
  const probe = http.get(`${BASE_URL}/health`);
  check(probe, {
    'setup: health endpoint reachable': (r) => r.status === 200,
  });
  console.log(`Setup complete — target ${BASE_URL}`);
  return { authToken: null };
}

// ── Main VU loop ────────────────────────────────────────────────────────
export default function (data) {
  // 1. Health check ──────────────────────────────────────────────────
  group('Health Check', function () {
    const resp = http.get(`${BASE_URL}/health`);
    const ok = check(resp, {
      'health status is 200': (r) => r.status === 200,
      'health returns json': (r) => r.headers['Content-Type'] === 'application/json',
    });
    healthTrend.add(resp.timings.duration);
    if (!ok) apiErrors.add(1);
  });

  randomSleep(0.5, 1.5);

  // 2. Login (if auth enabled) ───────────────────────────────────────
  let token = data.authToken;
  if (AUTH_ENABLED) {
    group('Login', function () {
      const payload = JSON.stringify({
        username: USERNAME,
        password: PASSWORD,
      });
      const params = {
        headers: { 'Content-Type': 'application/json' },
        tags: { name: 'login' },
      };
      const resp = http.post(`${BASE_URL}/api/v1/auth/login`, payload, params);
      const ok = check(resp, {
        'login status is 200': (r) => r.status === 200,
        'login returns token': (r) => {
          try {
            const body = JSON.parse(r.body);
            token = body.token || body.access_token || null;
            return token !== null;
          } catch {
            return false;
          }
        },
      });
      loginTrend.add(resp.timings.duration);
      if (!ok) loginFailures.add(1);

      randomSleep(0.3, 1.0);
    });
  }

  // 3. List VMs (authenticated) ──────────────────────────────────────
  group('List VMs', function () {
    const params = {
      headers: {},
      tags: { name: 'vms_list' },
    };
    if (token) {
      params.headers['Authorization'] = `Bearer ${token}`;
    }
    const resp = http.get(`${BASE_URL}/api/v1/vms`, params);
    const ok = check(resp, {
      'vms list status is 2xx': (r) => r.status >= 200 && r.status < 300,
    });
    vmsTrend.add(resp.timings.duration);
    if (!ok) apiErrors.add(1);
  });

  randomSleep(0.5, 2.0);

  // 4. Dashboard endpoint (authenticated) ────────────────────────────
  group('Dashboard', function () {
    const params = {
      headers: {},
      tags: { name: 'dashboard' },
    };
    if (token) {
      params.headers['Authorization'] = `Bearer ${token}`;
    }
    const resp = http.get(`${BASE_URL}/api/v1/dashboard`, params);
    const ok = check(resp, {
      'dashboard status is 2xx': (r) => r.status >= 200 && r.status < 300,
      'dashboard returns json': (r) => {
        const ct = r.headers['Content-Type'] || '';
        return ct.includes('application/json');
      },
    });
    dashTrend.add(resp.timings.duration);
    if (!ok) apiErrors.add(1);
  });

  // 5. Liveness probe (k8s-style, exercised more frequently) ─────────
  if (Math.random() < 0.3) {
    group('Liveness Probe', function () {
      const resp = http.get(`${BASE_URL}/livez`);
      check(resp, {
        'livez status is 200': (r) => r.status === 200,
      });
    });
  }

  // Think time between iterations
  randomSleep(1.0, 3.0);
}

// ── Teardown ────────────────────────────────────────────────────────────
export function teardown(data) {
  console.log('Load test complete.');
}
