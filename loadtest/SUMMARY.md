# K6 Load Testing Suite for imgforge - Summary

This document provides an overview of the complete K6 load testing suite for imgforge.

## Files Overview

### Test Scripts

1. **smoke-test.js** - Quick validation test
   - Duration: ~30 seconds
   - Virtual Users: 3
   - Purpose: Verify basic functionality and server availability
   - Use when: Quick sanity check, CI/CD pre-deployment validation

2. **processing-endpoint.js** - Comprehensive load test
   - Duration: ~4 minutes  
   - Virtual Users: Ramps from 0 → 10 → 20 → 0
   - Purpose: Test all 24+ processing parameter combinations under realistic load
   - Use when: Performance benchmarking, regression testing, feature validation
   - **Features:**
     - 24+ different processing scenarios
     - Covers resize, crop, blur, sharpen, format conversion, etc.
     - Random scenario selection
     - Custom metrics for cache hits/misses
     - Detailed error reporting

3. **stress-test.js** - Stress and capacity testing
   - Duration: ~24 minutes
   - Virtual Users: Ramps 0 → 50 → 100 → 150 → 200 → 0
   - Purpose: Find breaking points and performance limits
   - Use when: Capacity planning, infrastructure sizing, performance tuning
   - **Features:**
     - Weighted scenario selection (realistic usage patterns)
     - Focuses on common transformations
     - Identifies performance degradation points

4. **cache-performance.js** - Cache effectiveness test
   - Duration: ~3 minutes
   - Virtual Users: Ramps 0 → 10 → 15 → 0
   - Purpose: Measure cache hit rates and performance impact
   - Use when: Validating cache configuration, comparing cache backends
   - **Features:**
     - Cache warmup phase
     - Separate metrics for cached vs uncached requests
     - Fixed scenario set to ensure cache hits

### Documentation

- **README.md** - Comprehensive guide covering:
  - Installation and prerequisites
  - Running tests
  - Configuration options
  - Interpreting results
  - Troubleshooting

- **QUICK_START.md** - Get started in 5 minutes:
  - Quick setup steps
  - Common use cases
  - Example commands
  - Troubleshooting basics

- **SUMMARY.md** (this file) - High-level overview

### Configuration & Utilities

- **.env.example** - Template for environment configuration
- **run-test.sh** - Helper script for running tests easily
- **.gitignore** - Excludes test results and secrets

## Test Coverage

The load testing suite covers all major imgforge processing options:

### Image Resizing
- ✅ resize (fill, fit, force, auto modes)
- ✅ size (shorthand)
- ✅ width/height (individual dimensions)
- ✅ enlarge/extend options
- ✅ min_width/min_height
- ✅ zoom

### Image Cropping & Positioning
- ✅ crop (x, y, width, height)
- ✅ gravity (center, north, south, east, west, etc.)

### Image Effects
- ✅ blur
- ✅ sharpen
- ✅ pixelate

### Format & Quality
- ✅ format conversion (JPEG, PNG, WebP)
- ✅ quality control (1-100)

### Layout & Styling
- ✅ padding
- ✅ background colors
- ✅ rotation (90°, 180°, 270°)
- ✅ auto_rotate (EXIF handling)

### Advanced Features
- ✅ DPR (Device Pixel Ratio) scaling
- ✅ Complex multi-option combinations

### Security & Performance
- ✅ HMAC-signed URLs
- ✅ Unsigned URLs (development mode)
- ✅ Cache effectiveness
- ✅ Rate limiting behavior

## Quick Reference

### Running Tests

```bash
# Smoke test - quick validation
./run-test.sh smoke

# Load test - standard performance
./run-test.sh load

# Stress test - find limits
./run-test.sh stress

# Cache test - measure cache effectiveness
./run-test.sh cache

# Custom configuration
./run-test.sh -u https://example.com -k mykey -s mysalt load

# With unsigned URLs (development)
./run-test.sh --unsigned smoke
```

### Environment Variables

```bash
IMGFORGE_URL              # Server URL (default: http://localhost:3000)
IMGFORGE_KEY              # HMAC key for URL signing
IMGFORGE_SALT             # HMAC salt for URL signing
IMGFORGE_ALLOW_UNSIGNED   # Use unsigned URLs (true/false)
TEST_IMAGE_URL            # Test image URL (default: https://picsum.photos/800/600)
```

### Key Metrics to Monitor

| Metric | Good Value | Warning Signs |
|--------|-----------|---------------|
| http_req_failed | < 1% | > 5% |
| p(95) response time | < 3s | > 5s |
| checks | 100% | < 95% |
| errors rate | < 1% | > 5% |

