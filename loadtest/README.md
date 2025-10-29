# imgforge Load Testing with K6

This directory contains K6 load testing scripts for the imgforge image processing service.

## Prerequisites

**Install K6**: Follow the installation guide at [k6.io](https://k6.io/docs/getting-started/installation/)
   
   Quick install options:
   ```bash
   # macOS
   brew install k6
   
   # Linux (Debian/Ubuntu)
   sudo gpg -k
   sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
   echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
   sudo apt-get update
   sudo apt-get install k6
   
   # Docker
   docker pull grafana/k6:latest
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

## Understanding the Results

### Key Metrics

- **http_req_duration**: Time taken for HTTP requests
  - p(95): 95% of requests completed within this time
  - p(99): 99% of requests completed within this time
  
- **http_req_failed**: Rate of failed HTTP requests

- **errors**: Custom error rate metric

- **processing_duration**: Time taken for image processing

- **cache_hits/cache_misses**: Cache effectiveness (if cache headers are present)

### Thresholds

The test defines performance thresholds:
- 95th percentile response time < 3000ms
- 99th percentile response time < 5000ms
- Error rate < 5%

If any threshold is breached, the test will fail. Monitor the results to identify bottlenecks.

| Metric              | Good Value | Warning Signs |
|---------------------|------------|---------------|
| http_req_failed     | < 1%       | > 5%          |
| p(95) response time | < 3s       | > 5s          |
| checks              | 100%       | < 95%         |
| errors rate         | < 1%       | > 5%          |

### Sample Output

```
     ✓ status is 200
     ✓ has content-type header
     ✓ response has body

     checks.........................: 100.00% ✓ 2400      ✗ 0
     data_received..................: 245 MB  68 kB/s
     data_sent......................: 186 kB  52 B/s
     errors.........................: 0.00%   ✓ 0        ✗ 800
     http_req_blocked...............: avg=1.2ms    min=0s       med=0s      max=123ms   p(95)=0s      p(99)=12ms
     http_req_duration..............: avg=1.89s    min=234ms    med=1.65s   max=4.23s   p(95)=2.89s   p(99)=3.45s
     http_req_failed................: 0.00%   ✓ 0        ✗ 800
     http_reqs......................: 800     2.222222/s
     processing_duration............: avg=1890.23  min=234      med=1650    max=4230
```

## Test Image Sources

By default, the test uses `https://picsum.photos/800/600` which provides random test images. You can:

1. **Use a different public image:**
   ```bash
   export TEST_IMAGE_URL="https://example.com/your-image.jpg"
   ```

2. **Use a local test server:**
   Set up a simple HTTP server with test images:
   ```bash
   # In a directory with test images
   python3 -m http.server 8080
   
   # Then use
   export TEST_IMAGE_URL="http://localhost:8080/test-image.jpg"
   ```

3. **Use wiremock or similar:**
   For deterministic testing, use a mock server that returns consistent images.

## Troubleshooting

### HMAC Signature Errors

If you see "Invalid signature" errors:
1. Verify `IMGFORGE_KEY` and `IMGFORGE_SALT` match your server configuration
2. Ensure the values are in the correct format (typically hex strings)
3. Try enabling unsigned URLs for testing: `export IMGFORGE_ALLOW_UNSIGNED=true`

### Connection Refused

If K6 can't connect to the server:
1. Verify the server is running: `curl http://localhost:3000/status`
2. Check the `IMGFORGE_URL` environment variable
3. If using Docker, use `host.docker.internal` instead of `localhost`

### High Error Rates

If you see high error rates:
1. Check server logs for specific errors
2. Reduce the load (lower VU count or duration)
3. Verify the test image URL is accessible
4. Check server resource limits (CPU, memory, file descriptors)

### Timeout Errors

If requests are timing out:
1. Increase the timeout in K6: `k6 run --http-debug --timeout 60s processing-endpoint.js`
2. Reduce the complexity of processing operations
3. Check if the server is overloaded

## Advanced Configuration

### Running Specific Scenarios

To test specific scenarios, modify the script to filter scenarios:

```javascript
const scenarios = [
    scenarios.filter(s => s.name.includes('Resize')),
    // Or test just one
    [scenarios[0]]
][0];
```

### Adding Custom Scenarios

Add new test scenarios to the `scenarios` array:

```javascript
{
    name: 'My Custom Test',
    options: 'resize:fit:500:500/quality:80/sharpen:2',
    description: 'Custom processing pipeline'
}
```

### Integration with CI/CD

Example GitHub Actions workflow:

```yaml
- name: Run K6 Load Tests
  run: |
    k6 run \
      -e IMGFORGE_URL="http://localhost:3000" \
      -e IMGFORGE_KEY="${{ secrets.IMGFORGE_KEY }}" \
      -e IMGFORGE_SALT="${{ secrets.IMGFORGE_SALT }}" \
      --out json=loadtest-results.json \
      loadtest/processing-endpoint.js
```

## Performance Tuning Tips

1. **Monitor server resources** during tests using tools like `htop`, `prometheus`, or your cloud provider's monitoring
2. **Start small** and gradually increase load to find breaking points
3. **Use caching** to improve performance for repeated requests
4. **Adjust concurrency limits** in imgforge configuration (`IMGFORGE_WORKERS`)
5. **Profile the application** to identify bottlenecks

## Further Reading

- [K6 Documentation](https://k6.io/docs/)
- [imgforge Documentation](../doc/)
- [Processing Options Reference](../doc/5_processing_options.md)
- [Prometheus Monitoring](../doc/11_prometheus_monitoring.md)
