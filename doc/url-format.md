# URL Format

ImgForge processes images based on parameters embedded directly within the URL. Understanding this structure is key to effectively using the service. All requests follow a consistent format, allowing for flexible image manipulation on the fly.

## General URL Structure

ImgForge supports two primary URL structures for specifying the source image:

1.  **Plain (Percent-Encoded) Source URL**:
    ```
    http://your-server/{signature}/{processing_options}/plain/{encoded_source_url}@{extension}
    ```

2.  **Base64-Encoded Source URL**:
    ```
    http://your-server/{signature}/{processing_options}/{base64_encoded_source_url}.{extension}
    ```

3.  **(Pro Feature) AES-Encrypted Source URL**:
    ```
    http://your-server/{signature}/{processing_options}/enc/{aes_encrypted_source_url}.{extension}
    ```

### Components Explained

-   `http://your-server`: The base URL of your ImgForge instance (e.g., `http://localhost:8080`).

-   `{signature}`: A hexadecimal string used for URL validation. This is a critical security feature. If disabled via configuration (`ALLOW_UNSIGNED=true`), you can use `unsafe` or `_` as a placeholder. For signed URLs, the signature is computed as `hex(hmac_sha256(key, salt + url_path))`. Refer to the [Security](#security) section for more details.

-   `{processing_options}`: A slash-separated list of image processing options. Each option consists of an `option_name` followed by its arguments, separated by colons (e.g., `resize:fit:300:200`). The order of options does not matter.

-   `plain/`: Indicates that the subsequent `{encoded_source_url}` is a standard percent-encoded URL.

-   `{encoded_source_url}`: The full URL of the original image, percent-encoded. For example, `http://example.com/image.jpg` becomes `http%3A%2F%2Fexample.com%2Fimage.jpg`.

-   `{base64_encoded_source_url}`: The full URL of the original image, URL-safe Base64 encoded. This is useful for very long URLs or when dealing with characters that might be problematic with percent-encoding. If the Base64 string is very long, it can be split with `/` characters (e.g., `/aHR0cDovL2V4YW1w/bGUuY29tL2ltYWdl/cy9jdXJpb3NpdHku/anBn`).

-   `enc/`: (Pro Feature) Indicates that the subsequent `{aes_encrypted_source_url}` is an AES-CBC encrypted URL. This provides an additional layer of security and obfuscation. Requires a 16, 24, or 32-byte key.

-   `@{extension}` or `.{extension}`: Specifies the desired output format of the processed image (e.g., `@png`, `.webp`). If omitted, ImgForge will attempt to use the source image's format or default to JPEG if the source format is unsupported.

### Example URLs

#### 1. Resize and Convert (Plain URL)

Resizes `http://example.com/photos/image.png` to `400px` width (auto height), sets quality to `75`, and outputs as `jpeg`.

```
/unsafe/resize:fit:400:0/quality:75/plain/http%3A%2F%2Fexample.com%2Fphotos%2Fimage.png@jpeg
```

#### 2. Crop and Blur (Base64 URL)

Crops `http://example.com/gallery/pic.jpeg` (Base64 encoded) to `100x100` at `10,20` offset, applies a `5.0` blur, and outputs as `png`.

```
/unsafe/crop:10:20:100:100/blur:5.0/aHR0cDovL2V4YW1w/bGUuY29tL2dhbGxl/cnkvcGljLmpwZWc=.png
```

#### 3. Signed URL Example

Assuming `KEY` and `SALT` are configured, a signed URL for resizing `http://example.com/img.gif` to `200x200` (fill mode) and outputting as `webp` might look like this:

```
/AfrOrF3gWeDA6VOlDG4TzxMv39O7MXnF4CXpKUwGqRM/resize:fill:200:200/plain/http%3A%2F%2Fexample.com%2Fimg.gif@webp
```

(The signature `AfrOrF3gWeDA6VOlDG4TzxMv39O7MXnF4CXpKUwGqRM` is illustrative and would be dynamically generated based on your `KEY`, `SALT`, and the specific URL path.)

## Validation

ImgForge performs validation on source URLs, including:

-   **Protocol**: Only `http` and `https` schemes are allowed by default (configurable).
-   **Allowed Domains**: Source image domains can be restricted via configuration.
-   **Image Bomb Protection**: Limits on MIME type, dimensions, and file size are enforced to prevent abuse (see [Security](#security)).

Invalid URLs or options will result in appropriate HTTP error responses (e.g., `400 Bad Request`).