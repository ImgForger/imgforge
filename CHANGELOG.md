# Changelog

All notable changes to imgforge are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). While imgforge is
pre-1.0, minor versions may carry breaking changes; those are called out explicitly.

Entries start at 0.10.0. For earlier history, see the
[GitHub releases](https://github.com/ImgForger/imgforge/releases) and the git log.

## [Unreleased]

_No changes yet._

## [0.18.0] - 2026-08-16

The compatibility release. Every option in imgproxy's free tier is now implemented rather than parsed, and the
response carries the headers a CDN and a browser expect. Several defaults change to match imgproxy; those are
listed under **Changed** and are the reason this is a minor bump rather than a patch.

### Added

- **Animated and multi-page sources.** `page`, `pages`, and `disable_animation` reach the loader, and an animated
  GIF or WebP is processed frame by frame and re-encoded as an animation. libvips hands an animation over as one
  tall stack of frames and does not update the frame height when the geometry changes, so scaling the stack as a
  single image silently reinterprets four 80px frames as two 160px ones. Splitting it, running each frame through
  the whole pipeline, and telling the encoder the new frame height means rotation, padding, and cropping work on an
  animation as they do on a still. `/info` gained a `pages` field.

  Two new ceilings come with it, because an animation multiplies every cost by its frame count and the source
  resolution limit does not bound that: `IMGFORGE_MAX_ANIMATION_FRAMES` and
  `IMGFORGE_MAX_ANIMATION_FRAME_RESOLUTION`, with `maf` and `mafr` as request-level overrides.

- **Content negotiation.** `IMGFORGE_ENABLE_WEBP_DETECTION` and `IMGFORGE_ENABLE_AVIF_DETECTION` serve a modern
  format to clients whose `Accept` advertises it, where the URL left the format open; `IMGFORGE_ENFORCE_WEBP` and
  `IMGFORGE_ENFORCE_AVIF` do so over the top of an explicit format. The negotiated format is part of the cache key
  and the response carries `Vary: Accept`, so a shared cache cannot hand an AVIF to a client that said it could not
  read one. A format this libvips build cannot encode is skipped rather than attempted.

  `IMGFORGE_ENABLE_CLIENT_HINTS` honours the `Width` and `DPR` request headers for URLs that did not name their own.

- **Caching and provenance headers.** `IMGFORGE_TTL` sets `Cache-Control: max-age`;
  `IMGFORGE_CACHE_CONTROL_PASSTHROUGH` forwards the origin's policy instead. `IMGFORGE_USE_ETAG` emits an `ETag`
  over the response bytes and answers a matching `If-None-Match` with `304 Not Modified`.
  `IMGFORGE_LAST_MODIFIED_ENABLED` and `IMGFORGE_SET_CANONICAL_HEADER` pass the source's `Last-Modified` and a
  canonical `Link` through. All are off by default.

- **Source controls.** `IMGFORGE_BASE_URL` lets a URL carry only a path. `IMGFORGE_ALLOWED_SOURCES` restricts what
  may be fetched, with `https://*.example.com/` matching exactly one subdomain label. `IMGFORGE_USER_AGENT` and
  `IMGFORGE_MAX_REDIRECTS` control the fetch itself.

- **Deployment settings.** `IMGFORGE_PATH_PREFIX` mounts every route under a prefix.
  `IMGFORGE_HEALTH_CHECK_PATH` (default `/health`) answers alongside `/status`. `IMGFORGE_ALLOW_ORIGIN` sets a CORS
  origin. `IMGFORGE_SIGNATURE_SIZE` accepts signatures truncated to as few as 1 byte.
  `IMGFORGE_ENABLE_DEBUG_HEADERS` reports source and result sizes, and `IMGFORGE_DEVELOPMENT_ERRORS_MODE` returns
  the underlying error to the caller.

- **Processing defaults.** `IMGFORGE_AUTO_ROTATE`, `IMGFORGE_STRIP_METADATA`, `IMGFORGE_KEEP_COPYRIGHT`,
  `IMGFORGE_STRIP_COLOR_PROFILE`, `IMGFORGE_PRESERVE_HDR`, `IMGFORGE_ENFORCE_THUMBNAIL`,
  `IMGFORGE_RETURN_ATTACHMENT`, and `IMGFORGE_QUALITY` set the starting value for the matching option, which a URL
  still overrides.

- **Smart gravity.** `gravity:sm` hands the window choice to libvips' `smartcrop`, which scores the image for the
  region a viewer's eye would settle on. It is the answer when no fixed anchor is right for every image in a
  catalogue, and it applies to `crop` and to the implicit crop a `fill` resize performs. imgproxy gates this one
  behind its Pro tier.

- **New processing options.** `extend_aspect_ratio` (`exar`) pads to the requested shape without reaching its size.
  `keep_copyright` (`kcr`) carries the EXIF copyright across a metadata strip for JPEG, PNG and WebP output — an
  APP1 segment, an `eXIf` chunk, and an `EXIF` chunk respectively, synthesising the extended header WebP needs.
  TIFF, AVIF and HEIF can hold EXIF too but are not implemented; they strip as normal. `preserve_hdr` (`ph`) keeps a high bit-depth image high bit-depth and retains its gain map.
  `enforce_thumbnail` (`eth`) uses the source's embedded EXIF thumbnail. `skip_processing` (`skp`) returns the
  source untouched for the formats it lists.

- **Colour management.** The image is converted into a colourspace the pipeline's operations are written for before
  processing and back for the encoder, through the source's embedded ICC profile when it has one. A CMYK source no
  longer has its ink channels treated as if they were red, green, and blue by saturation, the background flatten,
  and trim's luminance fallback; a wide-gamut source is no longer read as though it were sRGB.

### Changed

- **`brightness` and `contrast` are applied.** They were parsed and validated but never used, on the basis that the
  libvips crate did not expose the `linear` operation they need. It does — the watermark code had been calling it
  all along. Requests that set them will now produce different images.

- **A crop with no gravity centres rather than pinning to the top-left**, which is what imgproxy has always done.
  `crop:300:200` on an existing URL will select a different region. Add `:nowe` to keep the old behaviour.

- **Watermark positioning follows imgproxy.** The implicit 5% margin is gone; use the new `x_offset` and `y_offset`
  arguments to reproduce it (`wm:0.5:soea:10:10`). Position also accepts `re`, which tiles the watermark.
  Watermark sizing still defaults to a quarter of the result's width, where imgproxy leaves it unscaled; the new
  `scale` argument takes imgproxy's meaning.

- **`strip_metadata` and `strip_color_profile` are independent.** Either one used to drop everything, because the
  generated save bindings can only express a single `keep` variant. Every encoder now goes through libvips' save
  suffix, which can express a combination, so stripping the colour profile keeps the EXIF and vice versa. JPEG,
  PNG, and TIFF moved to the suffix path along with the four formats that were already there.

- **An upstream failure is reported as one.** A 404 from the origin used to be handed to the decoder, surfacing as
  "failed to decode source image"; the response now names the upstream status.

- **The published image is built on Debian trixie** (libvips 8.16.1) rather than Ubuntu 24.04 (8.15.1). AVIF and
  HEIF output both work there — Debian packages libheif's codecs as plugins that libvips only Recommends, so the
  image names `libheif-plugin-aomenc` and `libheif-plugin-x265` explicitly — and `preserve_hdr`'s gain-map flag
  exists. The image now runs as a non-root user, carries a `HEALTHCHECK`, and builds its dependency graph as its
  own layer so an edit to imgforge's sources does not recompile every crate.

  A `.dockerignore` was added: `COPY . .` had been shipping the local `target/` directory into the build context.

- **An unsupported codec is reported at the format check** rather than at encode time. imgforge probes the
  codec-backed formats by encoding a real pixel once at startup, so a build with no AV1 or HEVC encoder returns
  `400 Unsupported output format` instead of a `500` mentioning `Unsupported compression`.

- **A result too large for its output container is scaled to fit** instead of failing in the encoder: WebP stops at
  16383px on a side, AVIF and HEIF at 16384.

- **`width` and `height` write into the resize target** rather than a separate field reconciled afterwards.
  `resizing_type:fill/width:300/height:200` used to fail with "resize requires at least one non-zero dimension",
  because the resizing type had already created the resize and the reconciliation only ran when there was none.

- **A resizing type with no dimensions leaves the image alone** instead of failing. `resize:fill` on its own names
  no target; imgproxy returns the image unresized, and now so does imgforge.

- **An unrecognised `webp_options` preset is refused** rather than accepted and then dropped on the way to the
  encoder, where a typo silently produced a different image.

- Booleans accept imgproxy's spellings (`1`, `t`, `T`, `true`, `TRUE`, `True`) rather than only `1` and `true`.
- `min_width` and `min_height` are accepted alongside imgforge's hyphenated `min-width` and `min-height`.
- `padding` accepts three values, following the CSS shorthand, rather than rejecting them.
- `resize` with an unrecognised type is rejected at parse time instead of failing during processing.

### Extended

- **`gravity` takes offsets and a focus point.** `gravity:no:0:20` nudges the window 20px down; a magnitude below 1
  is a fraction of the axis. `gravity:fp:0.5:0.25` centres the result on a point, which is the usual answer for
  portraits that a centre crop decapitates. `crop`, `extend`, and `extend_aspect_ratio` each take their own.
- **`crop` extents below 1 are a fraction of the source**, so `crop:0.5:0.5` takes the middle quarter of any image.
- **`fill-down` joins the resizing types.** Like `fill`, except a result smaller than the requested box is cropped
  to the box's aspect ratio rather than left at its own shape.
- **`zoom` takes independent axis factors**, so `zoom:2:1` doubles the width alone.
- **`extend` takes its own gravity**, independent of the request's.

### Fixed

- `extend_image` carried the same `u32` to `i32` cast that caused the padding overflow, and would have accepted a
  target past `i32::MAX` as a canvas smaller than the source. It now refuses one, as padding already did.
- `extend` with a target smaller on one axis and larger on the other used to fail the whole request; it now grows
  the axis that is short, which is what imgproxy does.

### Internal

- `options`, `transform`, and `service` were each over a thousand lines and are now directories whose submodules
  own one concern. `processing` gained `animation`, `colorspace`, `metadata`, `pipeline`, and `scale_on_load`;
  `negotiation` and `response` are new top-level modules.

## [0.17.0] - 2026-08-16

### Changed

- JPEG and WebP sources are decoded at a reduced scale when the request allows it, instead of being unpacked at
  full resolution and then downsampled. A 9000×7000 JPEG asked for a 450px thumbnail previously decoded 63 million
  pixels to keep 157 thousand. Measured through the real load path, peak memory per request drops from 114 MB to
  49 MB for JPEG, and from 737 MB to 50 MB for WebP — libwebp decodes the whole image at once rather than
  streaming, so a large WebP had been the heaviest request imgforge could receive.

  Since `IMGFORGE_WORKERS` is sized from measured peak memory, this converts fairly directly into concurrency at the
  same memory ceiling.

  The reduction never takes the source below what the request needs, is skipped for `raw`, and accounts for `dpr`,
  `zoom`, the minimum dimensions, and EXIF orientations that transpose the image. Cropped requests are included: the
  reduction is measured against the crop region rather than the whole source, and the region is rewritten to match
  what was decoded. Rounding can move a crop edge by up to a pixel, which the following resize absorbs.
  Source limits are still enforced against the original dimensions, before any of this.

  **Operational note:** output is visually identical but not bit-identical — DCT scaling and a lanczos downscale do
  not agree exactly. Mean absolute difference measured at 0.49 of 255 (max 3) on noise, the worst case for such a
  comparison. Existing cache entries keep serving their current bytes: the cache has no TTL, and entries go
  only when capacity evicts them, so a frequently requested URL can serve the old output indefinitely. Change the
  `cachebuster` token or clear the cache if you need the new bytes.

### Added

- `trim` / `t`, matching imgproxy: `trim:threshold[:color[:equal_hor[:equal_ver]]]` removes a uniform border before
  cropping and resizing. Omit the colour and imgforge reads the top-left pixel to work out what the border is, so a
  border of any colour is trimmed — libvips on its own assumes white and would leave a dark border in place.
  `equal_hor` and `equal_ver` cut the same amount from both sides, keeping an off-centre subject where it is.

  Trimming has to examine the pixels, so it **disables the reduced-scale decode**: the trimmed size cannot be known
  in advance, so there is no safe decode scale to choose and the source is read at full resolution. Worth reaching
  for deliberately rather than by default on large sources.

### Internal

- TIFF encoding has tests. It was the last save path with none, in the same position AVIF and GIF were in before
  their failure was found. Nothing was broken; the quality-dependent compression branch — LZW at 100, JPEG below —
  had simply never run.

## [0.16.0] - 2026-08-15

### Fixed

- `enlarge:false` — the default — no longer skips resizes that would only shrink the image. The
  check compared the requested box against the source and abandoned the whole resize when either
  side was larger, so a 1600×400 banner asked for `resize:fit:500:500` came back at full size
  instead of 500×125. Any square-ish box larger than the source's short side was affected.

  **Operational note:** URLs that were silently returning a full-size image now return the correctly
  scaled one — smaller responses and different bytes. Cache keys are URL-based, so existing entries
  keep serving the old output. There is no TTL — entries go only when capacity evicts
  them, so a frequently requested URL can serve the old size indefinitely. Change the `cachebuster` token or clear
  the cache to pick up the corrected sizes.

  Enlargement is now capped the way imgproxy caps it: the resizing type settles the scale, then every
  axis is divided by the largest scale when that exceeds 1. Two consequences worth knowing —
  `force` keeps the distortion you asked for rather than clamping each axis separately, and `fill`
  can return less than the requested box (500×100 from a 1000×100 source against a 500×200 box)
  where it previously failed with an error.

- Large `padding` values produced a wrong image instead of an error. The canvas was summed in `i32`,
  so a value above `i32::MAX` wrapped negative: `padding:0:4294967268:0:0` on a 64×64 source returned
  a **36×64 image with a 200**, quietly cropped rather than padded. Oversized padding is now refused
  with `400`. Debug builds previously panicked on the overflow.

- Resizing an image with transparency no longer bleeds the colour of invisible pixels into visible
  ones. libvips does not premultiply alpha inside `vips_resize` — its documentation says the caller
  must do it — and imgforge never did, so the kernel averaged fully transparent pixels into the
  edge. Downscaling white-on-transparent produced `241,241,241` at the boundary where it should
  produce `255,255,255`; in practice a dark halo around logos, icons, and cutouts, which also
  carried into `flatten` for JPEG output.

  Applies to every scaling path: `fit`, `fill`, `force`, `zoom`, the minimum-dimension pass,
  `pixelate`, and watermark scaling. Images without an alpha channel are untouched.

  **Operational note:** output bytes change for any image with transparency that gets scaled.

- Argument errors reach the client. Out-of-range values in `zoom`, `sharpen`, `extend`, and `padding`
  were flattened into `Error processing image`; the response now names the actual problem.

- The `crop` and `min-width`/`min-height` documentation described behaviour the code never had.
  `crop` is `crop:width:height[:gravity]` — there are no x/y arguments, and gravity is what positions
  the window; the previously documented `crop:x:y:width:height` returns `400`. The minimums upscale
  **regardless of `enlarge`**, so `enlarge:false` is not a guard against them.

### Internal

- Test images are no longer handed to libvips as freed memory. The suite's idiom passed a reference
  to a temporary, and libvips decodes lazily while holding a pointer into that buffer — it worked
  only while the freed allocation happened not to be reused. Forcing 26 MB of allocator churn
  between load and use made it fail outright. 82 call sites now go through a helper that owns the
  buffer.

### Added

- `max_result_dimension` / `mrd`, with `IMGFORGE_MAX_RESULT_DIMENSION`, capping the width and height
  of the processed image. Nothing previously bounded output size: a request could ask for any
  dimensions it liked and imgforge would attempt them. Per-request overrides require
  `IMGFORGE_ALLOW_SECURITY_OPTIONS`, matching `max_src_resolution` and `max_src_file_size`.

  Cached responses are namespaced by the effective limit, so entries stored under a higher ceiling
  are not served after you lower it. Enabling the option does not invalidate a cache that was not
  using it.

## [0.15.0] - 2026-08-15

### Fixed

- AVIF, HEIF, and GIF output failed on the libvips shipped with the published Docker image, with
  errors like `heifsave_buffer: no property named 'tune'`. The libvips crate's generated save
  bindings name encoder properties that only exist in libvips 8.16 and later — `tune` on heifsave,
  `keep-duplicate-frames` on gifsave, `exact` on webpsave — and an older libvips rejects the entire
  call, so those formats did not encode at all. The image is built `FROM ubuntu:24.04`, which ships
  libvips 8.15.1.

  These formats now encode through the libvips save suffix, as WebP already did since 0.14.0. The
  option-string parser sets only the options named, so it stays correct across libvips versions.
  JPEG, PNG, and TIFF are unaffected; every property their bindings name predates 8.15.

  HEIF specifically still fails on a libvips built without an HEVC encoder, but now reports the
  real reason (`Unsupported compression`) instead of a misleading property error. AVIF uses a
  different codec and is unaffected.

### Changed

- GIF output now honours `strip_metadata`. The previous encoder call never passed the metadata
  setting, so GIF retained metadata regardless of the option — unlike every other format.
- Dependency upgrades: `hmac` 0.12 → 0.13, `sha2` 0.10 → 0.11, `base64` 0.22 → 0.23, `tower-http`
  0.6 → 0.7, plus semver-compatible updates across the tree. URL signatures are byte-identical:
  signatures issued by earlier releases keep validating, and a pinned signature vector in the test
  suite now guards that across future dependency bumps.

  `libvips` stays on 1.7.6 deliberately. Version 2.x passes the same encoder property names, so it
  does not address the issue above.

## [0.14.0] - 2026-08-12

### Fixed

- WebP saves now apply the requested `quality` instead of encoding every image at libvips' default.
  `webp_options` (`lossless`, `smart_subsample`, `preset`) and metadata stripping are applied as
  well. The options travel through the libvips save suffix, so the workaround for the crash-prone
  generated `webpsave` bindings stays in place ([#49](https://github.com/ImgForger/imgforge/pull/49),
  closes [#46](https://github.com/ImgForger/imgforge/issues/46)).

  **Operational note:** existing WebP URLs change size. Quality was previously fixed at libvips'
  default of roughly Q75 regardless of the requested value, so low-quality requests now produce
  smaller responses and `quality:90` and above produce noticeably larger ones. Cache keys are
  URL-based and the cache has no TTL, so cached entries keep their old bytes until capacity
  evicts them. Change the `cachebuster` token to force the new output.

  Preset names outside libvips' own set (`default`, `picture`, `photo`, `drawing`, `icon`, `text`)
  are accepted in URLs but ignored by the encoder rather than failing the request.

## [0.13.0] - 2026-08-09

### Added

- Concurrency and queueing metrics at `/metrics`, all labelled by `operation` except where noted:
  `image_operation_semaphore_wait_duration_seconds`,
  `image_operation_blocking_queue_duration_seconds`,
  `image_operation_execution_duration_seconds`, `image_operations_active`,
  `image_operations_waiting`, and the unlabelled `image_operation_concurrency_limit` gauge.
- Grafana dashboard panels for queue latency and worker saturation, plus alerting patterns for
  worker, blocking-pool, and concurrency saturation in
  [Prometheus Monitoring](doc/11_prometheus_monitoring.md).
- Concurrency tuning guidance in [Performance Tips](doc/9_performance.md).

### Changed

- Image decoding, transformation, and encoding run on Tokio's blocking pool behind a semaphore
  bounded by `IMGFORGE_WORKERS`, so image work no longer occupies the async runtime and excess
  requests queue instead.
- A malformed `IMGFORGE_WORKERS` value stops startup rather than being silently ignored. `0` still
  selects `num_cpus * 2`.

## [0.12.0] - 2026-08-08

### Changed

- **Breaking:** format-less image URLs preserve the source image's format instead of defaulting to
  JPEG, matching imgproxy. This changes response bytes, `Content-Type`, file size, and transparency
  behavior. Set `IMGFORGE_DEFAULT_FORMAT=jpeg` to restore the previous behavior (closes
  [#45](https://github.com/ImgForger/imgforge/issues/45)).
- Cached format-less responses are namespaced by the configured default format, so responses encoded
  under an earlier setting are never reused after the setting changes.
- Fetch, processing, option and preset parsing, cache configuration, and server startup return typed
  errors that preserve their source, with the mapping to HTTP responses centralized in the service
  layer.

### Added

- `IMGFORGE_MAX_SRC_FILE_SIZE` and `IMGFORGE_MAX_SRC_RESOLUTION` are validated at startup: values
  must be positive, finite, and within their supported ranges, and imgforge refuses to start
  otherwise. Leaving a variable unset remains the only way to disable that limit.
- Unknown `resizing_type` values are rejected with an explicit error naming the supported values
  (`fill`, `fit`, `force`, `auto`).

## [0.11.0] - 2026-08-05

### Fixed

- File-based watermarks (`IMGFORGE_WATERMARK_PATH`) failed every request with `Composite2Error`
  while the same image worked through `watermark_url`. The prepared-watermark cache lost the image's
  colourspace on the round trip; it is now restored on the way out
  ([#48](https://github.com/ImgForger/imgforge/pull/48), closes
  [#47](https://github.com/ImgForger/imgforge/issues/47)).

### Added

- Tests covering prepared-watermark cache faithfulness and the no-alpha path.

### Changed

- Simplified the internal caching and image transformation APIs.
- Upgraded dependencies and CI workflow versions.

## [0.10.0] - 2026-05-26

### Added

- Broad imgproxy-compatible processing options:
  - Geometry: `size`/`s`, `min-width`/`mw`, `min-height`/`mh`, `crop`/`c`, `gravity`/`g`, `flip`/`fl`.
  - Appearance: `adjust`/`a`, `brightness`/`br`, `contrast`/`co`, `saturation`/`sa`, `pixelate`/`pix`,
    `background_alpha`/`bga`.
  - Output: `format`/`f`/`ext`, `format_quality`/`fq`, `max_bytes`/`mb`, `strip_metadata`/`sm`,
    `strip_color_profile`/`scp`, `jpeg_options`/`jpgo`, `png_options`/`pngo`, `webp_options`/`webpo`,
    `avif_options`/`avifo`.
  - Delivery: `cachebuster`/`cb`, `expires`/`exp`, `filename`/`fn`, `return_attachment`/`att`,
    `skip_processing`/`skp`.
  - Multi-page: `page`/`pg`, `pages`/`pgs`, `disable_animation`/`da`.
  - Per-request source limits: `max_src_resolution`/`msr`, `max_src_file_size`/`msfs`.

  Some options are parsed for URL compatibility but not applied by the encoder; see
  [Processing Options](doc/5_processing_options.md) for the current state of each.

### Changed

- WebP saves use the safe libvips save path, documented alongside the crash caveat in the generated
  `webpsave` bindings. (Superseded by the WebP encoder-option fix in 0.14.0, above.)

[Unreleased]: https://github.com/ImgForger/imgforge/compare/v0.18.0...HEAD
[0.18.0]: https://github.com/ImgForger/imgforge/compare/v0.17.0...v0.18.0
[0.17.0]: https://github.com/ImgForger/imgforge/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/ImgForger/imgforge/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/ImgForger/imgforge/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/ImgForger/imgforge/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/ImgForger/imgforge/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/ImgForger/imgforge/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/ImgForger/imgforge/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/ImgForger/imgforge/compare/v0.9.7...v0.10.0
