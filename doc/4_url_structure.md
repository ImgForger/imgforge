# 4. URL Structure & Signing

imgforge mirrors the URL layout of imgproxy: every transformation is encoded inside the request path, and requests are authenticated via an HMAC signature. Source URLs can be provided either in plain text (with the `plain/` prefix) or Base64-encoded format. This document details the anatomy of those URLs, explains how to sign them, and highlights development shortcuts.

## Path anatomy

```
http(s)://<host>/<signature>/<processing_options>/plain/<percent-encoded-source>@<extension>
http(s)://<host>/<signature>/<processing_options>/<base64url-source>.<extension>
```

| Segment                | Description                                                                                                                                 |
|------------------------|---------------------------------------------------------------------------------------------------------------------------------------------|
| `<signature>`          | Base64 URL-safe, unpadded HMAC-SHA256 digest generated from the path. Use `unsafe` when unsigned URLs are permitted.                        |
| `<processing_options>` | Slash-separated list of directives (e.g., `resize:fill:800:600/quality:85`). See [Processing Options](5_processing_options.md).             |
| `plain/...`            | Indicates the source URL is provided in plain text (percent-encoded if needed) and may include `@<extension>` to declare the output format. |
| `<base64url-source>`   | The source URL encoded using URL-safe Base64 without padding (`=`). The output extension, if specified, is appended after a dot.            |

## Choosing between `plain` and Base64

Source URLs can be provided in two formats:

- **Plain format** (`plain/` prefix): Use when the source URL contains only URL-safe characters and you want to specify the output format explicitly with `@<extension>`
- **Base64 format**: Use when the source URL contains special characters, query parameters, or when you want to avoid potential encoding conflicts

### Examples

**Plain URL format with format conversion:**
```
/<sig>/resize:fit:1024:0/plain/https://example.com/cats/siamese.jpg@webp
```

**Base64-encoded URL format:**
```
/<sig>/resize:fill:800:600/aHR0cHM6Ly9leGFtcGxlLmNvbS9jYXRzL3NpYW1lc2UuanBn.jpg
```
*Note: `aHR0cHM6Ly9leGFtcGxlLmNvbS9jYXRzL3NpYW1lc2UuanBn` is the Base64URL encoding of `https://example.com/cats/siamese.jpg`*

**Plain URL with query parameters (requires Base64):**
```
/<sig>/resize:fit:800:0/aHR0cHM6Ly9leGFtcGxlLmNvbS9pbWFnZS5qcGc_dj0xMjM.webp
```
*Note: `aHR0cHM6Ly9leGFtcGxlLmNvbS9pbWFnZS5qcGc_dj0xMjM` is the Base64URL encoding of `https://example.com/image.jpg?v=123`*

Generate Base64 URL-safe strings without padding (replace `+` with `-`, `/` with `_`, and remove trailing `=`).

## Signing a URL

### Why signatures matter

Signed URLs prevent tampering. Anyone with write access to a CDN, cache, or browser can attempt to modify processing directives to produce oversized images or trigger expensive transformations. The HMAC signature ensures only parties who know `IMGFORGE_KEY` and `IMGFORGE_SALT` can generate valid requests.

### How signing works

1. Convert `IMGFORGE_KEY` and `IMGFORGE_SALT` from hex to raw bytes.
2. Build the path portion beginning with the slash before the processing options (for example `/resize:fill:800:600/plain/...`).
3. Concatenate the salt bytes with the path bytes.
4. Compute an HMAC-SHA256 digest using the key from step 1.
5. Encode the digest using Base64 URL-safe without padding.
6. Prefix the signature to the path and send the request.

The same steps apply for both `plain` and Base64 source segments. Rotate keys periodically and store them securely; leaking either value lets attackers craft arbitrary URLs.

### Language examples

**Rust**

```rust
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

let key = hex::decode(std::env::var("IMGFORGE_KEY").unwrap()).unwrap();
let salt = hex::decode(std::env::var("IMGFORGE_SALT").unwrap()).unwrap();
let path = "/resize:fill:800:600/plain/https://example.com/cat.jpg@webp";

let mut mac = HmacSha256::new_from_slice(&key).unwrap();
mac.update(&salt);
mac.update(path.as_bytes());
let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
println!("{}{}", signature, path);
```

**Python**

```python
import base64, hmac, hashlib, os

key = bytes.fromhex(os.environ["IMGFORGE_KEY"])
salt = bytes.fromhex(os.environ["IMGFORGE_SALT"])
path = "/resize:fill:800:600/plain/https://example.com/cat.jpg@webp"

digest = hmac.new(key, salt + path.encode(), hashlib.sha256).digest()
signature = base64.urlsafe_b64encode(digest).rstrip(b"=").decode()
print(f"{signature}{path}")
```

### Validating signatures

When building automated tests, compute the expected signature using the same recipe and assert that imgforge accepts the resulting URL. Many teams wrap the logic in a shared helper so application servers, static-site generators, and edge functions share the same implementation.

## Unsigned URLs (`unsafe`)

When `IMGFORGE_ALLOW_UNSIGNED=true`, the signature segment can be replaced with `unsafe`:

```
http://localhost:3000/unsafe/resize:fit:600:0/plain/https://example.com/dog.jpg
```

Use this mode for development only; it bypasses HMAC validation entirely.

## Common signing mistakes

1. **Incorrect path prefix**: Include the leading slash (`/resize:...`) when computing the digest.
2. **Hex decoding**: `IMGFORGE_KEY` and `IMGFORGE_SALT` must decode to raw bytes. Do not reuse the hex string directly.
3. **Padding**: Remove trailing `=` when encoding the signature using Base64 URL-safe.
4. **Salt omission**: Always concatenate the salt bytes before the path.
5. **URL normalization**: Ensure the source URL is percent-encoded identically in both the signature computation and the request.

## Next steps

- Explore available transformations in [Processing Options](5_processing_options.md).
- Review the request lifecycle in [Request Lifecycle](6_request_lifecycle.md).
- If your application generates many URLs, encapsulate signing logic into a shared helper library to avoid drift.
