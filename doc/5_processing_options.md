# 5. Processing Options

Transformations live in the URL path. Each directive is `name:arg1:arg2`, and directives chain with `/`.

Unrecognised directive *names* are ignored rather than rejected, so a typo silently drops that transformation instead of failing the request. Invalid *arguments* to a known directive do return `400`. Cover your URL-building code with tests.

## Quick reference

| Option                  | Aliases    | Arguments                                                   | Purpose & defaults                                                                                           |
| ----------------------- | ---------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `preset`                | `pr`       | `name`                                                      | References a named preset defined via `IMGFORGE_PRESETS`. See [Configuration](3_configuration.md).           |
| `resize`                | `rs`       | `type:width:height[:enlarge][:extend]`                      | Primary resize control. Defaults to no resize. `enlarge`/`extend` default to `false`.                        |
| `size`                  | `s`        | `width:height[:enlarge][:extend]`                           | Convenience wrapper for `resize` with implicit `fit`.                                                        |
| `resizing_type`         | `rt`       | `type`                                                      | Overrides the mode used by other resizing directives.                                                        |
| `resizing_algorithm`    | `ra`       | `algorithm`                                                 | Interpolation kernel for resize operations. Defaults to `lanczos3`.                                          |
| `width`                 | `w`        | `value`                                                     | Sets a target width (infers height). Implies `fit`.                                                          |
| `height`                | `h`        | `value`                                                     | Sets a target height (infers width). Implies `fit`.                                                          |
| `gravity`               | `g`        | `anchor`                                                    | Controls crop/fill anchoring (`ce`, `noea`, etc.). Defaults to `ce`.                                         |
| `flip`                  | `fl`       | `horizontal[:vertical]`                                     | Flips the image horizontally and/or vertically. Defaults to no flip.                                         |
| `enlarge`               | `el`       | `bool`                                                      | Allows upscaling globally. Defaults to `false`.                                                              |
| `extend`                | `ex`       | `bool`                                                      | Pads to target dimensions after resize. Defaults to `false`.                                                 |
| `padding`               | `pd`       | `top[:right][:bottom][:left]`                               | Adds padding after resizing. Defaults to zero padding.                                                       |
| `min-width`             | `mw`       | `value`                                                     | Floor for result width. Upscales regardless of `enlarge`.                                                    |
| `min-height`            | `mh`       | `value`                                                     | Floor for result height. Upscales regardless of `enlarge`.                                                   |
| `zoom`                  | `z`        | `factor`                                                    | Multiplies dimensions after resizing. Defaults to `1.0`.                                                     |
| `crop`                  | `c`        | `width:height[:gravity]`                                    | Crops before resizing. Gravity positions the window. No crop by default.                                     |
| `rotate`                | `rot`      | `0\|90\|180\|270`                                           | Applies fixed rotation. Defaults to `0`.                                                                     |
| `auto_rotate`           | `ar`       | `bool`                                                      | Honours EXIF orientation (`true` by default).                                                                |
| `adjust`                | `a`        | `brightness[:contrast[:saturation]]`                        | Meta-option for brightness, contrast, and saturation. Saturation is applied; brightness/contrast are parsed. |
| `brightness`            | `br`       | `-255..255`                                                 | Parsed for imgproxy compatibility.                                                                           |
| `contrast`              | `co`       | `factor`                                                    | Parsed for imgproxy compatibility.                                                                           |
| `saturation`            | `sa`       | `factor`                                                    | Adjusts saturation when the image has RGB/RGBA bands. Defaults to `1.0`.                                     |
| `blur`                  | `bl`       | `sigma`                                                     | Gaussian blur (0 disables).                                                                                  |
| `sharpen`               | `sh`       | `sigma`                                                     | Sharpens edges.                                                                                              |
| `pixelate`              | `pix`      | `amount`                                                    | Pixelation strength.                                                                                         |
| `background`            | `bg`       | `RRGGBB[AA]`                                                | Canvas colour for extend/padding/flatten. Defaults to transparent unless JPEG output.                        |
| `background_alpha`      | `bga`      | `0.0-1.0`                                                   | Sets the alpha channel for `background`.                                                                     |
| `quality`               | `q`        | `1-100`                                                     | Compression quality. Defaults to `85` for lossy formats.                                                     |
| `format_quality`        | `fq`       | `format:quality...`                                         | Per-format quality overrides used when `quality` is omitted.                                                 |
| `format`                | `f`, `ext` | `jpeg\|png\|webp\|avif\|...`                                | Output format. Defaults to the source image's format; see `IMGFORGE_DEFAULT_FORMAT`.                         |
| `max_bytes`             | `mb`       | `bytes`                                                     | Re-encodes lossy formats at lower quality until the byte target is reached or quality reaches `1`.           |
| `strip_metadata`        | `sm`       | `bool`                                                      | Drops encoder metadata when supported by the output format.                                                  |
| `strip_color_profile`   | `scp`      | `bool`                                                      | Drops color profile metadata with the same encoder path as metadata stripping.                               |
| `jpeg_options`          | `jpgo`     | `progressive:no_subsample:trellis:dering:scans:quant_table` | Advanced JPEG encoder switches.                                                                              |
| `png_options`           | `pngo`     | `interlaced:quantize:colors`                                | Advanced PNG encoder switches.                                                                               |
| `webp_options`          | `webpo`    | `lossless:smart_subsample:preset`                           | Advanced WebP encoder switches.                                                                              |
| `avif_options`          | `avifo`    | `no_subsample`                                              | Advanced AVIF/HEIF encoder switches.                                                                         |
| `page`                  | `pg`       | `page`                                                      | Parses requested multi-page/animation page for imgproxy-compatible URLs.                                     |
| `pages`                 | `pgs`      | `count`                                                     | Parses requested multi-page/animation page count.                                                            |
| `disable_animation`     | `da`       | `bool`                                                      | Parses animation disable intent for compatibility.                                                           |
| `dpr`                   | —          | `1.0-5.0`                                                   | Device pixel ratio multiplier. Defaults to `1.0`.                                                            |
| `raw`                   | —          | —                                                           | Skips the concurrency semaphore. Defaults to disabled.                                                       |
| `cachebuster`           | `cb`       | `token`                                                     | Alters the cache key.                                                                                        |
| `expires`               | `exp`      | `unix_timestamp`                                            | Returns `404` after the timestamp.                                                                           |
| `filename`              | `fn`       | `filename[:encoded]`                                        | Sets `Content-Disposition` filename.                                                                         |
| `return_attachment`     | `att`      | `bool`                                                      | Uses `attachment` instead of `inline` when `filename` is set.                                                |
| `skip_processing`       | `skp`      | `extension...`                                              | Parses source format skip hints for signed URL compatibility.                                                |
| `max_src_resolution`    | `msr`      | `megapixels`                                                | Request-level override. Requires server opt-in.                                                              |
| `max_src_file_size`     | `msfs`     | `bytes`                                                     | Request-level override. Requires server opt-in.                                                              |
| `max_result_dimension`  | `mrd`      | `pixels`                                                    | Request-level override of the output size ceiling. Requires server opt-in.                                   |
| `watermark`             | `wm`       | `opacity:position`                                          | Enables watermarking. Requires watermark asset.                                                              |
| `watermark_url`         | `wmu`      | `base64url(url)`                                            | Fetches watermark per request. Overrides server default path.                                                |

