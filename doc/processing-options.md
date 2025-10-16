# Processing Options

imgforge follows the imgproxy convention of encoding transformation directives inside the request path. Each option appears as `name:arg1:arg2` and options are chained with `/` separators. Unknown options are ignored, so typographical errors may silently disable a transformation—take care to validate your URLs.

## Quick reference

| Option | Aliases | Arguments | Purpose |
| --- | --- | --- | --- |
| `resize` | `rs` | `type:width:height[:enlarge][:extend]` | Resize using the given strategy (`fill`, `fit`, `force`, `auto`). |
| `size` | `sz`, `s` | `width:height[:enlarge][:extend]` | Convenience wrapper for `resize` with implicit `fit`.
| `resizing_type` | `rt` | `type` | Override the resize mode when combined with `resize`/`size`.
| `width` | `w` | `value` | Target width used when implicit resizing occurs.
| `height` | `h` | `value` | Target height used when implicit resizing occurs.
| `gravity` | `g` | `anchor` | Anchor point for crops, fill, and extend (`center`, `north`, `south`, `east`, `west`, etc.). |
| `enlarge` | `el` | `0|1|true|false` | Allow images to scale beyond their original size. |
| `extend` | `ex` | `0|1|true|false` | Pad the image to match the target size after resizing. |
| `padding` | `pd` | `top[:right][:bottom][:left]` | Add padding (pixels). 1, 2, or 4 values accepted. |
| `min_width` | `mw` | `value` | Ensure the result is at least this width (scales up if needed). |
| `min_height` | `mh` | `value` | Ensure the result is at least this height. |
| `zoom` | `z` | `factor` | Multiply dimensions by the given zoom factor. |
| `crop` | — | `x:y:width:height` | Extract a rectangle before other operations. |
| `rotation` | `or` | `90|180|270` | Rotate in 90° increments. |
| `auto_rotate` | `ar` | `0|1|true|false` | Respect EXIF orientation metadata (default `true`). |
| `blur` | `bl` | `sigma` | Apply Gaussian blur. |
| `sharpen` | `sh` | `sigma` | Sharpen with the given sigma. |
| `pixelate` | `px` | `amount` | Pixelate by downscaling and upscaling. |
| `background` | `bg` | `RRGGBB[AA]` | Fill background for padding/extend/flatten (hex RGBA). |
| `quality` | `q` | `1-100` | Output quality (JPEG/WebP/AVIF/etc.). |
| `format` | — | `jpeg|png|webp|avif|...` | Requested output format. |
| `dpr` | — | `1.0-5.0` | Device pixel ratio multiplier for width/height/padding. |
| `raw` | — | — | Bypass the worker semaphore (disables concurrency throttling). |
| `cache_buster` | — | `token` | Append an arbitrary token to modify the cache key. |
| `max_src_resolution` | — | `megapixels` | Request-level override for resolution guard (requires `IMGFORGE_ALLOW_SECURITY_OPTIONS`). |
| `max_src_file_size` | — | `bytes` | Request-level override for file size guard (requires `IMGFORGE_ALLOW_SECURITY_OPTIONS`). |
| `watermark` | `wm` | `opacity:position` | Blend a watermark image with the given opacity (0–1) and position. |
| `watermark_url` | `wmu` | `base64url(url)` | Source for the watermark image. |

The output format specified via `@extension` or `format` must be supported by libvips (JPEG, PNG, WebP, AVIF, TIFF, etc.).

## Geometry and resizing

### `resize:type:width:height[:enlarge][:extend]`

Resizes using one of the following strategies:

- `fill` – Crop to exactly match the target dimensions using the specified `gravity`.
- `fit` – Fit inside the bounding box while preserving aspect ratio.
- `force` – Stretch to the exact dimensions (aspect ratio may change).
- `auto` – Chooses `fill` or `fit` depending on source vs target orientation.

Leaving width or height at `0` lets imgforge infer the missing dimension from the original aspect ratio. Set `enlarge` or `extend` arguments to `true`/`1` to allow upscaling or to pad instead of cropping.

Example:

