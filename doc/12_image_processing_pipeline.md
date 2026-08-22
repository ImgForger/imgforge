# 12. Image Processing Pipeline

The transformation phase, which runs after imgforge has validated the request and fetched the source. Read it alongside [Processing Options](5_processing_options.md): the order below is what makes directives interact the way they do.

```
plan ─▶ dpr ─▶ load ─▶ colour ─▶ frames ─┬▶ geometry ─▶ canvas ─▶ effects ─┬▶ colour ─▶ encode
                       to sRGB    split  │  trim        extend    adjust   │  to      format
                                         │  crop        extend_ar blur     │  result  quality
                                         │  resize      padding   sharpen  │          metadata
                                         │  min w/h     rotate    pixelate │          page height
                                         │  zoom        flip      watermark│
                                         └── once per frame ───────────────┘
                                                                     join ─┘
```

## The stages

1. **Plan normalization** – Parsed directives become a plan with explicit defaults. A missing width or height becomes `0`, which preserves aspect ratio. Quality defaults to `85`, EXIF auto-rotation starts enabled, and the background defaults to transparent or black depending on the output format.
2. **DPR scaling** – When `dpr` is above `1.0`, the resize width, height, and padding are multiplied by it. `min-width` and `min-height` are *not* scaled, so a minimum expressed in CSS pixels stays in CSS pixels while the resize target moves to device pixels.
3. **Image loading** – libvips reads the source buffer. `enforce_thumbnail` substitutes the source's embedded EXIF thumbnail here if there is one; `page` and `pages` decide how much of a multi-page or animated source is read.

   JPEG and WebP sources are decoded at a reduced scale when the plan allows it, so a large source is never unpacked at full resolution to produce a small result. JPEG decodes at 1/2, 1/4 or 1/8; WebP takes a continuous scale and can land much closer to what is needed. Both genuinely skip the work rather than doing it and discarding the result. The reduction is chosen so the decoded image is still at least as large as the target on both axes, and it is skipped entirely for `raw`, for any request with a `crop` (whose coordinates address source pixels), and when `dpr`, `zoom`, or the minimum dimensions mean more pixels are needed later. Source limits are enforced against the original dimensions, before any of this.
4. **Colour to processing** – The image is converted into a colourspace the pipeline's operations are written for. A source in CMYK, Lab, or anything else has no meaningful red, green, and blue channels, so every operation that weights them — saturation, the background flatten, trim's luminance fallback — produced nonsense without this. When the source carries an embedded ICC profile and is not already in a standard space, the conversion goes through that profile, so a wide-gamut image is interpreted the way its profile says rather than as if it were sRGB. A 16-bit source keeps its depth when `preserve_hdr` is set and the output format can carry it.
5. **Frame split** – An animated or multi-page image arrives from libvips as one tall stack of frames. It is taken apart here, and everything from step 6 to step 8 runs on each frame separately. This is what lets a rotation or a pad work on an animation at all: libvips does not update the frame height when the geometry changes, so transforming the stack as one image silently reinterprets four 80px frames as two 160px ones.
6. **Geometry** – `trim` runs first when present, removing a uniform border so everything after it works on the trimmed extent; it also disables the reduced-scale decode, since the trimmed size cannot be known in advance. Then crop, then resizing (`resize`, `size`, `width`, `height`) using the active `resizing_type`, then the minimum dimensions and `zoom`. Gravity positions the crop window and the fill canvas. Upscaling is refused unless `enlarge:true`.
7. **Canvas** – `extend`, `extend_aspect_ratio`, and `padding` apply after resizing, so they operate on the final viewport, followed by `rotate` and `flip`.
8. **Effects** – `adjust` (brightness, contrast, saturation), blur, sharpen, and pixelate, then the watermark. A watermark that cannot be fetched or decoded fails the request.
9. **Frame join and colour to result** – The frames are stacked back together and the encoder is told the new frame height. Output formats without an alpha channel are flattened against the background colour, and anything not already in a standard colourspace is converted for maximum compatibility.
10. **Encoding** – The image is encoded to the requested format. An explicit `format` directive beats the format implied by `@extension`, and content negotiation can beat both — see [Configuration](3_configuration.md). Quality follows the `quality` directive, defaulting to `85` for lossy codecs. A result too large for the output container is scaled down to fit first.

## How options interact

- **Resizing and padding** – Padding is additive: `resize:fit:800:600` with `padding:20` yields an 840×640 canvas before flattening. Padding inherits `dpr` scaling, so check both together.
- **Crop and gravity** – `crop:width:height[:gravity]` has no coordinates of its own: gravity positions the crop window, and without one of its own it falls back to the request's `gravity`, which defaults to centre. Gravity also drives the implicit crop that `fill` performs; `extend` and `extend_aspect_ratio` each take their own.
- **Zoom and minimums** – `zoom` multiplies the dimensions produced by resizing, and the minimum checks run afterward. A `zoom` below 1.0 can still be pulled back up by `min-width` or `min-height`.
- **Watermark precedence** – A `watermark_url` in the request beats `IMGFORGE_WATERMARK_PATH`. If the directive repeats, the last one wins.
- **Extend and extend_aspect_ratio** – Both can be set. `extend` runs first and fills the canvas to the requested pixel box; `extend_aspect_ratio` then grows whichever axis is still short of the requested *shape*. With both set the second has nothing left to do.
- **`raw` and `skip_processing`** – Both return the source bytes untouched, skipping every stage above. `skip_processing` applies only when the output format matches the source, because a conversion is processing.

## Failure modes

- Invalid numbers — negative widths, NaN, out-of-range blur sigma — are rejected with `400 Bad Request` before libvips runs.
- The resolution and file-size guards apply to the *source* image and run before this pipeline starts. Nothing here — `dpr`, padding, minimums — can trip them.
- The output has its own ceiling, `max_result_dimension`, and it is the only thing bounding how large a result you may ask for. It is checked at the end of this pipeline, once every stage above has settled the dimensions, but before encoding — so an over-limit request is rejected without the pixels ever being materialised. Unset, there is no ceiling at all.
- An animation has its own ceilings: `max_animation_frames` bounds how many frames are decoded and `max_animation_frame_resolution` bounds the size of one. The source-resolution limit measures the whole stack, which for an animation is the frame size multiplied by the frame count, so neither is implied by the other.
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