## Presets

### `preset:name`

Presets provide a way to define reusable sets of processing options via the `IMGFORGE_PRESETS` environment variable. Instead of repeating complex transformation chains in every URL, reference a preset by name.

**Example configuration:**

```bash
export IMGFORGE_PRESETS="thumbnail=resize:fit:150:150/quality:80,banner=resize:fill:1200:300/quality:90"
```

**URL usage:**

```
/signature/preset:thumbnail/aHR0cHM6Ly9leGFtcGxlLmNvbS9pbWFnZS5qcGc   # base64 URL-safe
/signature/pr:banner/plain/https%3A%2F%2Fexample.com%2Fhero.jpg@webp  # plain URL with @extension
```

**Behavior:**
- The `preset:name` (or `pr:name`) directive expands to the preset's defined options at processing time.
- Multiple presets can be chained: `/preset:base/preset:quality_high/encoded_url`.
- URL-specific options override preset values when the same parameter appears in both.
- A preset named `default` automatically applies to every request before other options or presets.

**Presets-only mode:**

Set `IMGFORGE_ONLY_PRESETS=true` to restrict URLs to preset references only. Non-preset options will return `400 Bad Request`. This is useful for enforcing strict governance over allowed transformations.

Both URL-safe base64 and `plain/` source references work with presets; choose whichever format fits your signing helper.