```
resize:fill:1200:675:true:false/gravity:north
```

### `size:width:height[:enlarge][:extend]`

Alias for `resize` with implicit `fit`. Useful for simple boxes:

```
size:800:0:true:false
```

### `width` / `height`

If only a single dimension is needed you can omit `resize` entirely:

```
width:1024/quality:80/plain/<src>
```

The parser will inject a `fit` resize using the supplied width/height.

### `gravity`

Controls how `fill`, cropping, and extending position the image. Accepted values mirror imgproxy (`center`, compass points like `north_west`, etc.).

### `padding`

Adds transparent (or background-colored) borders. Provide 1 value (uniform), 2 values (`top/bottom:right/left`), or 4 values (`top:right:bottom:left`). DPR scaling is applied before padding is embedded.

### `extend`

When `true`, imgforge embeds the image onto a canvas sized after `resize` to avoid cropping. Combine with `background` to fill the empty area.

### Minimum dimension and zoom

- `min_width`/`min_height` upscale only when the image is smaller than the target.
- `zoom` multiplies both dimensions (useful for incremental zoom sliders).

## Cropping and rotation

- `crop:x:y:width:height` extracts a sub-rectangle before resize/extend.
- `rotation` rotates by 90/180/270 degrees.
- `auto_rotate` toggles EXIF orientation handling (defaults to enabled).

Example: `crop:100:50:400:400/rotation:90`

## Output control

- `format:webp` – Convert to WebP (equivalent to appending `@webp`).
- `quality:85` – Compression quality for JPEG/WebP/AVIF (clamped to 1–100). Defaults to 85 when unspecified.
- `background:FFFFFFFF` – Flatten transparent backgrounds against white. When outputting JPEG, background colors are applied automatically if provided.
- `dpr:2` – Multiply target dimensions and padding by the device pixel ratio (max 5.0).

## Effects

- `blur:2.5` – Gaussian blur with sigma 2.5.
- `sharpen:1.2` – Sharpen using libvips’ sharpen operator.
- `pixelate:20` – Downscale by `1/amount` then upscale, producing a mosaic effect.

Effects can be chained; they run after resizing and before final encoding.

## Source safeguards

Set global defaults via environment variables and optionally allow per-request overrides when `IMGFORGE_ALLOW_SECURITY_OPTIONS=true`:

- `max_src_resolution:24` – Reject images above 24 megapixels.
- `max_src_file_size:5242880` – Reject images larger than 5 MiB.

If the feature flag is disabled, request-level values are ignored and only server-side configuration is enforced.

## Watermarking

Watermarks require two pieces of information:

1. `watermark:opacity:position` – Opacity is a float (e.g. `0.25`). Position accepts the same compass values as `gravity` plus `center`.
2. `watermark_url:<base64>` – Base64 URL-safe (no padding) encoding of the remote watermark image URL.

Example workflow:

```bash
WM_URL=$(printf 'https://cdn.example.com/watermarks/logo.png' | base64 | tr '+/' '-_' | tr -d '=')
URL="/resize:fit:800:0/watermark:0.3:south_east/watermark_url:${WM_URL}/plain/https://example.com/image.jpg"
```

If `IMGFORGE_WATERMARK_PATH` is set, requests can omit `watermark_url` and the server will read the watermark from disk instead.

## Cache control

`cache_buster:<token>` appends arbitrary data to the cache key. Changing the token forces a miss even if all other options match.

## Concurrency and raw mode

`raw` bypasses imgforge’s semaphore, processing the request without acquiring a worker permit. This option is equivalent to imgproxy’s `raw` flag and should be used sparingly to avoid saturating system resources.

## Compatibility with imgproxy

imgforge intentionally mirrors imgproxy’s naming and semantics so existing URL builders can be reused. Notable differences:

- Libvips formats and color-management behavior mirror imgproxy, but not every advanced imgproxy feature (e.g. presets, progressive settings) is implemented yet.
- Unknown options are silently ignored; keep builders and test suites in sync with this document.

For endpoint usage and signature details, see [doc/usage.md](usage.md).
