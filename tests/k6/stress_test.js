// MoxUI — Stress Test
//
// Step load up to 1000 concurrent VUs to find the breaking point.
// Increments by 100 VUs every 2 minutes, holding each level for 1 minute.
//
// Usage:
//   K6_BASE_URL=http://localhost:8080 k6 run tests/k6/stress_test.js

import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend } from 'k6/metrics';

const failures = new Rate('failed_requests');
const healthTrend = new Trend('health_duration_ms');

const BASE_URL = __ENV.K6_BASE_URL || 'http://localhost:8080';

export const options = {
  stages: [
    // Warm-up
    { duration: '1m', target: 10 },
    // Step up: 100 → 1000 VUs, 100 VUs every 3 minutes
    { duration: '2m', target: 100 },
    { duration: '1m', target: 100 },
    { duration: '2m', target: 200 },
    { duration: '1m', target: 200 },
    { duration: '2m', target: 300 },
    { duration: '1m', target: 300 },
    { duration: '2m', target: 400 },
    { duration: '1m', target: 400 },
    { duration: '2m', target: 500 },
    { duration: '1m', target: 500 },
    { duration: '2m', target: 600 },
    { duration: '1m', target: 600 },
    { duration: '2m', target: 700 },
    { duration: '1m', target: 700 },
    { duration: '2m', target: 800 },
    { duration: '1m', target: 800 },
    { duration: '2m', target: 900 },
    { duration: '1m', target: 900 },
    { duration: '2m', target: 1000 },
    { duration: '3m', target: 1000 },  // Hold at peak
    // Cool-down
    { duration: '2m', target: 0 },
  ],

  thresholds: {
    http_req_duration: ['p(95)<5000'],
    http_req_failed: ['rate<0.05'],  // More lenient under stress
    failed_requests: ['rate<0.05'],
  },
};

export default function () {
  group('Health Check', function () {
    const resp = http.get(`${BASE_URL}/health`);
    const ok = check(resp, {
      'health returns 200': (r) => r.status === 200,
    });
    healthTrend.add(resp.timings.duration);
    if (!ok) failures.add(1);
  });

  // Vary the think time to avoid thundering-herd
  sleep(Math.random() * 3 + 0.5);
}
