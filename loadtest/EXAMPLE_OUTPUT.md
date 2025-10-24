# Example K6 Load Test Output

This document shows example outputs from the K6 load testing suite.

## Smoke Test Output

```
          /\      |‾‾| /‾‾/   /‾‾/   
     /\  /  \     |  |/  /   /  /    
    /  \/    \    |     (   /   ‾‾\  
   /          \   |  |\  \ |  (‾)  | 
  / __________ \  |__| \__\ \_____/ .io

=== Smoke Test Starting ===
Testing: http://localhost:3000
✓ Server is responding

  execution: local
     script: smoke-test.js
     output: -

  scenarios: (100.00%) 1 scenario, 3 max VUs, 1m0s max duration (incl. graceful stop):
           * default: 3 looping VUs for 30s (gracefulStop: 30s)


running (0m30.5s), 0/3 VUs, 45 complete and 0 interrupted iterations
default ✓ [======================================] 3 VUs  30s

     ✓ Status Check: status is 200
     ✓ Basic Resize: status is 200
     ✓ Basic Resize: has body
     ✓ Format Conversion: status is 200
     ✓ Format Conversion: has body
     ✓ With Quality: status is 200
     ✓ With Quality: has body
     ✓ With Effect: status is 200
     ✓ With Effect: has body

     checks.........................: 100.00% ✓ 405       ✗ 0   
     data_received..................: 23 MB   755 kB/s
     data_sent......................: 12 kB   389 B/s
     http_req_blocked...............: avg=1.23ms   min=0s       med=0s      max=98.45ms  p(90)=0s      p(95)=0s     
     http_req_connecting............: avg=912µs    min=0s       med=0s      max=72.34ms  p(90)=0s      p(95)=0s     
     http_req_duration..............: avg=1.24s    min=24.56ms  med=1.12s   max=2.67s    p(90)=2.01s   p(95)=2.23s  
       { expected_response:true }...: avg=1.24s    min=24.56ms  med=1.12s   max=2.67s    p(90)=2.01s   p(95)=2.23s  
     http_req_failed................: 0.00%   ✓ 0         ✗ 225
     http_req_receiving.............: avg=12.45ms  min=54.67µs  med=8.23ms  max=123.45ms p(90)=28.9ms  p(95)=45.67ms
     http_req_sending...............: avg=234.56µs min=23.45µs  med=178.9µs max=2.34ms   p(90)=456.78µs p(95)=678.9µs
     http_req_tls_handshaking.......: avg=0s       min=0s       med=0s      max=0s       p(90)=0s      p(95)=0s     
     http_req_waiting...............: avg=1.23s    min=23.45ms  med=1.1s    max=2.61s    p(90)=1.99s   p(95)=2.21s  
     http_reqs......................: 225     7.377049/s
     iteration_duration.............: avg=4.06s    min=2.12s    med=3.98s   max=7.23s    p(90)=5.45s   p(95)=6.12s  
     iterations.....................: 45      1.475409/s
     vus............................: 3       min=3       max=3
     vus_max........................: 3       min=3       max=3
```

## Load Test Output

```
          /\      |‾‾| /‾‾/   /‾‾/   
     /\  /  \     |  |/  /   /  /    
    /  \/    \    |     (   /   ‾‾\  
   /          \   |  |\  \ |  (‾)  | 
  / __________ \  |__| \__\ \_____/ .io

=== K6 Load Test Configuration ===
Base URL: http://localhost:3000
Using unsigned URLs: true
Test image URL: https://picsum.photos/800/600
Total scenarios: 24
===================================
✓ Server is up and responding

  execution: local
     script: processing-endpoint.js
     output: -

  scenarios: (100.00%) 1 scenario, 20 max VUs, 4m30s max duration (incl. graceful stop):
           * default: Up to 20 looping VUs for 4m0s over 4 stages (gracefulRampDown: 30s, gracefulStop: 30s)


running (4m00.5s), 00/20 VUs, 1842 complete and 0 interrupted iterations
default ✓ [======================================] 00/20 VUs  4m0s

     ✓ status is 200
     ✓ has content-type header
     ✓ response has body

     cache_hits.....................: 456    1.899167/s
     cache_misses...................: 5526   23.024167/s
     checks.........................: 100.00% ✓ 5526      ✗ 0    
     data_received..................: 418 MB  1.7 MB/s
     data_sent......................: 334 kB  1.4 kB/s
     errors.........................: 0.00%   ✓ 0         ✗ 1842
     http_req_blocked...............: avg=1.45ms   min=0s       med=0s      max=156.78ms p(90)=0s      p(95)=1.23ms 
     http_req_connecting............: avg=1.12ms   min=0s       med=0s      max=123.45ms p(90)=0s      p(95)=978µs  
     http_req_duration..............: avg=2.12s    min=123.45ms med=1.89s   max=4.56s    p(90)=3.23s   p(95)=3.78s  
       { expected_response:true }...: avg=2.12s    min=123.45ms med=1.89s   max=4.56s    p(90)=3.23s   p(95)=3.78s  
     http_req_failed................: 0.00%   ✓ 0         ✗ 1842
     http_req_receiving.............: avg=18.67ms  min=67.89µs  med=12.34ms max=234.56ms p(90)=45.67ms p(95)=78.9ms 
     http_req_sending...............: avg=345.67µs min=34.56µs  med=234.56µs max=3.45ms  p(90)=678.9µs p(95)=1.23ms
     http_req_tls_handshaking.......: avg=0s       min=0s       med=0s      max=0s       p(90)=0s      p(95)=0s     
     http_req_waiting...............: avg=2.1s     min=120.12ms med=1.87s   max=4.52s    p(90)=3.21s   p(95)=3.75s  
     http_reqs......................: 1842    7.675/s
     iteration_duration.............: avg=4.23s    min=1.45s    med=4.01s   max=7.89s    p(90)=5.67s   p(95)=6.23s  
     iterations.....................: 1842    7.675/s
     processing_duration............: avg=2123.45  min=123.45   med=1890    max=4560     p(90)=3230    p(95)=3780   
     vus............................: 1       min=1       max=20 
     vus_max........................: 20      min=20      max=20 


=== Load Test Complete ===
Check the summary metrics above for detailed results
```

