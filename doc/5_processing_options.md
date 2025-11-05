# 5. Processing Options

imgforge encodes image transformations directly in the URL path. Each directive uses the format `name:arg1:arg2`, with multiple directives chained via `/`. Unknown options are ignored—typos silently disable transformations—so validate URLs in automated tests or internal tooling. The tables and sections below describe every option, the defaults applied by imgforge, and how directives interact.

## Quick reference

| Option               | Aliases   | Arguments                              | Purpose & defaults                                                                    |
|----------------------|-----------|----------------------------------------|---------------------------------------------------------------------------------------|
| `preset`             | `pr`      | `name`                                 | References a named preset defined via `IMGFORGE_PRESETS`. See [3_configuration.md](3_configuration.md). |
| `resize`             | `rs`      | `type:width:height[:enlarge][:extend]` | Primary resize control. Defaults to no resize. `enlarge`/`extend` default to `false`. |
| `size`               | `sz`, `s` | `width:height[:enlarge][:extend]`      | Convenience wrapper for `resize` with implicit `fit`.                                 |
| `resizing_type`      | `rt`      | `type`                                 | Overrides the mode used by other resizing directives.                                 |
| `resizing_algorithm` | `ra`      | `algorithm`                            | Interpolation kernel for resize operations. Defaults to `lanczos3`.                   |
| `width`              | `w`       | `value`                                | Sets a target width (infers height). Implies `fit`.                                   |
| `height`             | `h`       | `value`                                | Sets a target height (infers width). Implies `fit`.                                   |
| `gravity`            | `g`       | `anchor`                               | Controls crop/fill anchoring (`center`, `north_east`, etc.). Defaults to `center`.    |
| `enlarge`            | `el`      | `bool`                                 | Allows upscaling globally. Defaults to `false`.                                       |
| `extend`             | `ex`      | `bool`                                 | Pads to target dimensions after resize. Defaults to `false`.                          |
| `padding`            | `pd`      | `top[:right][:bottom][:left]`          | Adds padding after resizing. Defaults to zero padding.                                |
| `min_width`          | `mw`      | `value`                                | Ensures result width meets minimum. Upscales if required.                             |
| `min_height`         | `mh`      | `value`                                | Ensures result height meets minimum. Upscales if required.                            |
| `zoom`               | `z`       | `factor`                               | Multiplies dimensions after resizing. Defaults to `1.0`.                              |
| `crop`               | —         | `x:y:width:height`                     | Crops before resizing. No crop by default.                                            |
| `rotate`             | `rot`     | `0                                     | 90                                                                                    |180|270`                             | Applies fixed rotation. Defaults to `0`.                                                                                           |
| `auto_rotate`        | `ar`      | `bool`                                 | Honours EXIF orientation (`true` by default).                                         |
| `blur`               | `bl`      | `sigma`                                | Gaussian blur (0 disables).                                                           |
| `sharpen`            | `sh`      | `sigma`                                | Sharpens edges.                                                                       |
| `pixelate`           | `px`      | `amount`                               | Pixelation strength.                                                                  |
| `background`         | `bg`      | `RRGGBB[AA]`                           | Canvas colour for extend/padding/flatten. Defaults to transparent unless JPEG output. |
| `quality`            | `q`       | `1-100`                                | Compression quality. Defaults to `85` for lossy formats.                              |
| `format`             | —         | `jpeg                                  | png                                                                                   |webp|avif|...`                   | Output format override. Defaults to `jpeg` when unspecified.                                                                       |
| `dpr`                | —         | `1.0-5.0`                              | Device pixel ratio multiplier. Defaults to `1.0`.                                     |
| `raw`                | —         | —                                      | Skips the concurrency semaphore. Defaults to disabled.                                |
| `cache_buster`       | —         | `token`                                | Alters the cache key.                                                                 |
| `max_src_resolution` | —         | `megapixels`                           | Request-level override. Requires server opt-in.                                       |
| `max_src_file_size`  | —         | `bytes`                                | Request-level override. Requires server opt-in.                                       |
| `watermark`          | `wm`      | `opacity:position`                     | Enables watermarking. Requires watermark asset.                                       |
| `watermark_url`      | `wmu`     | `base64url(url)`                       | Fetches watermark per request. Overrides server default path.                         |

