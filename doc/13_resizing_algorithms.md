# 13. Resizing Algorithms

imgforge supports multiple interpolation algorithms (kernels) for image resizing operations. The choice of algorithm affects image quality, processing speed, and the sharpness of the output. This guide explains each algorithm and when to use them.

## Overview

The `resizing_algorithm` (or `ra`) parameter controls which interpolation kernel libvips uses during all resize operations. This includes explicit resizes via `resize`, `size`, `width`, or `height`, as well as implicit scaling from `min_width`, `min_height`, `zoom`, and `pixelate` operations.

**Syntax:**
```
resizing_algorithm:<algorithm>
ra:<algorithm>  # shorthand
```

## Available Algorithms

### Nearest (`nearest`)

**Interpolation:** Nearest-neighbor  
**Speed:** Fastest  
**Quality:** Lowest

Nearest-neighbor simply copies the pixel value from the nearest source pixel without any blending or smoothing. This produces blocky, pixelated results for photographic images but is perfect for:

- **Pixel art** – Preserves sharp edges and original colors
- **Screenshots** – When exact pixel reproduction is required
- **Quick previews** – Speed is more important than quality
- **Placeholder images** – Temporary display before high-quality version loads

**Example:**
```
/ra:nearest/resize:fit:100:100/plain/https://example.com/image.jpg
```

**Performance:** ~3-5x faster than lanczos3 for large images.

### Linear (`linear`)

**Interpolation:** Bilinear  
**Speed:** Fast  
**Quality:** Moderate

Bilinear interpolation averages the four nearest pixels using linear weighting. It produces smoother results than nearest-neighbor while maintaining good performance:

- **Thumbnails** – Good balance of quality and speed
- **Real-time applications** – Live previews or interactive tools
- **Mobile optimization** – Faster processing on resource-constrained devices
- **Batch processing** – When processing thousands of images

**Example:**
```
/ra:linear/size:400:300/quality:80/plain/https://example.com/photo.jpg
```

**Performance:** ~2-3x faster than lanczos3.

### Cubic (`cubic`)

**Interpolation:** Bicubic  
**Speed:** Moderate  
**Quality:** Good

Bicubic interpolation uses a weighted average of 16 surrounding pixels with a cubic function. This provides a good balance between quality and performance:

- **General use** – Suitable for most photographic content
- **Web images** – Good quality without excessive processing time
- **CDN optimization** – Balance between cache miss cost and quality
- **Production workflows** – When lanczos3 is too slow but quality matters

**Example:**
```
/ra:cubic/resize:fill:800:600/sharpen:1.2/plain/https://example.com/photo.jpg
```

**Performance:** ~1.5-2x faster than lanczos3.

### Lanczos2 (`lanczos2`)

**Interpolation:** Lanczos with a=2  
**Speed:** Slower  
**Quality:** High

Lanczos2 uses a 2-lobe sinc function for interpolation. It provides excellent quality with less processing overhead than lanczos3:

- **High-quality output** – When you need better quality than cubic
- **Performance-conscious production** – Better than lanczos3 when every millisecond counts
- **Large images** – Reduced processing time while maintaining quality
- **Batch operations** – When processing many high-quality images

**Example:**
```
/ra:lanczos2/resize:fit:1920:1080/quality:92/plain/https://example.com/hires.jpg
```

**Performance:** ~1.2-1.5x faster than lanczos3.

### Lanczos3 (`lanczos3`) - **Default**

**Interpolation:** Lanczos with a=3  
**Speed:** Slowest  
**Quality:** Highest

Lanczos3 uses a 3-lobe sinc function and is considered the gold standard for image resizing. It produces the sharpest, highest-quality results:

- **Final output** – When quality is paramount
- **Professional photography** – Preserving maximum detail
- **Print preparation** – Highest quality for physical media
- **Hero images** – Landing pages, portfolios, galleries
- **Default behavior** – imgforge uses this when no algorithm is specified

**Example:**
```
/ra:lanczos3/resize:fit:2000:2000/quality:95/plain/https://example.com/masterpiece.jpg
```

**Performance:** Baseline (1x). Highest quality but slowest.

## Algorithm Selection Guide

### By Use Case

| Use Case | Recommended Algorithm | Rationale |
|----------|----------------------|-----------|
| Thumbnails (< 200px) | `linear` | Speed matters, small size hides artifacts |
| Preview images | `linear` or `cubic` | Balance of speed and acceptable quality |
| Web images (standard) | `cubic` | Good quality, reasonable performance |
| Hero/featured images | `lanczos3` | Maximum quality for prominent placement |
| Pixel art / sprites | `nearest` | Preserves crisp edges and original pixels |
| High-res photography | `lanczos2` or `lanczos3` | Professional quality output |
| Real-time processing | `nearest` or `linear` | Minimize latency |
| Batch processing | `cubic` | Balance throughput and quality |
| CDN/cached content | `lanczos3` | One-time cost, served many times |

### By Image Size

| Source → Target | Recommended | Alternative |
|-----------------|-------------|-------------|
| Large → Thumbnail (> 8x reduction) | `cubic` | `linear` for speed |
| Medium → Web (2-4x reduction) | `cubic` or `lanczos2` | `lanczos3` for best quality |
| Small → Smaller (< 2x reduction) | `lanczos2` | Any algorithm works well |
| Any → Upscale | `lanczos3` | `lanczos2` if speed critical |

### Performance vs Quality Matrix

