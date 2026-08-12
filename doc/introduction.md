# Introduction

imgforge is an image proxy and transformation server written in Rust on libvips and Axum. It sits between your applications, a CDN, and wherever the original images live: it resizes, crops, converts formats, applies effects, and caches the result. The URL format is largely compatible with [imgproxy](https://github.com/imgproxy/imgproxy), so migrations are mostly mechanical.

Use it to serve images at the size and format each client actually needs, without pre-generating variants or doing the work on your application servers.

## How it fits together

```
  end users ──▶ CDN ──miss──▶ imgforge ──fetch──▶ origin storage
                                  │                (S3, HTTP, ...)
                                  ├── cache: memory / disk / hybrid
                                  └── /metrics, /status
```

Per request, imgforge verifies the URL signature, checks its cache, downloads the source with size and MIME validation, runs the libvips pipeline, and caches what it produced. [Request Lifecycle](6_request_lifecycle.md) walks through each stage and the failures it can return.

## Security model

- HMAC-signed URLs by default. Unsigned mode exists for local development and should stay off elsewhere.
- Optional bearer token on the image and info endpoints.
- Fetch guards on MIME type, source file size, and source resolution, applied before any decoding.
- Optional global rate limiting.

## What imgforge is not

- **A CDN.** Put one in front of it for TLS, edge caching, and global distribution.
- **A DAM or file server.** Your storage stays where it is; imgforge only transforms and delivers.

## Special thanks

imgforge is inspired by [imgproxy](https://github.com/imgproxy/imgproxy). So special thanks to the authors for their work!

Many thanks to the following open source projects that make imgforge possible:

- [libvips](https://github.com/libvips/libvips) for its powerful image processing library.
- [Foyer](https://github.com/foyer-rs/foyer) for its fast, memory-efficient hybrid cache.
- [Axum](https://github.com/tokio-rs/axum) for its fast, async web framework.
- [Tokio](https://github.com/tokio-rs/tokio) for its async runtime.
- [Serde](https://github.com/serde-rs/serde) for its powerful data serialization framework.
- [Hyper](https://github.com/hyperium/hyper) for its HTTP client.
- [Tokio-stream](https://github.com/tokio-rs/tokio/tree/master/tokio-stream) for its streaming abstractions.
- [Tokio-util](https://github.com/tokio-rs/tokio/tree/master/tokio-util) for its utilities.