## Presets

### `preset:name`

Presets provide a way to define reusable sets of processing options via the `IMGFORGE_PRESETS` environment variable. Instead of repeating complex transformation chains in every URL, reference a preset by name.

**Example configuration:**

```bash
export IMGFORGE_PRESETS="thumbnail=resize:fit:150:150/quality:80,banner=resize:fill:1200:300/quality:90"
```

**URL usage:**

```
/signature/preset:thumbnail/encoded_url
/signature/pr:banner/encoded_url
```

**Behavior:**
- The `preset:name` (or `pr:name`) directive expands to the preset's defined options at processing time.
- Multiple presets can be chained: `/preset:base/preset:quality_high/encoded_url`.
- URL-specific options override preset values when the same parameter appears in both.
- A preset named `default` automatically applies to every request before other options or presets.

**Presets-only mode:**

Set `IMGFORGE_ONLY_PRESETS=true` to restrict URLs to preset references only. Non-preset options will return `400 Bad Request`. This is useful for enforcing strict governance over allowed transformations.

See [3_configuration.md](3_configuration.md) for full configuration details.

## Geometry & resizing

### `resize:type:width:height[:enlarge][:extend]`

- **Types** – `fill`, `fit`, `force`, and `auto`. `auto` selects `fill` when orientations match and `fit` otherwise.
- **Defaults** – If width or height are omitted (or `0`), imgforge preserves aspect ratio using the provided dimension. `enlarge` and `extend` default to `false` unless explicitly set.
- **Enlarging** – Without `enlarge:true`, target dimensions that exceed the original image are clamped to avoid upscale work. Combine with `min_width`/`min_height` when you want conditional enlargement.
- **Extending** – `extend:true` pads the canvas to the requested size after resizing but before padding. The background colour determines the filled area.

### `size`

`size` and its aliases are shorthand for `resize:fit`. Width or height of `0` lets imgforge infer the missing dimension. Use the trailing arguments to flip `enlarge` or `extend` without switching to the long form.

### `width` / `height`

Setting a single dimension implicitly enables `fit` resizing. These options influence fallback behaviour when no explicit `resize` directive exists. `enlarge:false` still applies unless you opt in globally via the `enlarge` directive.

### `resizing_type`

`resizing_type:fill` (or similar) changes how implicit resizes behave. It affects `width`, `height`, `size`, and subsequent `resize` directives that omit the type. Place it before directives that rely on the mode to avoid surprises.

### `resizing_algorithm`

Controls the interpolation kernel used during resize operations. The algorithm affects image quality, sharpness, and processing speed:

- **`nearest`** – Nearest-neighbor interpolation. Fastest but produces blocky results. Suitable for pixel art or when speed is critical and quality is secondary.
- **`linear`** – Bilinear interpolation. Faster than cubic/lanczos with reasonable quality. Good for real-time applications.
- **`cubic`** – Bicubic interpolation. Balances quality and speed. Produces smoother results than linear.
- **`lanczos2`** – Lanczos with a=2. Good quality with less processing than lanczos3. Suitable for most use cases.
- **`lanczos3`** – **Default**. Lanczos with a=3. Highest quality interpolation with the sharpest results. Best for final output where quality matters.

The algorithm applies to all resize operations including `resize`, `size`, `width`, `height`, `min_width`, `min_height`, `zoom`, and `pixelate`. It also affects watermark scaling. More deep dive into the algorithms can be found [here](5.1_resizing_algorithms.md).

**Example:** `resizing_algorithm:cubic/resize:fit:800:600` uses bicubic interpolation for faster processing.

**Performance tip:** Use `nearest` or `linear` for thumbnails or temporary previews. Reserve `lanczos3` for production assets.

### `gravity`

Gravity defaults to `center`. It influences:

- Cropping windows when `fill` or `crop` is used.
- Canvas alignment for `extend`.
- Watermark positioning when combined with the `watermark` option (gravity only applies if you omit an explicit watermark position).

### Minimum dimensions & zoom

