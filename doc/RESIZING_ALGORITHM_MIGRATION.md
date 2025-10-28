# Resizing Algorithm Migration Guide

This guide helps you adopt the resizing algorithm feature in your imgforge deployment.

## Overview

The resizing algorithm feature allows you to control the interpolation kernel used during image resize operations. This enables you to optimize for either quality or performance based on your specific use case.

## No Action Required

**This feature is 100% backward compatible.** Existing URLs will continue to work exactly as before, using the default `lanczos3` algorithm (the same high-quality interpolation imgforge has always used).

## When to Consider This Feature

Consider using explicit resizing algorithms when:

1. **Performance matters** - Thumbnails, previews, or high-volume transformations
2. **Quality varies by use case** - Different algorithms for different image sizes
3. **Cost optimization** - Faster algorithms reduce CPU costs
4. **Specific requirements** - Pixel art needs `nearest`, photography needs `lanczos3`

## Adoption Strategies

### Strategy 1: Start with Thumbnails (Low Risk)

Begin by optimizing small thumbnails where speed matters more than quality:

**Before:**
```bash
/resize:fit:150:150/plain/https://example.com/image.jpg
```

**After (2-3x faster):**
```bash
/ra:linear/resize:fit:150:150/plain/https://example.com/image.jpg
```

**Expected Impact:**
- Faster processing: ~2-3x speed improvement
- Slightly softer edges: minimal visual impact at small sizes
- Reduced CPU usage: noticeable in high-volume scenarios

### Strategy 2: Segment by Size (Balanced)

Use different algorithms based on output size:

```javascript
function getResizeUrl(imageUrl, width, height) {
  let algorithm = 'lanczos3'; // default
  
  // Optimize small images for speed
  if (width <= 200 || height <= 200) {
    algorithm = 'linear';
  }
  // Balance quality and speed for medium sizes
  else if (width <= 800 || height <= 800) {
    algorithm = 'cubic';
  }
  // Use highest quality for large/hero images
  else {
    algorithm = 'lanczos3';
  }
  
  return `/ra:${algorithm}/resize:fit:${width}:${height}/plain/${imageUrl}`;
}
```

### Strategy 3: Progressive Enhancement (User-Centric)

Serve fast previews, then upgrade quality:

```html
<!-- Initial load: fast -->
<img src="/ra:linear/resize:fit:400:300/.../image.jpg" 
     id="hero"
     data-hq="/ra:lanczos3/resize:fit:800:600/.../image.jpg">

<script>
  // After page load, swap to high quality
  window.addEventListener('load', () => {
    setTimeout(() => {
      document.getElementById('hero').src = 
        document.getElementById('hero').dataset.hq;
    }, 500);
  });
</script>
```

### Strategy 4: Content-Type Based (Automatic)

Apply algorithms based on content type:

```javascript
function getAlgorithmForContent(contentType) {
  const rules = {
    'pixel-art': 'nearest',    // Preserve sharp edges
    'photo': 'lanczos3',       // Highest quality for photos
    'illustration': 'cubic',   // Balanced for graphics
    'thumbnail': 'linear',     // Speed for small images
    'screenshot': 'nearest',   // Exact pixel reproduction
  };
  return rules[contentType] || 'lanczos3';
}
```

## Testing Your Migration

### 1. Visual Comparison

Generate the same image with different algorithms and compare:

```bash
# Generate test images
for algo in nearest linear cubic lanczos2 lanczos3; do
  curl "http://localhost:3000/unsafe/ra:${algo}/resize:fit:400:300/plain/https://example.com/test.jpg" \
    -o "test-${algo}.jpg"
done

# Open all images side-by-side for visual inspection
```

### 2. Performance Testing

Measure processing time for different algorithms:

```bash
# Test with timing
for algo in nearest linear cubic lanczos2 lanczos3; do
  echo "Testing $algo..."
  time curl -s "http://localhost:3000/unsafe/ra:${algo}/resize:fit:800:600/plain/https://example.com/large.jpg" \
    -o "/dev/null"
done
```

### 3. Load Testing

Use the provided K6 scenarios:

```bash
cd loadtest
k6 run processing-endpoint.js
```

The test suite includes 6 scenarios specifically for resizing algorithms.

## Migration Examples

### E-commerce Site

**Before:**
```javascript
const productUrls = {
  thumbnail: `/resize:fit:100:100/plain/${productImage}`,
  card: `/resize:fit:400:400/plain/${productImage}`,
  detail: `/resize:fit:1000:1000/plain/${productImage}`,
  zoom: `/resize:fit:2000:2000/plain/${productImage}`,
};
```

**After (optimized):**
```javascript
const productUrls = {
  thumbnail: `/ra:linear/resize:fit:100:100/plain/${productImage}`,     // 3x faster
  card: `/ra:cubic/resize:fit:400:400/plain/${productImage}`,          // 2x faster
  detail: `/ra:lanczos2/resize:fit:1000:1000/plain/${productImage}`,   // 1.5x faster
  zoom: `/ra:lanczos3/resize:fit:2000:2000/plain/${productImage}`,    // max quality
};
```

