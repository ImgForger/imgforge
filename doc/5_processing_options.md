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
| `gravity`               | `g`        | `type[:x_offset[:y_offset]]`                                | Controls crop/fill anchoring (`ce`, `noea`, `fp`, ...). Defaults to `ce:0:0`.                                |
| `flip`                  | `fl`       | `horizontal[:vertical]`                                     | Flips the image horizontally and/or vertically. Defaults to no flip.                                         |
| `enlarge`               | `el`       | `bool`                                                      | Allows upscaling globally. Defaults to `false`.                                                              |
| `extend`                | `ex`       | `bool[:gravity]`                                            | Pads to target dimensions after resize. Defaults to `false:ce:0:0`.                                          |
| `extend_aspect_ratio`   | `exar`, `extend_ar` | `bool[:gravity]`                                   | Pads to the target aspect ratio without reaching its size. Defaults to `false:ce:0:0`.                       |
| `padding`               | `pd`       | `top[:right[:bottom[:left]]]`                               | Adds padding after resizing, CSS shorthand rules. Defaults to zero padding.                                  |
| `min-width`             | `mw`, `min_width` | `value`                                              | Floor for result width. Upscales regardless of `enlarge`.                                                    |
| `min-height`            | `mh`, `min_height` | `value`                                             | Floor for result height. Upscales regardless of `enlarge`.                                                   |
| `zoom`                  | `z`        | `factor` or `zoom_x:zoom_y`                                 | Multiplies dimensions after resizing. Defaults to `1.0`.                                                     |
| `crop`                  | `c`        | `width:height[:gravity]`                                    | Crops before resizing. Values below 1 are a fraction of the source. Gravity positions the window.            |
| `trim`                  | `t`        | `threshold[:color[:equal_hor[:equal_ver]]]`                 | Removes a uniform border before cropping and resizing. Ignored for animated sources.                         |
| `rotate`                | `rot`      | `0\|90\|180\|270`                                           | Applies fixed rotation. Defaults to `0`.                                                                     |
| `auto_rotate`           | `ar`       | `bool`                                                      | Honours EXIF orientation (`true` by default).                                                                |
| `adjust`                | `a`        | `brightness[:contrast[:saturation]]`                        | Meta-option for brightness, contrast, and saturation.                                                        |
| `brightness`            | `br`       | `-255..255`                                                 | Added to every colour channel. Defaults to `0`.                                                              |
| `contrast`              | `co`       | `factor`                                                    | Multiplier applied around mid-grey. Defaults to `1.0`.                                                       |
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
| `strip_color_profile`   | `scp`      | `bool`                                                      | Drops the embedded colour profile, leaving other metadata alone.                                             |
| `keep_copyright`        | `kcr`      | `bool`                                                      | Retains the EXIF copyright and artist tags across a metadata strip. JPEG output only.                        |
| `preserve_hdr`          | `ph`       | `bool`                                                      | Keeps a high bit-depth image high bit-depth and carries its gain map through. Needs libvips 8.16+.           |
| `enforce_thumbnail`     | `eth`      | `bool`                                                      | Uses the source's embedded EXIF thumbnail instead of the full image when one is present.                     |
| `jpeg_options`          | `jpgo`     | `progressive:no_subsample:trellis:dering:scans:quant_table` | Advanced JPEG encoder switches.                                                                              |
| `png_options`           | `pngo`     | `interlaced:quantize:colors`                                | Advanced PNG encoder switches.                                                                               |
| `webp_options`          | `webpo`    | `lossless:smart_subsample:preset`                           | Advanced WebP encoder switches.                                                                              |
| `avif_options`          | `avifo`    | `no_subsample`                                              | Advanced AVIF/HEIF encoder switches.                                                                         |
| `page`                  | `pg`       | `page`                                                      | First page of a multi-page or animated source to read. Defaults to `0`.                                      |
| `pages`                 | `pgs`      | `count`                                                     | How many pages to read. Defaults to all of them for an animated output format, one otherwise.                |
| `disable_animation`     | `da`       | `bool`                                                      | Collapses an animated source to its first frame.                                                             |
| `dpr`                   | —          | `1.0-5.0`                                                   | Device pixel ratio multiplier. Defaults to `1.0`.                                                            |
| `raw`                   | —          | `[bool]`                                                    | Returns the source bytes untouched. Defaults to disabled.                                                    |
| `cachebuster`           | `cb`       | `token`                                                     | Alters the cache key.                                                                                        |
| `expires`               | `exp`      | `unix_timestamp`                                            | Returns `404` after the timestamp.                                                                           |
| `filename`              | `fn`       | `filename[:encoded]`                                        | Sets `Content-Disposition` filename.                                                                         |
| `return_attachment`     | `att`      | `bool`                                                      | Uses `attachment` instead of `inline` when `filename` is set.                                                |
| `skip_processing`       | `skp`      | `extension...`                                              | Returns the source untouched when its format is listed and no conversion was asked for.                      |
| `max_src_resolution`    | `msr`      | `megapixels`                                                | Request-level override. Requires server opt-in.                                                              |
| `max_src_file_size`     | `msfs`     | `bytes`                                                     | Request-level override. Requires server opt-in.                                                              |
| `max_result_dimension`  | `mrd`      | `pixels`                                                    | Request-level override of the output size ceiling. Requires server opt-in.                                   |
| `max_animation_frames`  | `maf`      | `count`                                                     | Request-level override of the animation frame ceiling. Requires server opt-in.                               |
| `max_animation_frame_resolution` | `mafr` | `megapixels`                                        | Request-level override of the per-frame resolution ceiling. Requires server opt-in.                          |
| `watermark`             | `wm`       | `opacity[:position[:x_offset[:y_offset[:scale]]]]`          | Enables watermarking. Requires a watermark asset.                                                            |
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