## Stress Test Output (Showing Breaking Point)

```
          /\      |‾‾| /‾‾/   /‾‾/   
     /\  /  \     |  |/  /   /  /    
    /  \/    \    |     (   /   ‾‾\  
   /          \   |  |\  \ |  (‾)  | 
  / __________ \  |__| \__\ \_____/ .io

=== Stress Test Configuration ===
Target: http://localhost:3000
Load Pattern: 0 → 50 → 100 → 150 → 200 users over 24 minutes
WARNING: This test will generate significant load
=====================================
✓ Server is ready

  execution: local
     script: stress-test.js
     output: -

  scenarios: (100.00%) 1 scenario, 200 max VUs, 26m30s max duration (incl. graceful stop):
           * default: Up to 200 looping VUs for 24m0s over 7 stages (gracefulRampDown: 30s, gracefulStop: 2m0s)

Error [Complex Processing]: Status 503
Error [High DPR]: Status 503

running (24m00.8s), 000/200 VUs, 18456 complete and 0 interrupted iterations
default ✓ [======================================] 000/200 VUs  24m0s

     ✓ status is 200
     ✓ has body

     checks.........................: 97.23%  ✓ 35898     ✗ 1014
     data_received..................: 3.2 GB  2.2 MB/s
     data_sent......................: 2.8 MB  1.9 kB/s
     errors.........................: 2.77%   ✓ 507       ✗ 17949
     http_req_blocked...............: avg=2.34ms   min=0s       med=0s       max=456.78ms p(90)=0s      p(95)=4.56ms 
     http_req_connecting............: avg=1.89ms   min=0s       med=0s       max=389.12ms p(90)=0s      p(95)=3.45ms 
     http_req_duration..............: avg=3.67s    min=89.12ms  med=2.89s    max=15.23s   p(90)=7.89s   p(95)=10.23s 
       { expected_response:true }...: avg=3.45s    min=89.12ms  med=2.78s    max=12.34s   p(90)=7.12s   p(95)=9.23s  
     http_req_failed................: 2.77%   ✓ 507       ✗ 17949
     http_req_receiving.............: avg=28.9ms   min=78.9µs   med=18.9ms   max=567.89ms p(90)=78.9ms  p(95)=123.45ms
     http_req_sending...............: avg=567.89µs min=45.67µs  med=345.67µs max=12.34ms  p(90)=1.23ms  p(95)=2.34ms 
     http_req_tls_handshaking.......: avg=0s       min=0s       med=0s       max=0s       p(90)=0s      p(95)=0s     
     http_req_waiting...............: avg=3.64s    min=86.78ms  med=2.87s    max=15.12s   p(90)=7.85s   p(95)=10.18s 
     http_reqs......................: 18456   12.816667/s
     iteration_duration.............: avg=3.89s    min=234.56ms med=3.12s    max=16.78s   p(90)=8.23s   p(95)=10.67s 
     iterations.....................: 18456   12.816667/s
     processing_duration............: avg=3678.9   min=89.12    med=2890     max=15230    p(90)=7890    p(95)=10230  
     vus............................: 3       min=3       max=200
     vus_max........................: 200     min=200     max=200

=== Stress Test Complete ===
Review metrics above to identify breaking points and bottlenecks

WARN[0000] Thresholds on metrics 'http_req_failed, errors' have been breached
```

**Analysis:** The stress test shows the system starting to fail around 150-200 concurrent users, with a 2.77% error rate and 503 responses appearing. This indicates the server reached its capacity limit.

