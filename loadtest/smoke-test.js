import http from 'k6/http';
import { check, sleep } from 'k6';
import { generateSignature } from './common.js';

// Smoke test configuration - quick validation with minimal load
export const options = {
    vus: 3,
    duration: '30s',
    thresholds: {
        'http_req_duration': ['p(95)<5000'],
        'http_req_failed': ['rate<0.1'],
    },
};

const BASE_URL = __ENV.IMGFORGE_URL || 'http://localhost:3000';
const TEST_IMAGE_URL = __ENV.TEST_IMAGE_URL || 'https://picsum.photos/800/600';

const smokeTests = [
    { name: 'Status Check', path: '/status', needsSignature: false },
    { name: 'Basic Resize', options: 'resize:fit:300:300' },
    { name: 'Format Conversion', options: 'resize:fit:200:200/format:webp' },
    { name: 'With Quality', options: 'resize:fit:250:250/quality:85' },
    { name: 'With Effect', options: 'resize:fit:300:300/blur:1' },
];

export default async function () {
    for (const test of smokeTests) {
        if (test.needsSignature === false) {
            // Direct endpoint test
            const response = http.get(`${BASE_URL}${test.path}`);
            check(response, {
                [`${test.name}: status is 200`]: (r) => r.status === 200,
            });
        } else {
            // Processing endpoint test
            const processingPath = `/${test.options}/plain/${TEST_IMAGE_URL}`;
            const signature = await generateSignature(processingPath);
            const fullPath = `/${signature}${processingPath}`;
            const url = `${BASE_URL}${fullPath}`;

            const response = http.get(url);
            check(response, {
                [`${test.name}: status is 200`]: (r) => r.status === 200,
                [`${test.name}: has body`]: (r) => r.body && r.body.length > 0,
            });
        }
        sleep(0.5);
    }
}

export function setup() {
    console.log('=== Smoke Test Starting ===');
    console.log(`Testing: ${BASE_URL}`);

    const response = http.get(`${BASE_URL}/status`);
    if (response.status !== 200) {
        throw new Error('Server not available');
    }
    console.log('✓ Server is responding');
}
