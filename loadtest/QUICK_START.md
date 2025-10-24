# Quick Start Guide

Get started with K6 load testing for imgforge in under 5 minutes.

## Prerequisites

```bash
# Install K6 (choose your platform)
brew install k6                    # macOS
sudo apt-get install k6            # Debian/Ubuntu
# or see: https://k6.io/docs/getting-started/installation/
```

## 1. Configure Your Environment

```bash
cd loadtest
cp .env.example .env

# Edit .env with your settings
nano .env  # or vim, code, etc.
```

Minimum required configuration:
```bash
IMGFORGE_URL=http://localhost:3000
IMGFORGE_KEY=your-key-here
IMGFORGE_SALT=your-salt-here
```

For development/testing with unsigned URLs:
```bash
IMGFORGE_URL=http://localhost:3000
IMGFORGE_ALLOW_UNSIGNED=true
```

## 2. Start imgforge Server

```bash
# In another terminal, from project root
export IMGFORGE_KEY=your-key-here
export IMGFORGE_SALT=your-salt-here
export IMGFORGE_ALLOW_UNSIGNED=true  # Optional for testing
cargo run
```

Or with Docker:
```bash
docker run --rm -p 3000:3000 \
  -e IMGFORGE_KEY=your-key \
  -e IMGFORGE_SALT=your-salt \
  -e IMGFORGE_ALLOW_UNSIGNED=true \
  ghcr.io/imgforger/imgforge:latest
```

## 3. Run Your First Test

### Option A: Using the Helper Script (Recommended)

```bash
# Quick smoke test (30 seconds, 3 users)
./run-test.sh smoke

# Standard load test (~4 minutes, up to 20 users)
./run-test.sh load

# Stress test (~24 minutes, up to 200 users)
./run-test.sh stress
```

### Option B: Direct K6 Command

```bash
# Smoke test
k6 run smoke-test.js

# Load test
k6 run processing-endpoint.js

# Custom duration/VUs
k6 run --vus 10 --duration 1m processing-endpoint.js
```

## 4. Understanding Results

After the test completes, look for:

✅ **Good signs:**
- `checks: 100%` - All checks passed
- `http_req_failed: 0.00%` - No failed requests
- `p(95) < 3000ms` - 95% of requests under 3 seconds

⚠️ **Warning signs:**
- `http_req_failed > 5%` - High error rate
- `p(95) > 5000ms` - Slow response times
- Failed thresholds shown in red

## Common Use Cases

### Development - Quick Validation
```bash
# Fast smoke test with unsigned URLs
./run-test.sh --unsigned --duration 30s smoke
```

### Pre-deployment - Load Testing
```bash
# Standard load test, save results
./run-test.sh --output results.json load
```

### Performance Tuning - Finding Limits
```bash
# Stress test to find breaking point
./run-test.sh stress
```

### Testing Specific Image
```bash
# Test with your own image
./run-test.sh -i https://example.com/large-photo.jpg load
```

### Remote Server Testing
```bash
# Test production or staging environment
./run-test.sh -u https://imgforge.example.com \
  -k prod-key \
  -s prod-salt \
  load
```

## Troubleshooting

### "Server not available"
1. Check if server is running: `curl http://localhost:3000/status`
2. Verify the URL in your config

### "Invalid signature"
1. Check HMAC key and salt match server
2. Or use `--unsigned` for development

### High error rates
1. Reduce load: `./run-test.sh --vus 5 --duration 30s`
2. Check server logs for errors
3. Verify test image is accessible

### Slow performance
1. Check server resources (CPU, memory)
2. Enable caching in imgforge
3. Reduce concurrent workers if needed

## Next Steps

- Read [README.md](README.md) for detailed documentation
- Customize test scenarios in `processing-endpoint.js`
- Add your own test scripts
- Integrate with CI/CD pipeline

## Example Output

```
running (3m24.2s), 00/20 VUs, 1456 complete and 0 interrupted iterations
default ✓ [======================================] 00/20 VUs  3m24s

     ✓ status is 200
     ✓ has content-type header
     ✓ response has body

     checks.........................: 100.00% ✓ 4368     ✗ 0
     data_received..................: 312 MB  1.5 MB/s
     data_sent......................: 255 kB  1.2 kB/s
     http_req_blocked...............: avg=892µs  min=0s   med=0s    max=89ms   p(95)=0s   p(99)=5ms
     http_req_duration..............: avg=2.1s   min=145ms med=1.8s  max=4.5s   p(95)=3.2s p(99)=4.1s
     http_req_failed................: 0.00%   ✓ 0        ✗ 1456
     http_reqs......................: 1456    7.13/s

✓ Test completed successfully
```

Happy load testing! 🚀
