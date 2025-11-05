# 12. Image Processing Pipeline

This document zooms in on the transformation phase that runs after imgforge validates a request. Use it alongside [Processing Options](5_processing_options.md) to design URLs and reason about how directives interact inside libvips.

## High-level flow

1. **Plan normalization** – Parsed directives are expanded into a structured plan with explicit defaults. Missing widths or heights default to `0`, which allows imgforge to preserve aspect ratio. Quality defaults to `85`, backgrounds default to transparent/black depending on the target format, and EXIF auto-rotation starts enabled.
2. **Device-pixel-ratio scaling** – When `dpr` is present, imgforge multiplies all linear dimensions (width, height, padding) before any transformations take place. This scaling happens before limit checks so a large `dpr` can trip resolution safeguards.
3. **Image loading** – libvips ingests the source buffer, performs color-profile conversion when required, and applies EXIF orientation unless `auto_rotate:false` was specified.
4. **Geometry transforms** – Crops execute first, followed by explicit resizing directives (`resize`, `size`, `width`, `height`) using the active `resizing_type`. Gravity influences how libvips positions the crop window and fill canvas. Upscaling is blocked unless `enlarge:true` was provided globally or through the specific directive.
5. **Canvas adjustments** – Padding, extend, and background directives run after resizing so they operate on the final viewport. Padding values inherit `dpr` scaling. When outputting formats without alpha channels (e.g., JPEG), backgrounds are flattened against the target color.
6. **Effects & safeguards** – Blur, sharpen, pixelate, and zoom run after geometry changes. Minimum dimension checks (`min_width`, `min_height`) can trigger an additional upscale when the image still falls short. Watermarks load at this stage, clamped by the canvas size, and will fail with a descriptive error if the watermark image cannot be fetched or decoded.
7. **Encoding** – The final libvips image is encoded into the desired format. Explicit `format` directives override the implicit format derived from `@extension`. Compression quality honours the `quality` directive, falling back to `85` for JPEG/WebP and libvips defaults for other codecs. Metadata stripping follows libvips defaults.

## Inter-option nuances

- **Resizing + padding** – Padding is additive. A `resize:fit:800:600` followed by `padding:20` yields an 840×640 canvas before background flattening. Remember to scale padding expectations when using `dpr`.
- **Crop vs. gravity** – An explicit `crop:x:y:width:height` ignores gravity because coordinates are absolute. Gravity only matters for implicit crops triggered by `fill` resizing or watermark positioning.
- **Zoom and minimums** – `zoom` multiplies the dimensions produced by resizing. Minimum width/height run afterward, so a zoom factor under 1.0 can still be clamped upward by `min_width` or `min_height`.
- **Watermark precedence** – If both `watermark_url` and `IMGFORGE_WATERMARK_PATH` are available, the URL-specific asset wins. Repeated watermark directives use the last occurrence.
- **Concurrency and `raw` mode** – Setting `raw` skips the global semaphore, but the transformation order remains identical. Use it sparingly for high-priority jobs.

## Failure modes

- Invalid numerical values (negative widths, NaNs, out-of-range blur sigma) surface as `400 Bad Request` before libvips runs.
- Oversized outputs—after applying `dpr`, padding, and minimum dimensions—trigger the megapixel guard or source file size checks if derivatives exceed the thresholds.
- Watermark downloads share the same timeout and size limits as primary sources; failures stop the request unless a fallback watermark path exists.

## Observability

- `image_processing_duration_seconds` measures time spent inside this pipeline.
- `processed_images_total{format="..."}` tracks throughput per output format.
- Logging includes a request ID. Combine it with the detailed stages in [Request Lifecycle](6_request_lifecycle.md) when debugging complex transformations.

Use this pipeline map when troubleshooting visual discrepancies, designing new directives, or reasoning about how multiple options compose.
