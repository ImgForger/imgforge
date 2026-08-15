# Changelog

All notable changes to imgforge are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). While imgforge is
pre-1.0, minor versions may carry breaking changes; those are called out explicitly.

Entries start at 0.10.0. For earlier history, see the
[GitHub releases](https://github.com/ImgForger/imgforge/releases) and the git log.

## [Unreleased]

### Fixed

- `enlarge:false` — the default — no longer skips resizes that would only shrink the image. The
  check compared the requested box against the source and abandoned the whole resize when either
  side was larger, so a 1600×400 banner asked for `resize:fit:500:500` came back at full size
  instead of 500×125. Any square-ish box larger than the source's short side was affected.

  **Operational note:** URLs that were silently returning a full-size image now return the correctly
  scaled one — smaller responses and different bytes. Cache keys are URL-based, so existing entries
  keep serving the old output until they expire or are evicted; clear the cache if you need the
  corrected sizes immediately.

  Enlargement is now capped the way imgproxy caps it: the resizing type settles the scale, then every
  axis is divided by the largest scale when that exceeds 1. Two consequences worth knowing —
  `force` keeps the distortion you asked for rather than clamping each axis separately, and `fill`
  can return less than the requested box (500×100 from a 1000×100 source against a 500×200 box)
  where it previously failed with an error.

- Large `padding` values produced a wrong image instead of an error. The canvas was summed in `i32`,
  so a value above `i32::MAX` wrapped negative: `padding:0:4294967268:0:0` on a 64×64 source returned
  a **36×64 image with a 200**, quietly cropped rather than padded. Oversized padding is now refused
  with `400`. Debug builds previously panicked on the overflow.

- Argument errors reach the client. Out-of-range values in `zoom`, `sharpen`, `extend`, and `padding`
  were flattened into `Error processing image`; the response now names the actual problem.

- The `crop` and `min-width`/`min-height` documentation described behaviour the code never had.
  `crop` is `crop:width:height[:gravity]` — there are no x/y arguments, and gravity is what positions
  the window; the previously documented `crop:x:y:width:height` returns `400`. The minimums upscale
  **regardless of `enlarge`**, so `enlarge:false` is not a guard against them.

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
  URL-based, so cached entries refresh as they expire or are evicted.

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

[Unreleased]: https://github.com/ImgForger/imgforge/compare/v0.15.0...HEAD
[0.15.0]: https://github.com/ImgForger/imgforge/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/ImgForger/imgforge/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/ImgForger/imgforge/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/ImgForger/imgforge/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/ImgForger/imgforge/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/ImgForger/imgforge/compare/v0.9.7...v0.10.0