- **Types** – `fit`, `fill`, `fill-down`, `force`, and `auto`. `auto` selects `fill` when the source and the box share an orientation and `fit` otherwise. `fill-down` is `fill` except that a result which came out smaller than the box is cropped to the box's *aspect ratio* rather than left at its own shape — use it when the composition matters more than the exact size.
- **Defaults** – If width or height are omitted (or `0`), imgforge preserves aspect ratio using the provided dimension. `enlarge` and `extend` default to `false` unless explicitly set.
- **Enlarging** – `enlarge:false` (the default) means the image is never scaled *up*; it does not mean the resize is skipped. The resizing type settles the scale first, then that scale is capped so no axis grows. A 1600×400 banner asked for `resize:fit:500:500` still comes back at 500×125 — it fits the box, it just is not enlarged to fill it. Only when every axis would grow does the image pass through untouched.
- With `fill`, capping can leave the result smaller than the requested box: covering a 500×200 box from a 1000×100 source would need a 2× upscale, so the crop takes what exists and returns 500×100. Set `enlarge:true` to get the exact box. `min-width`/`min-height` are the other way to force a size — see below, they upscale regardless.
- **Extending** – `extend:true` pads the canvas to the requested size after resizing but before padding. The background colour determines the filled area. `extend` takes its own gravity — `extend:true:so` pins the image to the bottom of the new canvas — which is independent of the request's `gravity`.

### `extend_aspect_ratio`

`extend_aspect_ratio:true[:gravity]` (`exar`, `extend_ar`) pads out to the *shape* the resize asked for without reaching its size. Where `extend` fills the canvas to exactly `width`x`height`, this grows only the short axis until the ratio matches, leaving every source pixel at the size the resize produced. It is what you want for a uniform grid of thumbnails whose sources have mixed aspect ratios but whose displayed size should follow the image.

It has no effect with `force`, which already hit the requested box exactly.

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

`gravity:type[:x_offset[:y_offset]]`, defaulting to `ce:0:0`. It influences:

