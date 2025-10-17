# 4. URL Structure & Signing

imgforge mirrors the URL layout of imgproxy: every transformation is encoded inside the request path, and requests are authenticated via an HMAC signature. This document details the anatomy of those URLs, explains how to sign them, and highlights development shortcuts.

## Path anatomy

```
http(s)://<host>/<signature>/<processing_options>/plain/<percent-encoded-source>@<extension>
http(s)://<host>/<signature>/<processing_options>/<base64url-source>.<extension>
```

| Segment                | Description                                                                                                                          |
|------------------------|--------------------------------------------------------------------------------------------------------------------------------------|
| `<signature>`          | Base64 URL-safe, unpadded HMAC-SHA256 digest generated from the path. Use `unsafe` when unsigned URLs are permitted.                 |
| `<processing_options>` | Slash-separated list of directives (e.g., `resize:fill:800:600/quality:85`). See [5_processing_options.md](5_processing_options.md). |
| `plain/...`            | Indicates the source URL is percent-encoded and may include `@<extension>` to declare the output format.                             |
| `<base64url-source>`   | Encodes the source URL using URL-safe Base64 without padding (`=`). The extension, if present, is appended after a dot.              |

## Choosing between `plain` and Base64

Use `plain` when the source URL contains only characters legal within a path segment and does not already include a signature. Use Base64 to avoid double-encoding query strings or to embed signed/expiring source URLs.

### Examples

- Plain URL with format conversion:
  ```
  /<sig>/resize:fit:1024:0/plain/https://example.com/cats/siamese.jpg@webp
  ```
- Base64-encoded URL retaining the source format:
  ```
  /<sig>/resize:fill:800:600/https://example.com/cats/siamese.jpg (encoded) .jpg
  ```

Generate Base64 URL-safe strings without padding (replace `+` with `-`, `/` with `_`, and remove trailing `=`).

## Generating signatures

imgforge validates signatures by decoding `IMGFORGE_KEY` and `IMGFORGE_SALT` from hex and computing an HMAC-SHA256 digest over `salt || path`. The path starts with the slash preceding the processing options.

### Rust example

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

### Python example

```python
import base64, hmac, hashlib, os

key = bytes.fromhex(os.environ["IMGFORGE_KEY"])
salt = bytes.fromhex(os.environ["IMGFORGE_SALT"])
path = "/resize:fill:800:600/plain/https://example.com/cat.jpg@webp"

digest = hmac.new(key, salt + path.encode(), hashlib.sha256).digest()
signature = base64.urlsafe_b64encode(digest).rstrip(b"=").decode()
print(f"{signature}{path}")
```

### CLI helper

For quick experiments, use the official imgproxy helper (compatible with imgforge):

```bash
IMGFORGE_KEY=... IMGFORGE_SALT=... \
  imgproxy-url --key-env IMGFORGE_KEY --salt-env IMGFORGE_SALT \
  --resize fill 800 600 --format webp \
  https://example.com/cat.jpg
```

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

## Validating signatures in tests

Use the library’s `imgforge::url::validate_signature` helper:

```rust
let key = hex::decode(IMGFORGE_KEY).unwrap();
let salt = hex::decode(IMGFORGE_SALT).unwrap();
let valid = imgforge::url::validate_signature(&key, &salt, signature, path);
assert!(valid);
```

## Next steps

- Explore available transformations in [5_processing_options.md](5_processing_options.md).
- Review the request lifecycle in [6_processing_pipeline.md](6_processing_pipeline.md).
- If your application generates many URLs, encapsulate signing logic into a shared helper library to avoid drift.