### News/Blog Site

**Before:**
```javascript
function getArticleImageUrl(url, size) {
  return `/resize:fit:${size}:${size}/plain/${url}`;
}
```

**After (progressive):**
```javascript
function getArticleImageUrl(url, size, priority = 'normal') {
  // Fast loading for below-fold images
  const algorithm = priority === 'hero' ? 'lanczos3' : 
                    size > 600 ? 'cubic' : 'linear';
  return `/ra:${algorithm}/resize:fit:${size}:${size}/plain/${url}`;
}
```

### User Avatar Service

**Before:**
```javascript
const avatarUrl = `/resize:fill:${size}:${size}/gravity:center/plain/${userImage}`;
```

**After (optimized by size):**
```javascript
const algorithm = size <= 64 ? 'nearest' : 
                  size <= 128 ? 'linear' : 
                  'cubic';
const avatarUrl = `/ra:${algorithm}/resize:fill:${size}:${size}/gravity:center/plain/${userImage}`;
```

### Responsive Images with srcset

**Before:**
```html
<img srcset="/resize:fit:400:300/plain/.../image.jpg 400w,
             /resize:fit:800:600/plain/.../image.jpg 800w,
             /resize:fit:1200:900/plain/.../image.jpg 1200w">
```

**After (optimized):**
```html
<img srcset="/ra:linear/resize:fit:400:300/plain/.../image.jpg 400w,
             /ra:cubic/resize:fit:800:600/plain/.../image.jpg 800w,
             /ra:lanczos3/resize:fit:1200:900/plain/.../image.jpg 1200w">
```

## Monitoring Your Migration

### Metrics to Watch

1. **Processing Time** - Check Prometheus metrics:
   ```promql
   histogram_quantile(0.95, image_processing_duration_seconds)
   ```

2. **Cache Hit Rate** - New URLs create new cache entries:
   ```promql
   rate(cache_hits_total[5m]) / rate(cache_requests_total[5m])
   ```

3. **CPU Usage** - Faster algorithms reduce CPU load:
   ```promql
   rate(process_cpu_seconds_total[5m])
   ```

4. **Error Rate** - Ensure no regressions:
   ```promql
   rate(http_requests_failed_total[5m])
   ```

### Gradual Rollout Plan

**Week 1: Test**
- Deploy to staging with new algorithm URLs
- Run load tests with K6 scenarios
- Visually inspect output across device types
- Measure performance improvements

**Week 2: Pilot**
- Roll out to 5% of traffic (thumbnails only)
- Monitor metrics and visual quality
- Gather user feedback if possible
- Verify cache behavior

**Week 3: Expand**
- Increase to 25% of traffic
- Add medium-sized images (cubic algorithm)
- Continue monitoring
- Adjust algorithms based on data

**Week 4: Full Rollout**
- Deploy to 100% of traffic
- Update documentation for team
- Share results and best practices

## Rollback Plan

If issues arise, simply remove the `ra:` parameter from URLs:

**Current (with algorithm):**
```
/ra:cubic/resize:fit:400:300/plain/https://example.com/image.jpg
```

**Rollback (default):**
```
/resize:fit:400:300/plain/https://example.com/image.jpg
```

The signature will be different, so you'll need to regenerate URLs, but functionality remains identical to pre-migration behavior.

## Common Questions

### Will my cache be invalidated?

Yes, URLs with explicit `ra:` parameters are different from URLs without them, so they create new cache entries. Plan for:
- Increased storage (temporary during transition)
- Cache warm-up period (first requests will be slower)
- Potential cost increase (if using CDN bandwidth)

**Mitigation:** Roll out gradually, segment by image size/type.

### Do I need to change all URLs at once?

No! The feature is additive. You can:
- Keep existing URLs unchanged (they use `lanczos3` by default)
- Add `ra:` only to new URLs or specific use cases
- Mix algorithms across different pages/components

### What if I'm not sure which algorithm to use?

Start with these defaults:
- **Don't know?** Keep default (no `ra:` parameter)
- **Small (<200px)?** Use `ra:linear`
- **Medium (200-800px)?** Use `ra:cubic`
- **Large (>800px)?** Use `ra:lanczos3` (or keep default)
- **Pixel art?** Use `ra:nearest`

### How do I know if it's working?

Check the server logs for your request. With `IMGFORGE_LOG_LEVEL=debug`, you'll see the algorithm being applied. You can also compare processing times in the logs or Prometheus metrics.

## Resources

- [Resizing Algorithms Guide](13_resizing_algorithms.md) - Comprehensive reference
- [Processing Options](5_processing_options.md) - All available options
- [Performance Guide](9_performance.md) - Optimization strategies
- [Load Testing](../loadtest/README.md) - K6 test scenarios

## Support

If you encounter issues during migration:
1. Check the [troubleshooting section](13_resizing_algorithms.md#troubleshooting) in the algorithms guide
2. Review server logs with debug logging enabled
3. Compare URLs with and without algorithm parameter
4. Test with the K6 load testing suite
5. Open an issue with example URLs and expected vs actual behavior