- Cropping windows when `fill`, `fill-down`, or `crop` is used.
- Canvas alignment for `extend` and `extend_aspect_ratio`, each of which can also carry its own.
- Watermark positioning, via the `watermark` option's own position argument.

imgforge accepts imgproxy's gravity anchors: `ce`, `no`, `so`, `ea`, `we`, `noea`, `nowe`, `soea`, `sowe`, and `fp`.

**Offsets** nudge the window away from its anchor. A magnitude of 1 or more is a pixel count; anything smaller is a fraction of the axis being positioned. `gravity:no:0:20` takes the window from 20px below the top edge; `gravity:no:0:0.1` takes it from a tenth of the way down. The window is still clamped to the image, so an offset cannot push it off the edge.

**Focus point** — `gravity:fp:x:y` — reads the two arguments as coordinates between 0 and 1 and centres the result on that point. `gravity:fp:0.5:0.25` keeps the middle of the upper quarter in view, which is the usual answer for portraits where a centre crop cuts off the head.

### Minimum dimensions & zoom

- `min-width` and `min-height` trigger an extra resize pass when the image is still smaller after primary resizing. This pass **upscales regardless of `enlarge`** — the minimums are a floor, and `enlarge:false` does not override them. Use them only when you actually want a guaranteed size.
- That pass scales both axes by the same factor, so aspect ratio is preserved: `min-width:500` on a 100×100 image returns 500×500, not 500×100.
- `zoom` multiplies dimensions after resizing and minimum checks. Values < 1 shrink the image; values > 1 enlarge it even if `enlarge` is `false`. A second argument gives the axes independent factors: `zoom:2:1` doubles the width alone.

### `padding`

- Accepts 1 to 4 integers representing pixels, following the CSS shorthand: one value pads every side, two are vertical then horizontal, three are top, horizontal, bottom, and four are top, right, bottom, left.
- Padding runs after resizing/extend, so it doesn’t influence aspect ratio.
- `dpr` scaling multiplies all padding values before rendering.
- Transparent padding respects the output format: JPEG outputs are flattened against the background colour.

## Cropping & rotation

### `crop`

`crop:width:height[:gravity]` executes before any resizing, isolating a region for the rest of the pipeline to work on.

There are no x/y coordinates in the URL form: **gravity is what positions the crop window**, and it accepts the same offsets and focus point as the `gravity` option. `crop:300:200:soea` takes a 300×200 region from the bottom-right corner; `crop:300:200:fp:0.5:0.3` centres it on a point. Without a gravity of its own, the crop falls back to the request's `gravity`, which defaults to centre.

**Fractional extents.** A width or height below 1 is read as a fraction of the source, so `crop:0.5:0.5` takes the middle quarter of any image whatever its size. A value of `0` means "the full source extent in that direction", and everything is clamped to the source, so asking for more than exists yields the whole image rather than an error.

### `trim`

`trim:threshold[:color[:equal_hor[:equal_ver]]]` removes a uniform border, running before crop and resize so
everything after it sees the trimmed extent.

- **`threshold`** — how far a pixel may differ from the background and still be treated as part of the border. `10`
  is a reasonable starting point; larger values trim more aggressively.
- **`color`** — hex colour to treat as background. Omit it, or leave it empty, and imgforge reads the top-left pixel
  and uses that, which is what you want for a border of any colour. Naming a colour inverts what counts as content,
  so `trim:10:ff0000` on a red-bordered image trims the red.
- **`equal_hor` / `equal_ver`** — cut the same amount from both sides, so a subject that is off-centre keeps its
  position instead of shifting toward the thicker border.

An image that is entirely background is returned untouched rather than reduced to nothing.

**Animated sources ignore `trim`**, with a warning in the log, and the rest of the request proceeds normally.
Trim measures one image's borders, and every frame of an animation has its own: a subject that grows across the
animation trims to a different width in each frame, which no animated container can hold — the frames share a
single canvas. Refusing the request would fail an otherwise reasonable URL over an option that simply does not
apply to it, so the option is dropped instead. imgproxy behaves the same way.