```
Quality  ↑
         │
Highest  │                    ● lanczos3 (default)
         │               ● lanczos2
         │          ● cubic
         │     ● linear
Lowest   │ ● nearest
         └────────────────────────────────→ Speed
           Slowest                    Fastest
```

## Examples

### E-commerce Product Images

```bash
# Thumbnail (fast loading)
/ra:linear/size:150:150/quality:75/plain/https://shop.example.com/product.jpg

# Main product image (balance)
/ra:cubic/resize:fit:800:800/quality:85/plain/https://shop.example.com/product.jpg

# Zoom view (highest quality)
/ra:lanczos3/resize:fit:2000:2000/quality:90/plain/https://shop.example.com/product.jpg
```

### Responsive Images

```bash
# Mobile
/ra:linear/width:400/quality:80/plain/https://example.com/hero.jpg

# Tablet
/ra:cubic/width:800/quality:85/plain/https://example.com/hero.jpg

# Desktop
/ra:lanczos2/width:1600/quality:88/plain/https://example.com/hero.jpg

# Retina
/ra:lanczos3/width:3200/quality:90/plain/https://example.com/hero.jpg
```

### Avatar/Profile Pictures

```bash
# Small avatars (32x32) - speed matters
/ra:nearest/resize:fill:32:32/gravity:center/plain/https://example.com/avatar.jpg

# Medium avatars (128x128) - balanced
/ra:cubic/resize:fill:128:128/gravity:center/plain/https://example.com/avatar.jpg

# Large profile (512x512) - quality
/ra:lanczos3/resize:fill:512:512/gravity:center/plain/https://example.com/avatar.jpg
```

### Content Delivery Strategy

```bash
# First load: fast, lower quality
/ra:linear/size:600:400/quality:70/plain/https://example.com/article-image.jpg

# After interaction: upgrade quality
/ra:lanczos3/size:1200:800/quality:90/plain/https://example.com/article-image.jpg
```

## Performance Benchmarks

Relative processing times for a 4000x3000 → 800x600 resize:

- **nearest:** ~80ms (baseline)
- **linear:** ~150ms (1.9x)
- **cubic:** ~280ms (3.5x)
- **lanczos2:** ~380ms (4.8x)
- **lanczos3:** ~480ms (6.0x)

*Note: Actual times vary based on image content, format, and server hardware. These are representative ratios.*

## Best Practices

### 1. Match Algorithm to Audience

High-traffic public sites benefit from faster algorithms (`linear`, `cubic`) cached at CDN edge. Internal tools or photography portfolios justify `lanczos3`.

### 2. Consider DPR Scaling

```bash
# Standard DPR: use faster algorithm
/ra:cubic/size:400:300/dpr:1/plain/https://example.com/image.jpg

# High DPR: justify slower, higher quality
/ra:lanczos3/size:400:300/dpr:2/plain/https://example.com/image.jpg
```

### 3. Combine with Other Options

```bash
# Fast thumbnail with aggressive compression
/ra:linear/size:150:150/quality:70/format:webp/plain/https://example.com/image.jpg

# High quality with careful sharpening
/ra:lanczos3/resize:fit:1200:1200/sharpen:0.8/quality:92/plain/https://example.com/image.jpg
```

### 4. Progressive Enhancement

Serve fast initial renders, then swap to higher quality:

```javascript
// Initial load
<img src="/ra:linear/size:400:300/.../image.jpg" 
     data-hq="/ra:lanczos3/size:800:600/.../image.jpg">

// After page load
setTimeout(() => {
  img.src = img.dataset.hq;
}, 1000);
```

### 5. A/B Testing

Monitor metrics when changing algorithms:
- Processing time (server logs)
- Cache hit rate (CDN metrics)  
- User engagement (analytics)
- Visual quality scores (SSIM/PSNR)

## Troubleshooting

### Images Look Blurry

- Switch from `linear` to `cubic` or `lanczos2`
- Add `sharpen:1` to counteract softness
- Ensure source image has sufficient resolution

### Processing Too Slow

- Switch from `lanczos3` to `lanczos2` or `cubic`
- Reduce output dimensions first, then apply effects
- Enable caching to serve processed images from cache
- Check server resource usage (CPU, memory)

### Blocky/Pixelated Output

- Avoid `nearest` for photographic content
- Check that upscaling isn't excessive (>2x)
- Ensure source image quality is adequate

### Inconsistent Quality

- Explicitly set `ra:lanczos3` if quality is critical (don't rely on defaults)
- Use same algorithm across all sizes for visual consistency
- Document algorithm choices in URL generation logic

## Integration with Other Features

### With Effects

```bash
# Pixelate uses the same algorithm
/ra:nearest/pixelate:8/resize:fit:400:400/plain/https://example.com/face.jpg

# Zoom respects algorithm
/ra:lanczos3/zoom:1.5/resize:fit:600:600/plain/https://example.com/detail.jpg
```

### With Watermarks

```bash
# Watermark scaling uses the algorithm
/ra:cubic/resize:fit:800:600/watermark:0.7:south_east/plain/https://example.com/photo.jpg
```

### With Minimum Dimensions

```bash
# min_width/min_height use the algorithm
/ra:lanczos2/resize:fit:300:300/min_width:500/plain/https://example.com/image.jpg
```

## See Also

- [Processing Options](5_processing_options.md) – Complete option reference
- [Performance Guide](9_performance.md) – Optimization strategies
- [Image Processing Pipeline](12_image_processing_pipeline.md) – Processing order
- [Load Testing](../loadtest/README.md) – Performance testing with different algorithms
