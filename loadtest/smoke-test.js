import http from 'k6/http';
import { check, sleep } from 'k6';
import encoding from 'k6/encoding';
import { crypto } from 'k6/experimental/webcrypto';

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

const smokeTests = [
    { name: 'Status Check', path: '/status', needsSignature: false },
    { name: 'Basic Resize', options: 'resize:fit:300:300' },
    { name: 'Format Conversion', options: 'resize:fit:200:200/format:webp' },
    { name: 'With Quality', options: 'resize:fit:250:250/quality:85' },
    { name: 'With Effect', options: 'resize:fit:300:300/blur:1' },
];

export default async function() {
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
