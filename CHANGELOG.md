# Changelog

All notable changes to imgforge are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). While imgforge is
pre-1.0, minor versions may carry breaking changes; those are called out explicitly.

Entries start at 0.10.0. For earlier history, see the
[GitHub releases](https://github.com/ImgForger/imgforge/releases) and the git log.

## [Unreleased]

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
  `webpsave` bindings. (Superseded by the WebP encoder-option fix in Unreleased, above.)

[Unreleased]: https://github.com/ImgForger/imgforge/compare/v0.13.0...HEAD
[0.13.0]: https://github.com/ImgForger/imgforge/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/ImgForger/imgforge/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/ImgForger/imgforge/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/ImgForger/imgforge/compare/v0.9.7...v0.10.0