## Cache Performance Test Output

```
          /\      |‾‾| /‾‾/   /‾‾/   
     /\  /  \     |  |/  /   /  /    
    /  \/    \    |     (   /   ‾‾\  
   /          \   |  |\  \ |  (‾)  | 
  / __________ \  |__| \__\ \_____/ .io

=== Cache Performance Test ===
Server: http://localhost:3000
This test measures cache effectiveness by:
1. Warming up cache with common transformations
2. Repeatedly requesting the same transformations
3. Comparing first-request vs cached-request performance
=====================================
✓ Server is ready

NOTE: Ensure imgforge is configured with caching enabled
(IMGFORGE_CACHE_TYPE=memory or hybrid or disk)
=====================================

  execution: local
     script: cache-performance.js
     output: -

  scenarios: (100.00%) 1 scenario, 15 max VUs, 3m30s max duration (incl. graceful stop):
           * default: Up to 15 looping VUs for 3m0s over 3 stages (gracefulRampDown: 30s, gracefulStop: 30s)

[Iter 100] Cache status: HIT, Duration: 45ms
[Iter 200] Cache status: HIT, Duration: 38ms
[Iter 300] Cache status: HIT, Duration: 42ms

running (3m00.5s), 00/15 VUs, 1234 complete and 0 interrupted iterations
default ✓ [======================================] 00/15 VUs  3m0s

     ✓ warmup successful
     ✓ status is 200
     ✓ has body

     cached_request_duration........: avg=48.23    min=23.45    med=45.67    max=234.56   p(90)=67.89   p(95)=89.12  
     checks.........................: 100.00% ✓ 3702      ✗ 0    
     data_received..................: 312 MB  1.7 MB/s
     data_sent......................: 245 kB  1.4 kB/s
     first_request_duration.........: avg=2145.67  min=456.78   med=2012.34  max=4234.56  p(90)=3123.45 p(95)=3567.89
     http_req_blocked...............: avg=867µs    min=0s       med=0s       max=89.12ms  p(90)=0s      p(95)=0s     
     http_req_connecting............: avg=645µs    min=0s       med=0s       max=67.89ms  p(90)=0s      p(95)=0s     
     http_req_duration..............: avg=234.56ms min=23.45ms  med=48.9ms   max=4.23s    p(90)=567.89ms p(95)=1.23s  
       { expected_response:true }...: avg=234.56ms min=23.45ms  med=48.9ms   max=4.23s    p(90)=567.89ms p(95)=1.23s  
     http_req_failed................: 0.00%   ✓ 0         ✗ 1234
     http_req_receiving.............: avg=4.56ms   min=45.67µs  med=2.34ms   max=123.45ms p(90)=12.34ms p(95)=23.45ms
     http_req_sending...............: avg=123.45µs min=23.45µs  med=89.12µs  max=2.34ms   p(90)=234.56µs p(95)=456.78µs
     http_req_tls_handshaking.......: avg=0s       min=0s       med=0s       max=0s       p(90)=0s      p(95)=0s     
     http_req_waiting...............: avg=229.89ms min=21.23ms  med=46.78ms  max=4.21s    p(90)=563.45ms p(95)=1.21s  
     http_reqs......................: 1234    6.855556/s
     iteration_duration.............: avg=2.45s    min=123.45ms med=1.89s    max=6.78s    p(90)=4.56s   p(95)=5.23s  
     iterations.....................: 1234    6.855556/s
     vus............................: 1       min=1       max=15 
     vus_max........................: 15      min=15      max=15 

=== Cache Performance Results ===
Compare metrics:
- first_request_duration: Time for uncached requests
- cached_request_duration: Time for cached requests
- Large difference indicates good cache performance
===================================
```

**Analysis:** Cache is working excellently! Cached requests (48ms avg) are ~44x faster than first requests (2146ms avg), showing a cache hit improvement of 97.8%.

## Interpreting the Metrics

### Key Performance Indicators

| Metric | Good | Warning | Critical |
|--------|------|---------|----------|
| http_req_duration p(95) | < 3s | 3-5s | > 5s |
| http_req_failed | < 1% | 1-5% | > 5% |
| checks | 100% | > 95% | < 95% |
| errors | < 1% | 1-5% | > 5% |

### Common Patterns

**Healthy System:**
- All checks passing (100%)
- No failed requests (0%)
- Consistent response times
- p(95) and p(99) close together

**Cache Working:**
- High cache_hits counter
- Low cached_request_duration
- Significant difference between first and cached request times

**System Under Stress:**
- Increasing error rates
- Growing p(95) and p(99) times
- Failed requests appearing (500s, 503s)
- Threshold violations

**Overloaded System:**
- Error rate > 10%
- Many 503 Service Unavailable
- p(99) times 2-3x higher than p(95)
- Significant request failures