- `min_width` and `min_height` trigger an extra resize pass if the image is still smaller after primary resizing. This pass honours `enlarge`; if you want guaranteed minimums, set `enlarge:true`.
- `zoom` multiplies dimensions after resizing and minimum checks. Values < 1 shrink the image; values > 1 enlarge it even if `enlarge` is `false`.

### `padding`

- Accepts 1, 2, or 4 integers representing pixels.
- Padding runs after resizing/extend, so it doesn’t influence aspect ratio.
- `dpr` scaling multiplies all padding values before rendering.
- Transparent padding respects the output format: JPEG outputs are flattened against the background colour.

## Cropping & rotation

### `crop`

`crop:x:y:width:height` executes before any resizing. Coordinates are absolute, so gravity has no effect. Use it to isolate a region of interest that subsequent resizes should operate on.

### `auto_rotate` and `rotate`

- `auto_rotate` defaults to `true`, applying EXIF orientation automatically. Disable (`auto_rotate:false`) when you need the raw sensor orientation.
- `rotate` applies an explicit 90° multiple after auto-rotation and resizing. Non-right-angle values are ignored.

## Output control

### `format`

If omitted, imgforge encodes output as JPEG. Provide an explicit format (`webp`, `png`, `avif`, etc.) or use the `@extension` suffix following the source URL. Some formats may not be available if libvips lacks support.

### `quality`

Defaults to `85` for lossy codecs (JPEG, WebP, AVIF). `quality` is ignored for lossless formats such as PNG. Raising quality increases file size and processing time; lowering it can introduce artefacts.

### `background`

Accepts RGB or RGBA hex (`FFFFFF` or `FFFFFFFF`). The colour fills areas introduced by `extend` or `padding`. When outputting JPEG, imgforge automatically flattens transparency against the background colour. Without a background, JPEG outputs fall back to black.

### `dpr`

- Defaults to `1.0` and caps at `5.0`.
- Scales width, height, padding, and minimum dimensions before processing. This scaling happens before safeguards, so very high DPR values can trigger resolution limits.
- Combine with `quality` adjustments to tailor assets for HiDPI displays.

## Effects

### `blur`

Gaussian blur with sigma > 0 softens the image after resizing and padding. Values between 1 and 5 offer noticeable smoothing without obliterating detail.

### `sharpen`

Enhances edge contrast. Apply after resizing to counteract softness introduced by downscaling. Overly large values can create haloes.

### `pixelate`

Downsamples and rescales the image to create a mosaic effect. Use high values (40+) for anonymisation.

### `zoom`

Listed earlier under geometry, but keep in mind it also affects the intensity of subsequent effects—zooming in increases the apparent blur or pixelation radius.

## Watermarking

1. Add `watermark:<opacity>:<position>` to enable overlay. Opacity ranges from `0.0` (invisible) to `1.0` (solid). Position accepts the same anchors as gravity (e.g., `south_east`).
2. Supply the watermark image via `watermark_url:<base64url>` or configure `IMGFORGE_WATERMARK_PATH` on the server. When both are present, the URL value wins.
3. Watermarks render after resizing, padding, and effects. Oversized or missing watermark assets fail the request with `400 Bad Request`.

## Cache control & concurrency

- `cache_buster:<token>` appends arbitrary data to the cache key. Change the token when you want to force reprocessing without altering transformations.
- `raw` bypasses the concurrency semaphore that ordinarily limits the number of simultaneous libvips jobs. Reserve it for high-priority tasks; uncontrolled usage can starve other requests.

## Security overrides

`max_src_resolution` and `max_src_file_size` relax server-wide safeguards for a single request. They only take effect when `IMGFORGE_ALLOW_SECURITY_OPTIONS=true` is set. Use cautiously, preferably on trusted internal URLs.

## Validation tips

- Use the signing guidance in [4_url_structure.md](4_url_structure.md) to confirm the encoded path matches the intended options.
- Reference the lifecycle and processing order in [6_request_lifecycle.md](6_request_lifecycle.md) and [12_image_processing_pipeline.md](12_image_processing_pipeline.md) when debugging unexpected output.
- Automate regression tests that call the processing endpoint with representative options to catch typos or changed defaults.