See [Presets](5.2_presets.md) for comprehensive preset documentation including patterns, examples, and best practices. Configuration reference available in [Configuration](3_configuration.md).

## Geometry & resizing

### `resize:type:width:height[:enlarge][:extend]`

- **Types** – `fill`, `fit`, `force`, and `auto`. `auto` selects `fill` when orientations match and `fit` otherwise.
- **Defaults** – If width or height are omitted (or `0`), imgforge preserves aspect ratio using the provided dimension. `enlarge` and `extend` default to `false` unless explicitly set.
- **Enlarging** – `enlarge:false` (the default) means the image is never scaled *up*; it does not mean the resize is skipped. The resizing type settles the scale first, then that scale is capped so no axis grows. A 1600×400 banner asked for `resize:fit:500:500` still comes back at 500×125 — it fits the box, it just is not enlarged to fill it. Only when every axis would grow does the image pass through untouched.
- With `fill`, capping can leave the result smaller than the requested box: covering a 500×200 box from a 1000×100 source would need a 2× upscale, so the crop takes what exists and returns 500×100. Set `enlarge:true` to get the exact box. `min-width`/`min-height` are the other way to force a size — see below, they upscale regardless.
- **Extending** – `extend:true` pads the canvas to the requested size after resizing but before padding. The background colour determines the filled area.

### `size`

`size` and its aliases are shorthand for `resize:fit`. Width or height of `0` lets imgforge infer the missing dimension. Use the trailing arguments to flip `enlarge` or `extend` without switching to the long form.

### `width` / `height`

Setting a single dimension implicitly enables `fit` resizing. These options influence fallback behaviour when no explicit `resize` directive exists. `enlarge:false` still applies unless you opt in globally via the `enlarge` directive.

### `resizing_type`

`resizing_type:fill` (or similar) changes how implicit resizes behave. It affects `width`, `height`, `size`, and subsequent `resize` directives that omit the type. Place it before directives that rely on the mode to avoid surprises.

### `resizing_algorithm`

The interpolation kernel used whenever the image is scaled — by `resize`, `size`, `width`, `height`, `min-width`, `min-height`, `zoom`, `pixelate`, and watermark scaling.

- **`nearest`** – copies the nearest pixel. Hard edges, no blending. For pixel art and sprites.
- **`linear`** – bilinear. Softens detail; fine for throwaway previews.
- **`cubic`** – bicubic. Smooth, without the ringing the lanczos kernels can produce.
- **`lanczos2`** – sharp, with less ringing than `lanczos3`.
- **`lanczos3`** – **default**. Sharpest; can show faint halos on high-contrast edges.

Pick on appearance rather than speed: the kernel is rarely where the processing time goes. [Resizing Algorithms](5.1_resizing_algorithms.md) explains why, and which kernel suits which content.

### `gravity`

Gravity defaults to `ce`. It influences:

- Cropping windows when `fill` or `crop` is used.
- Canvas alignment for `extend`.
- Watermark positioning when combined with the `watermark` option (gravity only applies if you omit an explicit watermark position).

