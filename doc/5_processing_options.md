# 5. Processing Options

imgforge encodes image transformations directly in the URL path. Each directive uses the format `name:arg1:arg2`, with multiple directives chained via `/`. Unknown options are ignored—typos silently disable transformations—so validate URLs in automated tests or internal tooling.

## Quick reference

| Option | Aliases | Arguments | Purpose |
| --- | --- | --- | --- |
| `resize` | `rs` | `type:width:height[:enlarge][:extend]` | Resize using `fill`, `fit`, `force`, or `auto`. |
| `size` | `sz`, `s` | `width:height[:enlarge][:extend]` | Convenience wrapper for `resize` with implicit `fit`. |
| `resizing_type` | `rt` | `type` | Override the resize mode when combined with other directives. |
| `width` | `w` | `value` | Target width when implicit resizing occurs. |
| `height` | `h` | `value` | Target height when implicit resizing occurs. |
| `gravity` | `g` | `anchor` | Anchor for crops, fill resizing, and extending (`center`, `north_east`, etc.). |
| `enlarge` | `el` | `bool` | Permit upscaling beyond the original size. |
| `extend` | `ex` | `bool` | Pad the image to match the target after resizing. |
| `padding` | `pd` | `top[:right][:bottom][:left]` | Add transparent/background padding. Accepts 1, 2, or 4 values. |
| `min_width` | `mw` | `value` | Ensure the result is at least this wide; scales up if necessary. |
| `min_height` | `mh` | `value` | Ensure the result is at least this tall. |
| `zoom` | `z` | `factor` | Multiply dimensions by a floating point factor. |
| `crop` | — | `x:y:width:height` | Extract a rectangle before further processing. |
| `rotation` | `or` | `90|180|270` | Rotate the image in 90° increments. |
| `auto_rotate` | `ar` | `bool` | Respect EXIF orientation metadata (`true` by default). |
| `blur` | `bl` | `sigma` | Apply Gaussian blur. |
| `sharpen` | `sh` | `sigma` | Sharpen using libvips’ operator. |
| `pixelate` | `px` | `amount` | Pixelate by shrinking then scaling back up. |
| `background` | `bg` | `RRGGBB[AA]` | Background color used for padding/extend/flatten (hex RGBA). |
| `quality` | `q` | `1-100` | Compression quality for lossy formats (JPEG/WebP/AVIF). |
| `format` | — | `jpeg|png|webp|avif|...` | Output format override. Equivalent to `@extension`. |
| `dpr` | — | `1.0-5.0` | Device pixel ratio multiplier for width, height, and padding. |
| `raw` | — | — | Bypass the concurrency semaphore (unthrottled processing). |
| `cache_buster` | — | `token` | Alters the cache key; use to bust specific variants. |
| `max_src_resolution` | — | `megapixels` | Request-level override; requires `IMGFORGE_ALLOW_SECURITY_OPTIONS=true`. |
| `max_src_file_size` | — | `bytes` | Request-level override for file size limit. |
| `watermark` | `wm` | `opacity:position` | Apply a watermark (see below). |
| `watermark_url` | `wmu` | `base64url(url)` | URL for watermark image (Base64 URL-safe, no padding). |

## Geometry & resizing

### `resize:type:width:height[:enlarge][:extend]`

- `fill` – Resizes and crops to fill the target box using `gravity`.
- `fit` – Maintains aspect ratio while fitting inside the box.
- `force` – Ignores aspect ratio, stretching to the exact size.
- `auto` – Chooses `fill` when source and target orientations match; otherwise `fit`.

Set width or height to `0` to let imgforge infer the missing dimension. Append `:true` to enable enlarging; append a fourth argument to control `extend` simultaneously (`resize:fill:800:600:true:true`).

### `size`

`size` and `sz` are shorthand for `resize` with an implicit `fit`. Provide width, height, and optional `enlarge` / `extend` flags.

### `width` / `height`

Specifying width or height alone automatically inserts a `fit` resize. Useful when only one dimension matters (`width:1200/plain/...`).

### `gravity`

Accepted values mirror imgproxy: `center`, `north`, `south`, `east`, `west`, and corner variants like `north_east`. Gravity influences cropping, `fill` resizing, and `extend` canvas alignment.

### `padding`

Provide one, two, or four integer values (pixels). A single value applies to all sides; two values apply to vertical and horizontal pairs; four values map to `top:right:bottom:left`. Padding respects `dpr` scaling when present.

### Minimum dimensions & zoom

- `min_width` / `min_height` upscale only when the image is smaller than the target.
- `zoom` multiplies dimensions by a floating-point factor, useful for progressive zoom controls.

## Cropping & rotation

- `crop:x:y:width:height` runs before resizing, letting you focus on a region of interest.
- `rotation` applies a forced rotation. Values other than multiples of 90 degrees are ignored.
- `auto_rotate:false` disables EXIF orientation handling when you prefer the raw source orientation.

## Output control

- `format:webp` converts to WebP; other supported formats include `jpeg`, `png`, `avif`, `tiff`, and more depending on libvips.
- `quality:92` sets the compression quality, clamped between 1 and 100. Defaults to 85.
- `background:FFFFFFFF` flattens transparency against white. Particularly relevant when outputting JPEG.
- `dpr:2` doubles target dimensions and padding to support Retina/HiDPI displays.

## Effects

- `blur:3.5` applies Gaussian blur with sigma 3.5.
- `sharpen:1.2` sharpens the image. Combine with resizing for crisp thumbnails.
- `pixelate:40` produces a mosaic effect; values above 100 are usually sufficient for anonymization.

Effects are applied after resizing but before encoding.

## Watermarking

Watermarking requires both configuration and per-request options:

1. Include `watermark:opacity:position` (opacity between 0.0 and 1.0). Positions mirror gravity values.
2. Provide either `watermark_url:<base64>` (Base64 URL-safe, no padding) or set `IMGFORGE_WATERMARK_PATH` so the server loads a watermark from disk.

Example:

```bash
WM_URL=$(printf 'https://cdn.example.com/assets/watermark.png' | base64 | tr '+/' '-_' | tr -d '=')
PATH="/resize:fit:800:0/watermark:0.3:south_east/watermark_url:${WM_URL}/plain/https://example.com/image.jpg"
```

## Cache control

`cache_buster:<token>` forces cache misses by introducing unique tokens into the cache key. Change the token whenever you need imgforge to reprocess a URL.

## Raw mode & safeguards

- `raw` bypasses the semaphore used to limit concurrent libvips work. Reserve this for high-priority or low-cost operations to avoid starving other requests.
- `max_src_resolution` and `max_src_file_size` can loosen server-imposed defaults when `IMGFORGE_ALLOW_SECURITY_OPTIONS=true`. Without that flag, per-request overrides are ignored.

## Validation tips

- Use [4_url_structure.md](4_url_structure.md) to verify the signed path matches the encoded options.
- Rely on [6_processing_pipeline.md](6_processing_pipeline.md) to understand when each directive runs.
- Automated test suites should call the processing endpoint with representative options to prevent regressions.
