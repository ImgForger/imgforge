import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import encoding from 'k6/encoding';
import { crypto } from 'k6/experimental/webcrypto';

// Custom metrics
const errorRate = new Rate('errors');
const processingDuration = new Trend('processing_duration');

// Stress test configuration - gradually increase load to find breaking point
export const options = {
    stages: [
        { duration: '2m', target: 50 },   // Ramp up to 50 users
        { duration: '3m', target: 100 },  // Ramp up to 100 users
        { duration: '5m', target: 100 },  // Stay at 100 users
        { duration: '2m', target: 150 },  // Push to 150 users
        { duration: '5m', target: 150 },  // Stay at 150 users
        { duration: '2m', target: 200 },  // Push to 200 users
        { duration: '3m', target: 200 },  // Stay at 200 users
        { duration: '2m', target: 0 },    // Ramp down
    ],
    thresholds: {
        'http_req_duration': ['p(95)<10000', 'p(99)<15000'],
        'http_req_failed': ['rate<0.1'],
        'errors': ['rate<0.1'],
    },
};

const BASE_URL = __ENV.IMGFORGE_URL || 'http://localhost:3000';
const HMAC_KEY = __ENV.IMGFORGE_KEY || '';
const HMAC_SALT = __ENV.IMGFORGE_SALT || '';
const USE_UNSIGNED = __ENV.IMGFORGE_ALLOW_UNSIGNED === 'true';
const TEST_IMAGE_URL = __ENV.TEST_IMAGE_URL || 'https://picsum.photos/800/600';

async function generateSignature(path) {
    if (USE_UNSIGNED) {
        return 'unsafe';
    }

    const keyBytes = encoding.b64decode(encoding.b64encode(HMAC_KEY), 'rawstd');
    const saltBytes = encoding.b64decode(encoding.b64encode(HMAC_SALT), 'rawstd');

    const key = await crypto.subtle.importKey(
        'raw',
        keyBytes,
        { name: 'HMAC', hash: 'SHA-256' },
        false,
        ['sign']
    );

    const encoder = new TextEncoder();
    const dataToSign = new Uint8Array([...saltBytes, ...encoder.encode(path)]);

    const signature = await crypto.subtle.sign('HMAC', key, dataToSign);
    const base64 = encoding.b64encode(new Uint8Array(signature), 'rawurl');
    return base64;
}

// Focus on realistic, high-impact scenarios for stress testing
const scenarios = [
    {
        name: 'Standard Thumbnail',
        options: 'resize:fit:200:200/quality:80',
        weight: 30  // Most common
    },
    {
        name: 'Medium Size',
        options: 'resize:fit:500:500/quality:85',
        weight: 25
    },
    {
        name: 'Large Size',
        options: 'resize:fit:1000:1000/quality:90',
        weight: 15
    },
    {
        name: 'WebP Conversion',
        options: 'resize:fit:400:400/format:webp/quality:85',
        weight: 10
    },
    {
        name: 'With Blur',
        options: 'resize:fit:400:400/blur:2',
        weight: 5
    },
    {
        name: 'With Sharpen',
        options: 'resize:fit:600:600/sharpen:1.5',
        weight: 5
    },
    {
        name: 'Complex Processing',
        options: 'resize:fill:800:600/gravity:center/quality:88/sharpen:1/background:FFFFFF',
        weight: 5
    },
    {
        name: 'High DPR',
        options: 'resize:fit:300:300/dpr:3/quality:85',
        weight: 5
    }
];

// Weighted random selection based on realistic usage patterns
function selectScenario() {
    const totalWeight = scenarios.reduce((sum, s) => sum + s.weight, 0);
    let random = Math.random() * totalWeight;

    for (const scenario of scenarios) {
        random -= scenario.weight;
        if (random <= 0) {
            return scenario;
        }
    }
    return scenarios[0];
}

export default async function () {
    const scenario = selectScenario();

    const processingPath = `/${scenario.options}/plain/${TEST_IMAGE_URL}`;
    const signature = await generateSignature(processingPath);
    const fullPath = `/${signature}${processingPath}`;
    const url = `${BASE_URL}${fullPath}`;

    const startTime = Date.now();
    const response = http.get(url, {
        tags: { scenario: scenario.name },
    });
    const duration = Date.now() - startTime;

    processingDuration.add(duration);

    const success = check(response, {
        'status is 200': (r) => r.status === 200,
        'has body': (r) => r.body && r.body.length > 0,
    });

    if (!success) {
        errorRate.add(1);
        if (__ITER % 50 === 0) {  // Log every 50th error to avoid spam
            console.log(`Error [${scenario.name}]: Status ${response.status}`);
        }
    } else {
        errorRate.add(0);
    }

    // Minimal sleep to maximize stress
    sleep(Math.random() * 0.5);
}

export function setup() {
    console.log('=== Stress Test Configuration ===');
    console.log(`Target: ${BASE_URL}`);
    console.log('Load Pattern: 0 → 50 → 100 → 150 → 200 users over 24 minutes');
    console.log('WARNING: This test will generate significant load');
    console.log('=====================================');

    const response = http.get(`${BASE_URL}/status`);
    if (response.status !== 200) {
        throw new Error('Server not available');
    }
    console.log('✓ Server is ready');
}

export function teardown(data) {
    console.log('=== Stress Test Complete ===');
    console.log('Review metrics above to identify breaking points and bottlenecks');
}
