import http from 'k6/http';
import { check, group, sleep } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';
import encoding from 'k6/encoding';
import { generateSignature } from './common.js';

// Custom metrics
const errorRate = new Rate('errors');
const processingDuration = new Trend('processing_duration');
const cacheMisses = new Counter('cache_misses');
const cacheHits = new Counter('cache_hits');

// Test configuration
export const options = {
    stages: [
        { duration: '30s', target: 10 },  // Ramp up to 10 users
        { duration: '1m', target: 20 },   // Ramp up to 20 users
        { duration: '2m', target: 20 },   // Stay at 20 users
        { duration: '30s', target: 0 },   // Ramp down to 0 users
    ],
    thresholds: {
        'http_req_duration': ['p(95)<3000', 'p(99)<5000'],
        'http_req_failed': ['rate<0.05'],
        'errors': ['rate<0.05'],
    },
};

// Configuration from environment variables
const BASE_URL = __ENV.IMGFORGE_URL || 'http://localhost:3000';
const USE_UNSIGNED = __ENV.IMGFORGE_ALLOW_UNSIGNED === 'true';
const TEST_IMAGE_URL = __ENV.TEST_IMAGE_URL || 'https://picsum.photos/800/600';

// Helper function to encode URL to base64url
function encodeUrlToBase64(url) {
    return encoding.b64encode(url, 'rawurl');
}

// Test scenarios with different processing parameters
const scenarios = [
    {
        name: 'Basic Resize - Fill',
        options: 'resize:fill:300:200',
        description: 'Resize to 300x200 with fill mode'
    },
    {
        name: 'Basic Resize - Fit',
        options: 'resize:fit:400:300',
        description: 'Resize to 400x300 with fit mode'
    },
    {
        name: 'Size with Quality',
        options: 'size:500:500/quality:90',
        description: 'Resize to 500x500 with 90% quality'
    },
    {
        name: 'Width Only',
        options: 'width:600',
        description: 'Resize to width 600, maintain aspect ratio'
    },
    {
        name: 'Height Only',
        options: 'height:400',
        description: 'Resize to height 400, maintain aspect ratio'
    },
    {
        name: 'Resize with Blur',
        options: 'resize:fit:350:350/blur:2',
        description: 'Resize and apply blur effect'
    },
    {
        name: 'Resize with Sharpen',
        options: 'resize:fit:450:450/sharpen:1.5',
        description: 'Resize and sharpen'
    },
    {
        name: 'Format Conversion - WebP',
        options: 'resize:fit:400:400/format:webp/quality:85',
        description: 'Resize and convert to WebP format'
    },
    {
        name: 'Format Conversion - PNG',
        options: 'resize:fit:300:300/format:png',
        description: 'Resize and convert to PNG format'
    },
    {
        name: 'Crop and Resize',
        options: 'crop:100:100:400:400/resize:fit:250:250',
        description: 'Crop region then resize'
    },
    {
        name: 'Rotate 90',
        options: 'resize:fit:350:350/rotate:90',
        description: 'Resize and rotate 90 degrees'
    },
    {
        name: 'With Gravity',
        options: 'resize:fill:400:400/gravity:north_east',
        description: 'Resize with north-east gravity'
    },
    {
        name: 'With Padding',
        options: 'resize:fit:300:300/padding:20',
        description: 'Resize with 20px padding on all sides'
    },
    {
        name: 'With Background Color',
        options: 'resize:fit:350:350/extend:true/background:FF0000',
        description: 'Resize with red background extension'
    },
    {
        name: 'DPR Scaling',
        options: 'resize:fit:200:200/dpr:2',
        description: 'Resize with 2x device pixel ratio'
    },
    {
        name: 'Min Width and Height',
        options: 'resize:fit:150:150/min_width:250/min_height:250',
        description: 'Resize with minimum dimensions'
    },
    {
        name: 'Zoom Effect',
        options: 'resize:fit:300:300/zoom:1.5',
        description: 'Resize with 1.5x zoom'
    },
    {
        name: 'Pixelate Effect',
        options: 'resize:fit:400:400/pixelate:10',
        description: 'Resize with pixelation effect'
    },
    {
        name: 'Complex Multi-Option',
        options: 'resize:fill:500:400/gravity:center/quality:88/sharpen:1/background:FFFFFF',
        description: 'Multiple options combined'
    },
    {
        name: 'Enlarge and Extend',
        options: 'resize:fit:1000:1000:true:true/background:00FF00',
        description: 'Resize with enlarge and extend enabled'
    },
    {
        name: 'Auto Rotate Disabled',
        options: 'resize:fit:350:350/auto_rotate:false',
        description: 'Resize without auto-rotation'
    },
    {
        name: 'High Quality JPEG',
        options: 'resize:fit:600:600/format:jpeg/quality:95',
        description: 'High quality JPEG output'
    },
    {
        name: 'Low Quality for Thumbnails',
        options: 'resize:fit:150:150/quality:70',
        description: 'Low quality for thumbnail generation'
    },
    {
        name: 'Square Crop Fill',
        options: 'resize:fill:400:400/gravity:center',
        description: 'Square crop with center gravity'
    },
    {
        name: 'Resize with Nearest Algorithm',
        options: 'resizing_algorithm:nearest/resize:fit:400:400',
        description: 'Fast resize using nearest-neighbor interpolation'
    },
    {
        name: 'Resize with Linear Algorithm',
        options: 'resizing_algorithm:linear/resize:fit:500:500',
        description: 'Resize using bilinear interpolation'
    },
    {
        name: 'Resize with Cubic Algorithm',
        options: 'resizing_algorithm:cubic/resize:fit:450:450',
        description: 'Resize using bicubic interpolation'
    },
    {
        name: 'Resize with Lanczos2 Algorithm',
        options: 'resizing_algorithm:lanczos2/resize:fit:600:600',
        description: 'High quality resize with lanczos2'
    },
    {
        name: 'Resize with Lanczos3 Algorithm',
        options: 'resizing_algorithm:lanczos3/resize:fit:550:550',
        description: 'Highest quality resize with lanczos3 (default)'
    },
    {
        name: 'Algorithm with Complex Processing',
        options: 'resizing_algorithm:cubic/resize:fill:400:300/sharpen:1.2/quality:88',
        description: 'Cubic algorithm with additional processing'
    },
];

