import http from 'k6/http';
import { check, group, sleep } from 'k6';
import { Trend } from 'k6/metrics';
import encoding from 'k6/encoding';
import { crypto } from 'k6/experimental/webcrypto';

// Metrics for cache performance comparison
const firstRequestDuration = new Trend('first_request_duration');
const cachedRequestDuration = new Trend('cached_request_duration');

// Test configuration - moderate load to measure cache effectiveness
export const options = {
    stages: [
        { duration: '30s', target: 10 },
        { duration: '2m', target: 15 },
        { duration: '30s', target: 0 },
    ],
    thresholds: {
        'first_request_duration': ['avg<3000'],
        'cached_request_duration': ['avg<500'],  // Cached should be much faster
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

// Fixed set of transformations to test cache effectiveness
const cacheableScenarios = [
    { name: 'Thumbnail 200x200', options: 'resize:fit:200:200/quality:80' },
    { name: 'Medium 500x500', options: 'resize:fit:500:500/quality:85' },
    { name: 'Large 800x800', options: 'resize:fit:800:800/quality:90' },
    { name: 'WebP 400x400', options: 'resize:fit:400:400/format:webp/quality:85' },
    { name: 'Square Crop', options: 'resize:fill:300:300/gravity:center' },
    { name: 'With Blur', options: 'resize:fit:350:350/blur:1.5' },
    { name: 'With Sharpen', options: 'resize:fit:450:450/sharpen:1' },
    { name: 'High Quality PNG', options: 'resize:fit:400:400/format:png' },
];

let cacheWarmedUp = false;
let warmupComplete = false;

export default async function () {
    // First iteration for each VU: warm up the cache
    if (__ITER === 0 && !warmupComplete) {
        await group('Cache Warmup', async function () {
            for (const scenario of cacheableScenarios) {
                const processingPath = `/${scenario.options}/plain/${TEST_IMAGE_URL}`;
                const signature = await generateSignature(processingPath);
                const url = `${BASE_URL}/${signature}${processingPath}`;

                const response = http.get(url, { tags: { type: 'warmup' } });
                check(response, {
                    'warmup successful': (r) => r.status === 200,
                });
                sleep(0.2);
            }
        });
        warmupComplete = true;
        sleep(2);  // Let cache settle
        return;
    }

    // Select a random scenario from the cacheable set
    const scenario = cacheableScenarios[Math.floor(Math.random() * cacheableScenarios.length)];

    await group(scenario.name, async function () {
        const processingPath = `/${scenario.options}/plain/${TEST_IMAGE_URL}`;
        const signature = await generateSignature(processingPath);
        const url = `${BASE_URL}/${signature}${processingPath}`;

        // Make request
        const startTime = Date.now();
        const response = http.get(url, {
            tags: {
                scenario: scenario.name,
                type: 'cached'
            },
        });
        const duration = Date.now() - startTime;

        // Track based on whether it was cached
        const xCacheStatus = response.headers['X-Cache-Status'];
        if (xCacheStatus === 'HIT') {
            cachedRequestDuration.add(duration);
        } else {
            firstRequestDuration.add(duration);
        }

        // Check response
        check(response, {
            'status is 200': (r) => r.status === 200,
            'has body': (r) => r.body && r.body.length > 0,
        });

        // Log cache statistics periodically
        if (__ITER % 100 === 0 && __VU === 1) {
            console.log(`[Iter ${__ITER}] Cache status: ${xCacheStatus || 'UNKNOWN'}, Duration: ${duration}ms`);
        }
    });

    sleep(Math.random() * 1 + 0.5);
}

export function setup() {
    console.log('=== Cache Performance Test ===');
    console.log(`Server: ${BASE_URL}`);
    console.log('This test measures cache effectiveness by:');
    console.log('1. Warming up cache with common transformations');
    console.log('2. Repeatedly requesting the same transformations');
    console.log('3. Comparing first-request vs cached-request performance');
    console.log('=====================================');

    const response = http.get(`${BASE_URL}/status`);
    if (response.status !== 200) {
        throw new Error('Server not available');
    }
    console.log('✓ Server is ready');

    console.log('\nNOTE: Ensure imgforge is configured with caching enabled');
    console.log('(IMGFORGE_CACHE_TYPE=memory or hybrid or disk)');
    console.log('=====================================\n');
}

export function teardown(data) {
    console.log('\n=== Cache Performance Results ===');
    console.log('Compare metrics:');
    console.log('- first_request_duration: Time for uncached requests');
    console.log('- cached_request_duration: Time for cached requests');
    console.log('- Large difference indicates good cache performance');
    console.log('===================================');
}