imgforge accepts imgproxy's gravity anchors: `ce`, `no`, `so`, `ea`, `we`, `noea`, `nowe`, `soea`, and `sowe`.

### Minimum dimensions & zoom

- `min-width` and `min-height` trigger an extra resize pass when the image is still smaller after primary resizing. This pass **upscales regardless of `enlarge`** — the minimums are a floor, and `enlarge:false` does not override them. Use them only when you actually want a guaranteed size.
- That pass scales both axes by the same factor, so aspect ratio is preserved: `min-width:500` on a 100×100 image returns 500×500, not 500×100.
- `zoom` multiplies dimensions after resizing and minimum checks. Values < 1 shrink the image; values > 1 enlarge it even if `enlarge` is `false`.

### `padding`

- Accepts 1, 2, or 4 integers representing pixels.
- Padding runs after resizing/extend, so it doesn’t influence aspect ratio.
- `dpr` scaling multiplies all padding values before rendering.
- Transparent padding respects the output format: JPEG outputs are flattened against the background colour.

## Cropping & rotation

### `crop`

`crop:width:height[:gravity]` executes before any resizing, isolating a region for the rest of the pipeline to work on.

There are no x/y coordinates in the URL form: **gravity is what positions the crop window**. Without it the region is taken from the top-left. `crop:300:200:soea` takes a 300×200 region from the bottom-right corner. A width or height of `0` means "the full source extent in that direction", and both are clamped to the source, so asking for more than exists yields the whole image rather than an error.

### `auto_rotate` and `rotate`

- `auto_rotate` defaults to `true`, applying EXIF orientation automatically. Disable (`auto_rotate:false`) when you need the raw sensor orientation.
- `rotate` applies an explicit 90° multiple after auto-rotation and resizing. Non-right-angle values are ignored.
- `flip` runs after rotation and flips horizontally, vertically, or both depending on boolean arguments.

## Output control

### `format`

If omitted, imgforge keeps the **source image's format** (a PNG stays a PNG, preserving transparency; imgproxy-compatible). Set `IMGFORGE_DEFAULT_FORMAT` to a concrete format to fix the default instead (see [Configuration](3_configuration.md)); sources that can't be sniffed or that this libvips build can't encode fall back to JPEG. Provide an explicit format (`webp`, `png`, `avif`, etc.) or use the `@extension` suffix following the source URL to override per request. Some formats may not be available if libvips lacks support.

### `quality`

Defaults to `85` for lossy codecs (JPEG, WebP, AVIF). `quality` is ignored for lossless formats such as PNG. Raising quality increases file size and processing time; lowering it can introduce artefacts.

### `format_quality`

`format_quality:webp:80:jpeg:90` supplies per-format defaults when `quality` is not set. Explicit `quality` always wins.

### Encoder options

- `max_bytes` repeatedly lowers quality for supported lossy encoders until output fits the byte budget or reaches quality `1`.
- `strip_metadata` and `strip_color_profile` map to libvips metadata retention controls for formats that expose them.
- `jpeg_options` maps to progressive JPEG, chroma subsampling, trellis quantization, overshoot deringing, optimized scans, and quant table controls.
- `png_options` maps to interlacing and palette quantization controls.
- `webp_options` maps to lossless, smart chroma subsampling, and encoder preset controls. Preset names outside libvips' own set (`default`, `picture`, `photo`, `drawing`, `icon`, `text`) are accepted in URLs but ignored by the encoder.
- `avif_options` maps to AVIF/HEIF chroma subsampling.

WebP, AVIF, HEIF, and GIF are encoded through the libvips save suffix (`.webp[Q=80,keep=all]` and friends) rather than the libvips crate's generated save bindings. Those bindings name encoder properties that only exist in libvips 8.16 and later — `exact` on webpsave, `tune` on heifsave, `keep-duplicate-frames` on gifsave — and an older libvips rejects the whole call with `no property named ...`, so nothing encodes. Ubuntu 24.04, the base of the published image, ships libvips 8.15.1, which is exactly that case. JPEG, PNG, and TIFF use the generated bindings; every property they name predates 8.15.

