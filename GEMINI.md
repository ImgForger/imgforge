# Building imgproxy in Rust: Implementation Guide for Claude Code

This document provides a detailed specification for implementing a Rust-based clone of [imgproxy](https://github.com/imgproxy/imgproxy), a fast and secure standalone server for resizing, processing, and converting remote images on the fly. The goal is to create a lightweight HTTP server that processes images based on URL parameters, emphasizing security, speed, and simplicity.

The implementation should use Rust's ecosystem for HTTP handling (e.g., `axum` or `warp`), image fetching (`reqwest`), and processing (e.g., `image` crate for basic ops or `libvips` via `vips-rs` for advanced performance). Focus on core features first, with extensibility for Pro-like features (marked below).

## Project Overview
- **Purpose**: Serve processed images via HTTP GET requests. Parse URLs to extract source image URL, processing options, and signature. Fetch the source, apply transformations, and return the result.
- **Principles**:
  - **Security**: Support signed URLs to prevent abuse; validate dimensions to avoid "image bombs."
  - **Speed**: Use efficient libraries like libvips; minimize memory usage.
  - **Simplicity**: No built-in caching or HTTPS—recommend external CDNs/proxies.
- **Deployment**: Configurable via environment variables (e.g., keys, max dimensions). Run as a standalone binary.
- **Supported Formats** (Input/Output):
  - Input: JPEG, PNG, WebP, AVIF, JPEG XL, GIF (animated), SVG, ICO, HEIC, BMP, TIFF.
  - Output: Same as input, plus format conversion (e.g., to WebP for browsers). Default to source format or JPEG if unsupported. Support `best` for auto-selection (Pro).

## URL Structure
All requests follow this format:
```
http://your-server/{signature}/{processing_options}/plain/{encoded_source_url}@{extension}
```
or
```
http://your-server/{signature}/{processing_options}/{base64_encoded_source_url}.{extension}
```
or (Pro)
```
http://your-server/{signature}/{processing_options}/enc/{aes_encrypted_source_url}.{extension}
```

- **Signature** (`{signature}`): Hex string (e.g., `AfrOrF3gWeDA6VOlDG4TzxMv39O7MXnF4CXpKUwGqRM`). Required for security. Use HMAC-SHA256 with key + salt + path. If disabled, accept `unsafe` or `_`.
- **Processing Options** (`{processing_options}`): Slash-separated list of `option_name:arg1:arg2:...` (default separator `:`, configurable). Order doesn't matter.
- **Source URL**:
  - `plain/`: Percent-encode the full URL (e.g., `/plain/http%3A//example.com/image.jpg`).
  - Base64: URL-safe Base64 (split with `/` if long, e.g., `/aHR0cDovL2V4YW1w/bGUuY29tL2ltYWdl/cy9jdXJpb3NpdHku/anBn`).
  - `enc/`: AES-CBC encrypted (Pro; use 16/24/32-byte keys).
- **Extension** (`@{extension}` or `.{extension}`): Output format (e.g., `@png`, `.webp`). Omit for auto (source or JPEG fallback).

**Example URL**:
```
/AfrOrF3g.../resize:fill:300:400:0/gravity:sm/plain/http%3A//example.com/image.jpg@webp
```
This resizes to 300x400 (fill mode, no enlarge), smart gravity, outputs WebP.

**Implementation Notes**:
- Parse URL path into components.
- Decode source URL (percent-decode for plain, base64-decode for encoded, AES decrypt for enc).
- Validate source URL (e.g., allow only HTTP/HTTPS, configurable domains).
- Fetch source with `reqwest`; check MIME type and dimensions against config (e.g., max 10MP, max file 10MB).

## Security Features
- **Signed vs Unsigned URLs**:
  - **Signed**: Compute signature as `hex(hmac_sha256(key, salt + url_path))`. Validate on receipt. Config: `KEY` (16-byte hex), `SALT` (hex). Multiple key/salt pairs supported.
  - **Unsigned**: Disabled by default; enable via `ALLOW_UNSIGNED=true`. Use dummy signature (e.g., `unsafe`).
  - Reject invalid signatures with 403.
- **Authorization Header**: Support `Authorization: Bearer <token>` for CDN proxying (validate against config).
- **Image Bomb Protection**: On fetch, validate:
  - MIME type in allowed list.
  - Dimensions ≤ `MAX_SRC_RESOLUTION` (e.g., 50MP).
  - File size ≤ `MAX_SRC_FILE_SIZE` (e.g., 10MB).
  - For animated: `MAX_ANIMATION_FRAMES`, `MAX_ANIMATION_FRAME_RESOLUTION`.
- **Security Options** (allow via `ALLOW_SECURITY_OPTIONS=true`): Per-request limits like `max_src_resolution:5000000`.
- **Config**: Env vars like `IMGPROXY_KEY`, `IMGPROXY_SALT`, `IMGPROXY_MAX_SRC_RESOLUTION`.

**Implementation**: Use `hmac` and `sha2` crates for signing. `ring` or `aes` for encryption (Pro).

## Core Processing Options
Process images using a pipeline: load → apply transforms → encode → serve. Support chaining options.

| Option | Short | Args | Description | Type/Values | Default/Example |
|--------|-------|------|-------------|-------------|-----------------|
| **resize** | rs | resizing_type:width:height:enlarge:extend | Meta-option for sizing. Width/height=0 auto-scales. | Type: fill/fit/force/auto; W/H: uint; Enlarge/Extend: 0/1/true/false | resize:fill:300:400:0:0<br>Fill: Crop to exact size; Fit: Scale to fit. |
| **size** | sz | width:height:enlarge:extend | Alias for resize without type (uses `fit`). | Same as resize. | size:300:0:1:0 (scale to 300px wide). |
| **resizing_type** | rt | type | Resize mode (used with resize/size). | fill (crop fill), fit (contain), force (stretch), auto (fit if smaller, enlarge if needed). | fit |
| **width** | w | pixels | Set width (auto height). | uint | w:300 |
| **height** | h | pixels | Set height (auto width). | uint | h:400 |
| **enlarge** | - | 0/1/true/false | Allow upscaling. | bool | 0 (no) |
| **extend** | - | 0/1/true/false | Pad with background if needed. | bool | 0 (no) |
| **gravity** | g | mode | Crop/resize anchor point. | no (center), sm (smart: edges/saturation/skin), faces (face detection, Pro), north/south/east/west/center. | center<br>g:sm |
| **crop** | - | x:y:width:height | Exact crop from top-left. | uints | crop:100:50:200:300 |
| **padding** | - | left:right:top:bottom | Add padding (scales with DPR). | uints (or single for all) | padding:10:10:10:10 |
| **background** | bg | color | Background for transparent/PNG or extend. | hex (e.g., ffffff) or rgba. | bg:ffff00 |
| **quality** | q | level | Compression quality. | 1-100 | 85<br>q:90 |
| **format** | - | fmt | Output format (overrides extension). | jpg/png/webp/avif/etc. | Source |
| **dpr** | - | factor | Device pixel ratio (scale up). | float (1.0-5.0) | 1.0<br>dpr:2 |
| **auto_rotate** | ar | 0/1/true/false | Rotate based on EXIF. | bool | true (config-dependent) |
| **orientation** | - | angle | Force rotation. | 0/90/180/270 | 0 |
| **blur** | - | sigma | Gaussian blur. | float | blur:5 |
| **raw** | - | - | Bypass processing limits (e.g., workers). | flag | - |
| **cache_buster** | - | value | Bypass cache (e.g., timestamp). | string | - |

**Pro Features (Optional Extensions)**:
- **Watermark**: `wm:base64_encoded_watermark_url:position:opacity:dpr`. Dynamic via env.
- **Smart Crop**: Enhanced gravity with AI.
- **Video Thumbnails**: `video_thumbnail:time:step:keyframe`. Fallback image.
- **Color Profile**: `color_profile:name`.
- **Max Security Per-Request**: `max_src_resolution:val`, etc.

**Implementation Notes**:
- Use `image` crate for basic (resize, crop, format); `vips` for advanced (smart crop, WebP/AVIF).
- Pipeline: Load image → Apply gravity/crop → Resize → Background/pad → Encode.
- Handle errors: 400 for invalid options, 404 for fetch fail, 500 for processing fail.
- Headers: Set `Content-Type`, `Cache-Control` (e.g., max-age from config).

## Additional Features
- **Info Endpoint** (`/info`): Return JSON with source metadata (width, height, format) using similar URL format + `info_options`.
- **Presets**: Named sets of options (e.g., `preset:thumbnail`). Config via `PRESETS_JSON`.
- **Configuration**: All via env (e.g., `IMGPROXY_BIND=:8080`, `IMGPROXY_MAX_ANIMATION_FRAMES=100`).
- **Logging/Metrics**: Basic logging; optional Prometheus.
- **Testing**: Unit tests for URL parsing, signing, basic processing. Integration with sample images.

## Development Guidelines
- **Crates**: `axum` (server), `reqwest` (fetch), `image` or `vips` (process), `hmac`, `base64`, `percent-encoding`.
- **Error Handling**: Graceful, with HTTP status codes.
- **Performance**: Benchmark against original; aim for low memory (<50MB/image).
- **Phases**:
  1. Basic server + URL parsing + fetch/serve raw.
  2. Signing/unsigned support.
  3. Core processing (resize, crop, format).
  4. Advanced options + security checks.
  5. Pro extensions.

Implement iteratively, starting with unsigned URLs and basic resize. Reference imgproxy docs for edge cases. Output binary: `cargo run --release`.