The background is matched to the image: libvips wants one value per non-alpha band, so greyscale sources reduce an
explicit colour to its luminance, and a CMYK source refuses one outright — omit the colour there and let it be
detected, since an sRGB value has no meaningful reading against four ink channels.

Two costs worth knowing. Trimming has to examine the pixels, so it **disables scale-on-load**: the trimmed size is
not knowable in advance, so there is no safe decode scale to choose, and the source is read at full resolution.
Reach for it when you need it, not by default, and be wary of it on large sources.

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

Every format is encoded through the libvips save suffix (`.webp[Q=80,keep=all]` and friends) rather than the crate's generated save bindings. Those bindings name encoder properties that only exist in libvips 8.16 and later — `exact` on webpsave, `tune` on heifsave, `keep-duplicate-frames` on gifsave — and an older libvips rejects the whole call with `no property named ...`, so nothing encodes at all. The suffix parser sets only the options named, which keeps one code path working across libvips versions, and it is also the only form that can express a *combination* of metadata `keep` flags.

That combination is why `strip_metadata` and `strip_color_profile` are now independent: stripping the colour profile keeps `keep=exif|xmp|iptc|other`, and stripping metadata keeps `keep=icc`. Previously either one dropped everything.

Formats also depend on what your libvips was compiled with. imgforge probes the codec-backed formats by encoding a real pixel once at startup, so a build with no AV1 or HEVC encoder reports AVIF or HEIF as an unsupported *format* — a clear `400` — instead of failing at encode time with `Unsupported compression` and a `500`.

A result too large for its output container is scaled down to fit rather than handed to an encoder that will refuse it: WebP stops at 16383px on a side, AVIF and HEIF at 16384.

### Metadata

- **`strip_metadata`** drops the descriptive tags (EXIF, XMP, IPTC) and leaves the colour profile alone. **`strip_color_profile`** does the reverse. Set both to drop everything.
- **`keep_copyright`** carries the EXIF `Copyright` and `Artist` tags across a `strip_metadata`. libvips has no copyright granularity in its `keep` flags — they are `none|exif|xmp|iptc|icc|other|gainmap|all` — so imgforge reads the two fields from the source and splices a minimal EXIF segment back into the encoded result. That mechanism is JPEG-only; other output formats strip as normal, and the option is a no-op for them.
- **`preserve_hdr`** keeps a high bit-depth source at its own depth when the output format can carry it (PNG, TIFF, AVIF, HEIF) and retains the gain map that makes the image HDR, even while other metadata is being stripped. The gain-map flag needs libvips 8.16 or later; on an older build, enabling it makes the encode fail.
- **`enforce_thumbnail`** uses the source's embedded EXIF thumbnail in place of the full image whenever one is present, which turns a large JPEG into a very cheap request. The thumbnail is usually a few hundred pixels wide, so the result is only as good as that; a thumbnail that will not decode falls back to the full image rather than failing.

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

`adjust` is parsed as `brightness:contrast:saturation`, matching imgproxy's meta-option shape; the three can also be set individually.

- **`brightness`** — `-255` to `255`, added to every colour channel. Expressed in 8-bit terms whatever the source's depth, so `brightness:64` moves a 16-bit image the same visible distance as an 8-bit one.
- **`contrast`** — a positive multiplier applied around mid-grey, so it darkens shadows and brightens highlights rather than shifting the whole image. `1.0` is unchanged.
- **`saturation`** — a positive multiplier on chroma, applied to RGB and RGBA images. `0` is greyscale, `1.0` is unchanged.

Brightness and contrast go through a single pass over the pixels, with contrast applied first. The alpha channel is left alone: brightening it would fade the image in or out rather than lighten it.

## Animation and multi-page sources

