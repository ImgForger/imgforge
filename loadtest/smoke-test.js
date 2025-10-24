import http from 'k6/http';
import { check, sleep } from 'k6';
import encoding from 'k6/encoding';
import { TextEncoder } from "https://raw.githubusercontent.com/inexorabletash/text-encoding/master/index.js";


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

function hexToBytes(hexString) {
    const normalized = hexString.trim().replace(/^0x/, '');

    if (normalized.length === 0) {
        return new Uint8Array([]);
    }

    if (normalized.length % 2 !== 0) {
        throw new Error('IMGFORGE_KEY and IMGFORGE_SALT must be valid hex strings');
    }

    const bytes = new Uint8Array(normalized.length / 2);
    for (let i = 0; i < normalized.length; i += 2) {
        const byte = parseInt(normalized.slice(i, i + 2), 16);
        if (Number.isNaN(byte)) {
            throw new Error('IMGFORGE_KEY and IMGFORGE_SALT must be valid hex strings');
        }
        bytes[i / 2] = byte;
    }

    return bytes;
}

async function generateSignature(path) {
    if (USE_UNSIGNED) {
        return 'unsafe';
    }

    const subtle = globalThis.crypto && globalThis.crypto.subtle;
    if (!subtle) {
        throw new Error('Web Crypto API is not available in this environment.');
    }

    const keyBytes = hexToBytes(HMAC_KEY);
    const saltBytes = hexToBytes(HMAC_SALT);
    const pathBytes = new TextEncoder().encode(path);

    const payload = new Uint8Array(saltBytes.length + pathBytes.length);
    payload.set(saltBytes);
    payload.set(pathBytes, saltBytes.length);

    const cryptoKey = await subtle.importKey(
        'raw',
        keyBytes.buffer,
        { name: 'HMAC', hash: 'SHA-256' },
        false,
        ['sign']
    );

    const digest = await subtle.sign('HMAC', cryptoKey, payload.buffer);
    return encoding.b64encode(new Uint8Array(digest), 'rawurl');
}

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
