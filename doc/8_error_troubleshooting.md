# 8. Error Handling & Troubleshooting

imgforge strives to return clear error messages and appropriate HTTP status codes. This guide collects common responses, explains their causes, and offers debugging steps.

## Response codes

| Status                                      | When it occurs                                                                                                         | Notes                                                                          |
|---------------------------------------------|------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------|
| `200 OK`                                    | Successful processing or cache hit.                                                                                    | Response body contains image bytes; `Content-Type` reflects the output format. |
| `400 Bad Request`                           | Invalid path structure, malformed processing option, disallowed MIME type, oversize file, failed watermark fetch, etc. | Body contains a short explanatory string (e.g., `"Invalid URL format"`).       |
| `401 Unauthorized`                          | Missing or invalid `Authorization: Bearer` token when `IMGFORGE_SECRET` is set.                                        | Include the correct secret header.                                             |
| `403 Forbidden`                             | Signature mismatch, unsigned URLs when disabled, or bearer token mismatch.                                             | Recompute the signature or re-enable unsigned mode for development.            |
| `404 Not Found`                             | Only surfaced when a specific endpoint is unknown (e.g., `/metrics` disabled listener).                                | Ensure you are hitting the correct path.                                       |
| `408 Request Timeout / 504 Gateway Timeout` | Source fetch exceeded `IMGFORGE_DOWNLOAD_TIMEOUT` or the request exceeded `IMGFORGE_TIMEOUT`.                          | Increase timeouts or optimize upstream latency.                                |
| `429 Too Many Requests`                     | Global rate limiter rejected the request.                                                                              | Increase `IMGFORGE_RATE_LIMIT_PER_MINUTE` or add upstream throttling.          |
| `500 Internal Server Error`                 | Unexpected libvips errors, I/O issues, or cache initialization failures.                                               | Check logs for stack traces and error context.                                 |

## Troubleshooting workflow

1. **Check logs** – `IMGFORGE_LOG_LEVEL=debug` reveals detailed traces. Look for the generated `id` in `TraceLayer` spans to correlate multiple log lines.
2. **Inspect `/metrics`** – Counters such as `status_codes_total` and `source_images_fetched_total` help spot systemic issues (e.g., many fetch errors).
3. **Validate signatures** – Use helper scripts from [URL Structure](4_url_structure.md) to ensure the signature and encoded URL match exactly.
4. **Replicate without signature** – Temporarily set `IMGFORGE_ALLOW_UNSIGNED=true` and replace the signature with `unsafe` to isolate signing issues.
5. **Reproduce locally** – Run the same URL against a local instance with `IMGFORGE_LOG_LEVEL=debug`. Compare log output with production behavior.
6. **Confirm dependencies** – Ensure libvips is installed and accessible. Missing shared libraries can cause runtime panics or `500` responses.

## Common error scenarios

### Signature mismatch (`403`)

- Verify that both the client and server use identical salts and keys.
- Confirm the path used for signing begins with `/` and matches the request exactly.
- Ensure Base64 encoding is URL-safe and unpadded.

### Fetch failures (`400`, `504`)

- The upstream host might be unreachable or rejecting requests. Use `curl` to test from the same network.
- Increase `IMGFORGE_DOWNLOAD_TIMEOUT` for slow sources.
- Consider whitelisting IP ranges or setting up an HTTP proxy if egress is restricted.

### Source blocked by safeguards (`400`)

- Error messages such as `"Source image file size is too large"` or `"Source image MIME type is not allowed"` indicate guardrails triggered.
- Relax global limits in [Configuration](3_configuration.md) or allow per-request overrides with `IMGFORGE_ALLOW_SECURITY_OPTIONS=true`.

### Watermark issues (`400`)

- Ensure the watermark URL is reachable and returns an image.
- When using `IMGFORGE_WATERMARK_PATH`, confirm the file exists and is readable by the imgforge process.

### Cache initialization errors (`500` on startup)

- Check directory permissions for `IMGFORGE_CACHE_DISK_PATH`.
- Verify there is enough disk space. Foyer will fail to start if capacity cannot be reserved.

### 429 responses

- Rate limiting is global, not per-client. Increase the limit or add request queueing upstream.
- Monitor `status_codes_total{status="429"}` and adjust thresholds before public incidents.

## Support checklist

When filing a bug report or seeking help, provide:

- The exact request URL (with sensitive signatures removed or redacted).
- Response status and body.
- Relevant log excerpts (include request IDs).
- Output from `/metrics` showing related counters or histograms.
- Environment settings related to the issue (timeouts, cache configuration, security flags).

Use [Contributing](11_contributing.md) for additional guidelines on opening issues.
