// MoxUI — Smoke Test
//
// Quick validation that the service is working (5 VUs, 30 seconds).
// Use after every deploy before running heavier tests.
//
// Usage:
//   K6_BASE_URL=http://localhost:8080 k6 run tests/k6/smoke_test.js

import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate } from 'k6/metrics';

const failures = new Rate('failed_requests');

const BASE_URL = __ENV.K6_BASE_URL || 'http://localhost:8080';

export const options = {
  vus: 5,
  duration: '30s',

  thresholds: {
    http_req_duration: ['p(95)<500'],
    http_req_failed: ['rate<0.01'],
    failed_requests: ['rate<0.01'],
  },
};

export default function () {
  // 1. Health endpoint
  group('Health', function () {
    const resp = http.get(`${BASE_URL}/health`);
    const ok = check(resp, {
      'health returns 200': (r) => r.status === 200,
    });
    if (!ok) failures.add(1);
  });

  sleep(1);

  // 2. Liveness
  group('Liveness', function () {
    const resp = http.get(`${BASE_URL}/livez`);
    const ok = check(resp, {
      'livez returns 200': (r) => r.status === 200,
    });
    if (!ok) failures.add(1);
  });

  sleep(1);

  // 3. Readiness
  group('Readiness', function () {
    const resp = http.get(`${BASE_URL}/readyz`);
    check(resp, {
      'readyz returns 200 or 503': (r) => r.status === 200 || r.status === 503,
    });
  });

  sleep(1);

  // 4. Prometheus metrics
  group('Metrics', function () {
    const resp = http.get(`${BASE_URL}/metrics`);
    const ok = check(resp, {
      'metrics returns 200': (r) => r.status === 200,
      'metrics content-type is text/plain': (r) => {
        const ct = r.headers['Content-Type'] || '';
        return ct.includes('text/plain');
      },
    });
    if (!ok) failures.add(1);
  });

  sleep(1);
}