Formats also depend on what your libvips was compiled with. A build without an HEVC encoder advertises `heif` support but fails at encode time with `Unsupported compression`; AVIF, which uses a different codec, is unaffected.

### `background`

Accepts RGB or RGBA hex (`FFFFFF` or `FFFFFFFF`). The colour fills areas introduced by `extend` or `padding`. When outputting JPEG, imgforge automatically flattens transparency against the background colour. Without a background, JPEG outputs fall back to black.

`background` also accepts imgproxy's `R:G:B` channel form. `background_alpha` can be supplied before or after `background` to set the alpha channel.

### `dpr`

- Defaults to `1.0` and caps at `5.0`.
- Scales the resize width, height, and padding. `min-width` and `min-height` are not scaled.
- Only values above `1.0` have any effect; the source-image safeguards are evaluated before this runs, so `dpr` cannot trip them.
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

### `adjust`, `brightness`, `contrast`, and `saturation`

`adjust` is parsed as `brightness:contrast:saturation`, matching imgproxy's meta-option shape. `saturation` is applied for RGB/RGBA images. `brightness` and `contrast` are accepted and validated for URL compatibility, but the current libvips Rust crate does not publicly expose the needed `linear` operation, so those two controls are not applied yet.

## Watermarking

1. Add `watermark:<opacity>:<position>` to enable overlay. Opacity ranges from `0.0` (invisible) to `1.0` (solid). Position accepts the same anchors as gravity (e.g., `soea`).
2. Supply the watermark image via `watermark_url:<base64url>` or configure `IMGFORGE_WATERMARK_PATH` on the server (see [Configuration](3_configuration.md) for details). When both are present, the URL value wins.
3. Watermarks render after resizing, padding, and effects. Oversized or missing watermark assets fail the request with `400 Bad Request`.

## Cache control & concurrency

- `cachebuster:<token>` appends arbitrary data to the cache key. Change the token when you want to force reprocessing without altering transformations. See [Caching](7_caching.md) for more details on cache behavior.
- `raw` bypasses the concurrency semaphore that ordinarily limits the number of simultaneous libvips jobs. Reserve it for high-priority tasks; uncontrolled usage can starve other requests.
- `expires:<unix_timestamp>` rejects stale URLs with `404`.
- `filename:<name>` sets `Content-Disposition`; add `:true` when the filename is URL-safe Base64 encoded.
- `return_attachment:true` makes filename responses use `attachment`; otherwise they use `inline`.
- `skip_processing`, `page`, `pages`, and `disable_animation` are accepted for signed URL compatibility. Full multi-page and animation source loading is not yet implemented in the current single-image decode path.

## Security overrides

`max_src_resolution`, `max_src_file_size`, and `max_result_dimension` override the server-wide safeguards for a single request. They only take effect when `IMGFORGE_ALLOW_SECURITY_OPTIONS=true` is set (see [Configuration](3_configuration.md)); otherwise the configured value applies and the directive is ignored. Use cautiously, preferably on trusted internal URLs.

`max_result_dimension:<pixels>` caps the width and height of the *processed* image, where the other two cap the source. A request whose result would exceed it fails with `400 Bad Request`, and the response body names both the result size and the limit. The check runs after the full transformation chain — so it sees `dpr`, padding, `extend`, `zoom`, and the minimum dimensions — but before encoding, so an over-limit request never materialises the pixels.

Cached responses are namespaced by the effective limit, so entries stored under a higher ceiling (or none at all) are not served after you lower it. Turning the option on does not invalidate a cache that was not using it.

## When output does not match the URL

Because unknown directive names are silently ignored, an option that appears to do nothing is usually a spelling mistake. Check the name against the quick reference above, then check the order: [Image Processing Pipeline](12_image_processing_pipeline.md) documents which stage each directive runs in, and several surprises (padding after resize, gravity ignored by explicit crops, minimums overriding zoom) come from ordering rather than the directive itself.
