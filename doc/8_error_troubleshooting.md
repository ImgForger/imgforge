# 8. Error Handling & Troubleshooting

What each status code means, and how to work back to the cause.

## Response codes

| Status                      | When it occurs                                                                                                        | Notes                                                                          |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `200 OK`                    | Successful processing or cache hit.                                                                                   | Body is the image; `Content-Type` reflects the output format.                  |
| `400 Bad Request`           | Invalid path, malformed option, disallowed MIME type, oversize source, failed watermark fetch, or a source download that timed out. | Body carries a short reason such as `Invalid URL format`.                      |
| `403 Forbidden`             | Signature mismatch, an unsigned URL while `IMGFORGE_ALLOW_UNSIGNED` is off, or a missing/invalid bearer token when `IMGFORGE_SECRET` is set. | imgforge does not use `401` — a token problem is a `403`.                      |
| `404 Not Found`             | Unknown endpoint, or a URL whose `expires` timestamp has passed.                                                       | Check the path and any `expires` directive.                                    |
| `429 Too Many Requests`     | Global rate limiter depleted.                                                                                         | Raise `IMGFORGE_RATE_LIMIT_PER_MINUTE` or throttle upstream.                   |
| `408 Request Timeout`       | The request exceeded `IMGFORGE_TIMEOUT`, imgforge's own whole-request budget.                                         | A slow *source* returns `400` instead. imgforge never emits `504` itself.      |
| `504 Gateway Timeout`       | An upstream proxy gave up waiting for imgforge.                                                                       | Comes from the proxy, not imgforge. Align its timeout with `IMGFORGE_TIMEOUT`. |
| `500 Internal Server Error` | Unexpected libvips or I/O failure.                                                                                    | Check logs for the error context.                                              |

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

### Fetch failures (`400`)

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

See [Contributing](../CONTRIBUTING.md) for guidelines on opening issues.