// Main test function
export default function () {
    // Randomly select a scenario
    const scenario = scenarios[Math.floor(Math.random() * scenarios.length)];

    group(scenario.name, function () {
        // Build the processing path with plain URL
        const processingPath = `/${scenario.options}/plain/${TEST_IMAGE_URL}`;

        // Generate signature
        const signature = generateSignature(processingPath);

        // Build the full URL
        const fullPath = `/${signature}${processingPath}`;
        const url = `${BASE_URL}${fullPath}`;

        // Make the request
        const startTime = Date.now();
        const response = http.get(url, {
            tags: {
                scenario: scenario.name,
                options: scenario.options
            },
        });
        const duration = Date.now() - startTime;

        // Track metrics
        processingDuration.add(duration);

        // Check response
        const success = check(response, {
            'status is 200': (r) => r.status === 200,
            'has content-type header': (r) => r.headers['Content-Type'] !== undefined,
            'response has body': (r) => r.body && r.body.length > 0,
        });

        if (!success) {
            errorRate.add(1);
            console.log(`Error for ${scenario.name}: Status ${response.status}, Body: ${response.body ? response.body.substring(0, 200) : 'empty'}`);
        } else {
            errorRate.add(0);

            // Track cache hits/misses if header is present
            const cacheStatus = response.headers['X-Cache-Status'];
            if (cacheStatus === 'HIT') {
                cacheHits.add(1);
            } else if (cacheStatus === 'MISS') {
                cacheMisses.add(1);
            }
        }
    });

    // Random sleep between 1-3 seconds to simulate real user behavior
    sleep(Math.random() * 2 + 1);
}

// Warmup function - runs once at the start
export function setup() {
    console.log('=== K6 Load Test Configuration ===');
    console.log(`Base URL: ${BASE_URL}`);
    console.log(`Using unsigned URLs: ${USE_UNSIGNED}`);
    console.log(`Test image URL: ${TEST_IMAGE_URL}`);
    console.log(`Total scenarios: ${scenarios.length}`);
    console.log('===================================');

    // Test connection to server
    const statusResponse = http.get(`${BASE_URL}/status`);
    const serverUp = statusResponse.status === 200;

    if (!serverUp) {
        console.error('ERROR: Server is not responding at /status endpoint');
        throw new Error('Server not available');
    }

    console.log('✓ Server is up and responding');
    return { serverUp };
}

// Teardown function - runs once at the end
export function teardown(data) {
    console.log('=== Load Test Complete ===');
    console.log('Check the summary metrics above for detailed results');
}
