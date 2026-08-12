# 12. Image Processing Pipeline

The transformation phase, which runs after imgforge has validated the request and fetched the source. Read it alongside [Processing Options](5_processing_options.md): the order below is what makes directives interact the way they do.

```
plan ─▶ dpr ─▶ load ─▶ geometry ─▶ canvas ─▶ effects ─▶ encode
                       crop        padding    blur       format
                       resize      extend     sharpen    quality
                                   background pixelate   metadata
                                              zoom
                                              min-width/height
                                              watermark
```

## The stages

1. **Plan normalization** – Parsed directives become a plan with explicit defaults. A missing width or height becomes `0`, which preserves aspect ratio. Quality defaults to `85`, EXIF auto-rotation starts enabled, and the background defaults to transparent or black depending on the output format.
2. **DPR scaling** – When `dpr` is present, every linear dimension (width, height, padding, minimums) is multiplied before anything else runs. This happens *before* the resolution safeguards, so a large `dpr` can trip them.
3. **Image loading** – libvips reads the source buffer, converts the colour profile when needed, and applies EXIF orientation unless `auto_rotate:false`.
4. **Geometry** – Crop runs first, then resizing (`resize`, `size`, `width`, `height`) using the active `resizing_type`. Gravity positions the crop window and the fill canvas. Upscaling is refused unless `enlarge:true`.
5. **Canvas** – Padding, `extend`, and `background` apply after resizing, so they operate on the final viewport. Output formats without an alpha channel are flattened against the background colour.
6. **Effects and safeguards** – Blur, sharpen, pixelate, and zoom run after geometry. `min-width` and `min-height` can trigger one more upscale if the image is still too small. Watermarks load here, clamped to the canvas; a watermark that cannot be fetched or decoded fails the request.
7. **Encoding** – The image is encoded to the requested format. An explicit `format` directive beats the format implied by `@extension`. Quality follows the `quality` directive, defaulting to `85` for lossy codecs.

## How options interact

- **Resizing and padding** – Padding is additive: `resize:fit:800:600` with `padding:20` yields an 840×640 canvas before flattening. Padding inherits `dpr` scaling, so check both together.
- **Crop and gravity** – `crop:x:y:width:height` uses absolute coordinates and ignores gravity. Gravity only matters for the implicit crop that `fill` performs, and for watermark placement.
- **Zoom and minimums** – `zoom` multiplies the dimensions produced by resizing, and the minimum checks run afterward. A `zoom` below 1.0 can still be pulled back up by `min-width` or `min-height`.
- **Watermark precedence** – A `watermark_url` in the request beats `IMGFORGE_WATERMARK_PATH`. If the directive repeats, the last one wins.
- **`raw` mode** – Skips the worker semaphore but changes nothing about the order above.

## Failure modes

- Invalid numbers — negative widths, NaN, out-of-range blur sigma — are rejected with `400 Bad Request` before libvips runs.
- The resolution and file-size guards see the dimensions *after* `dpr`, padding, and minimums are applied, so a request can trip them even when the source is well within limits.
- Watermark fetches share the timeout and size limits of the main source. A failure fails the request.

## Observability

| Metric                                            | Covers                                                          |
| ------------------------------------------------- | ---------------------------------------------------------------- |
| `image_processing_duration_seconds`               | This pipeline.                                                   |
| `image_operation_execution_duration_seconds`      | This pipeline plus source decoding and validation.               |
| `image_operation_semaphore_wait_duration_seconds` | Waiting for an imgforge worker permit.                           |
| `image_operation_blocking_queue_duration_seconds` | Waiting for a Tokio blocking thread.                             |
| `processed_images_total{format="..."}`            | Throughput per output format.                                    |

The two wait histograms separate imgforge worker saturation from blocking-pool saturation — see [Performance Tips](9_performance.md). Logs carry the request ID; pair them with [Request Lifecycle](6_request_lifecycle.md) when a transformation misbehaves.
