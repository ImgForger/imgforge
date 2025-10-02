# imgforge

imgforge is a fast and secure standalone server for resizing, processing, and converting remote images on the fly. It is a lightweight HTTP server that processes images based on URL parameters, emphasizing security, speed, and simplicity.

## Features

- **Image Processing**: Resize, crop, blur, and more.
- **Format Conversion**: Convert images between various formats (JPEG, PNG, WebP, AVIF, etc.).
- **Security**: Signed URLs to prevent abuse.
- **Configurable**: All options are configurable via environment variables.

## URL Structure

All requests follow this format:

```
http://your-server/{signature}/{processing_options}/plain/{encoded_source_url}@{extension}
```

or

```
http://your-server/{signature}/{processing_options}/{base64_encoded_source_url}.{extension}
```

- **Signature**: A hex-encoded HMAC-SHA256 signature of the URL path.
- **Processing Options**: A slash-separated list of processing options.
- **Source URL**: The URL of the source image, either plain (percent-encoded) or Base64-encoded.
- **Extension**: The desired output format.

## Processing Options

| Option | Shorthand | Description |
| --- | --- | --- |
| `resize` | `rs` | Resizes the image. |
| `size` | `sz` | Alias for resize without type (uses `fit`). |
| `resizing_type` | `rt` | The resizing mode to use. |
| `width` | `w` | The target width. |
| `height` | `h` | The target height. |
| `gravity` | `g` | The crop/resize anchor point. |
| `quality` | `q` | The compression quality (1-100). |
| `auto_rotate` | `ar` | Automatically rotate the image based on EXIF data. |
| `background` | `bg` | The background color to use. |
| `enlarge` | `el` | Allow enlarging the image. |
| `extend` | `ex` | Extend the image to the target dimensions. |
| `padding` | `p` | Add padding to the image. |
| `orientation` | - | Force a specific orientation. |
| `blur` | - | Apply a Gaussian blur. |
| `crop` | - | Crop the image. |
| `format` | - | The output format. |
| `dpr` | - | The device pixel ratio. |
| `cache_buster` | - | A value to bypass the cache. |
| `raw` | - | Bypass processing limits. |

## Security

imgforge uses signed URLs to prevent unauthorized use. The signature is a hex-encoded HMAC-SHA256 hash of the URL path, using a secret key and salt.

To enable unsigned URLs, set the `ALLOW_UNSIGNED` environment variable to `true`.

## Configuration

imgforge is configured using environment variables:

| Variable | Description |
| --- | --- |
| `IMGFORGE_KEY` | The secret key for signing URLs. |
| `IMGFORGE_SALT` | The salt for signing URLs. |
| `IMGFORGE_AUTH_TOKEN` | An optional authorization token. |
| `ALLOW_UNSIGNED` | Allow unsigned URLs. |
| `IMGFORGE_MAX_SRC_FILE_SIZE` | The maximum allowed source image file size. |
| `IMGFORGE_ALLOWED_MIME_TYPES` | A comma-separated list of allowed MIME types. |
| `IMGFORGE_MAX_SRC_RESOLUTION` | The maximum allowed source image resolution. |
| `IMGFORGE_MAX_ANIMATION_FRAMES` | The maximum number of frames in an animated image. |
| `IMGFORGE_MAX_ANIMATION_FRAME_RESOLUTION` | The maximum resolution of a single frame in an animated image. |
| `IMGFORGE_ALLOW_SECURITY_OPTIONS` | Allow security options to be set per-request. |
| `IMGFORGE_WORKERS` | The number of worker threads to use for image processing. |
| `IMGFORGE_LOG_LEVEL` | The log level to use. |

## Running the Application

To run the application, you will need to have Rust and Cargo installed.

1.  **Clone the repository**:

    ```bash
    git clone https://github.com/your-username/imgforge.git
    cd imgforge
    ```

2.  **Set the environment variables**:

    ```bash
    export IMGFORGE_KEY="your-secret-key"
    export IMGFORGE_SALT="your-secret-salt"
    ```

3.  **Run the application**:

    ```bash
    cargo run
    ```

The server will be available at `http://0.0.0.0:3000`.

## Endpoints

- **`/status`**: Returns a JSON object with the status of the server.
- **`/info/{*path}`**: Returns a JSON object with information about the source image.
- **`/{*path}`**: The main image processing endpoint.