An animated GIF or WebP, or a multi-page PDF or TIFF, is read as many frames and every frame goes through the whole pipeline independently — resize, crop, rotate, pad, effects — before the frames are stacked back together and the encoder is told where they divide. Rotating an animation by 90° therefore works, which it cannot when the stacked frames are treated as one tall image.

- **`page:<n>`** — the first page to read. Defaults to `0`.
- **`pages:<n>`** — how many pages to read. Defaults to all of them when the output format can hold them (GIF, WebP, AVIF, HEIF) and one otherwise, because decoding frames that are about to be discarded is pure cost.
- **`disable_animation:true`** — collapse an animated source to its first frame.
- **`max_animation_frames`** and **`max_animation_frame_resolution`** bound what an animated source may cost. The source-resolution limit measures the whole stack; these bound the frame count and the size of one frame. See [Configuration](3_configuration.md).

`/info` reports a `pages` field so you can tell a still from an animation before deciding what to request.

## Watermarking

1. Add `watermark:<opacity>[:<position>[:<x_offset>[:<y_offset>[:<scale>]]]]` to enable the overlay. Opacity ranges from `0.0` (invisible) to `1.0` (solid). Position accepts the gravity anchors (`ce`, `soea`, ...) plus `re`, which tiles the watermark across the whole image. Offsets follow the same absolute-or-fractional rule as gravity offsets, and unlike a crop they may push part of the watermark off the edge. `scale` sets the watermark's width as a fraction of the result; imgforge defaults to `0.25`, where imgproxy leaves an unscaled watermark at its natural pixel size.
2. Supply the watermark image via `watermark_url:<base64url>` or configure `IMGFORGE_WATERMARK_PATH` on the server (see [Configuration](3_configuration.md) for details). When both are present, the URL value wins.
3. Watermarks render after resizing, padding, and effects. Oversized or missing watermark assets fail the request with `400 Bad Request`.

## Cache control & concurrency

- `cachebuster:<token>` appends arbitrary data to the cache key. Change the token when you want to force reprocessing without altering transformations. See [Caching](7_caching.md) for more details on cache behavior.
- `raw` bypasses the concurrency semaphore that ordinarily limits the number of simultaneous libvips jobs. Reserve it for high-priority tasks; uncontrolled usage can starve other requests.
- `expires:<unix_timestamp>` rejects stale URLs with `404`.
- `filename:<name>` sets `Content-Disposition`; add `:true` when the filename is URL-safe Base64 encoded.
- `return_attachment:true` makes filename responses use `attachment`; otherwise they use `inline`.
- `skip_processing:<ext>[:<ext>...]` returns the source bytes untouched when its format is listed. A request that also asks for a different format is asking for a conversion, which cannot be skipped.

## Security overrides

`max_src_resolution`, `max_src_file_size`, and `max_result_dimension` override the server-wide safeguards for a single request. They only take effect when `IMGFORGE_ALLOW_SECURITY_OPTIONS=true` is set (see [Configuration](3_configuration.md)); otherwise the configured value applies and the directive is ignored. Use cautiously, preferably on trusted internal URLs.

`max_result_dimension:<pixels>` caps the width and height of the *processed* image, where the other two cap the source. A request whose result would exceed it fails with `400 Bad Request`, and the response body names both the result size and the limit. The check runs after the full transformation chain — so it sees `dpr`, padding, `extend`, `zoom`, and the minimum dimensions — but before encoding, so an over-limit request never materialises the pixels.

Cached responses are namespaced by the effective limit, so entries stored under a higher ceiling (or none at all) are not served after you lower it. Turning the option on does not invalidate a cache that was not using it.

## When output does not match the URL

Because unknown directive names are silently ignored, an option that appears to do nothing is usually a spelling mistake. Check the name against the quick reference above, then check the order: [Image Processing Pipeline](12_image_processing_pipeline.md) documents which stage each directive runs in, and several surprises (padding after resize, gravity ignored by explicit crops, minimums overriding zoom) come from ordering rather than the directive itself.