## Integration Examples

### Local Development

```bash
# Start imgforge
export IMGFORGE_ALLOW_UNSIGNED=true
cargo run

# In another terminal
cd loadtest
./run-test.sh --unsigned smoke
```

### CI/CD Pipeline

```yaml
# GitHub Actions example
- name: Run Load Tests
  run: |
    cd loadtest
    ./run-test.sh \
      -u http://localhost:3000 \
      -k ${{ secrets.IMGFORGE_KEY }} \
      -s ${{ secrets.IMGFORGE_SALT }} \
      --output results.json \
      load
```

### Docker Environment

```bash
# Run imgforge in Docker
docker-compose up -d

# Run K6 tests
docker run --rm \
  --network host \
  -v $(pwd)/loadtest:/scripts \
  -e IMGFORGE_URL=http://localhost:3000 \
  -e IMGFORGE_ALLOW_UNSIGNED=true \
  grafana/k6:latest run /scripts/smoke-test.js
```

### Production Testing

```bash
# Test against production (be careful!)
./run-test.sh \
  -u https://imgforge.prod.example.com \
  -k $(vault read secret/imgforge-key) \
  -s $(vault read secret/imgforge-salt) \
  --vus 5 \
  --duration 2m \
  load
```

## Interpreting Results

### Successful Test
```
✓ All checks passed
✓ http_req_failed: 0.00%
✓ p(95) < 3000ms
✓ No threshold violations
```
**Action:** Document baseline, use for future comparisons

### Performance Degradation
```
✓ checks: 100%
⚠ p(95): 4500ms (threshold: 3000ms)
⚠ p(99): 6200ms
```
**Action:** Investigate slow requests, check resource usage, consider optimization

### Errors Under Load
```
✗ http_req_failed: 8.5% (threshold: 5%)
✗ errors rate: 9.2%
Status 500: 45 occurrences
```
**Action:** Check server logs, reduce load, investigate error causes

### Cache Performance
```
first_request_duration: avg=2.5s
cached_request_duration: avg=120ms
Cache improvement: 95%
```
**Action:** Validate cache is working optimally

## Best Practices

1. **Start Small**: Begin with smoke tests before running stress tests
2. **Monitor Resources**: Watch CPU, memory, disk I/O during tests
3. **Baseline First**: Establish performance baseline before changes
4. **Test Incrementally**: Test after each significant change
5. **Use Realistic Images**: Test with representative image sizes and types
6. **Document Results**: Keep records of test results over time
7. **Test in Staging**: Run stress tests in non-production environments
8. **Review Logs**: Always check server logs after failed tests

## Troubleshooting

| Problem | Likely Cause | Solution |
|---------|--------------|----------|
| Connection refused | Server not running | Start imgforge server |
| Invalid signature | Key/salt mismatch | Verify credentials or use --unsigned |
| High error rate | Server overload | Reduce VUs or check resources |
| Slow responses | Insufficient resources | Scale up or optimize |
| Test won't start | K6 not installed | Install K6 |
| Cache not working | Cache disabled | Check IMGFORGE_CACHE_TYPE config |

## Performance Tuning Tips

1. **Enable Caching**: Configure memory or hybrid cache for best performance
2. **Adjust Workers**: Set `IMGFORGE_WORKERS` based on CPU cores
3. **Optimize Images**: Use CDN or fast storage for source images  
4. **Rate Limiting**: Configure appropriate rate limits
5. **Resource Limits**: Set `max_src_resolution` and `max_src_file_size`
6. **Monitor**: Use Prometheus metrics to identify bottlenecks

## Next Steps

1. **Customize Scenarios**: Add your specific use cases to test scripts
2. **Automate**: Integrate into CI/CD pipeline
3. **Benchmark**: Establish performance baselines
4. **Monitor**: Set up continuous performance monitoring
5. **Document**: Keep records of test results and configurations
6. **Iterate**: Regular testing as codebase evolves

## Resources

- [K6 Documentation](https://k6.io/docs/)
- [imgforge Documentation](../doc/)
- [Processing Options Reference](../doc/5_processing_options.md)
- [Prometheus Monitoring](../doc/11_prometheus_monitoring.md)

## Support

For issues or questions:
1. Check the [README.md](README.md) troubleshooting section
2. Review [QUICK_START.md](QUICK_START.md) for common scenarios
3. Check imgforge server logs
4. Review K6 documentation
5. Open an issue on the imgforge repository

---

**Version**: 1.0  
**Last Updated**: 2024
**Maintainer**: imgforge team
