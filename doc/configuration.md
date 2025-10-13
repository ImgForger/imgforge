# Configuration

ImgForge is configured entirely through environment variables. This makes it easy to deploy and manage in various environments, from local development to production.

To set a configuration option, set the corresponding environment variable before running the `imgforge` binary.

## General Configuration

| Environment Variable | Description                                     | Default Value |
| -------------------- | ----------------------------------------------- | ------------- |
| `IMGPROXY_BIND`      | The address and port to bind to.                | `:8080`       |
| `IMGPROXY_WORKERS`   | The number of worker threads to use.            | Number of CPU cores |
| `IMGPROXY_LOG_LEVEL` | The logging level (`error`, `warn`, `info`, `debug`, `trace`). | `info`        |

## Security Configuration

| Environment Variable        | Description                                                                                                | Default Value |
| --------------------------- | ---------------------------------------------------------------------------------------------------------- | ------------- |
| `IMGPROXY_KEY`              | The secret key for signing URLs (hex-encoded).                                                             | (none)        |
| `IMGPROXY_SALT`             | The salt for signing URLs (hex-encoded).                                                                   | (none)        |
| `IMGPROXY_ALLOW_UNSIGNED`   | If `true`, allows unsigned URLs with the `unsafe` signature. **Not recommended for production.**             | `false`       |
| `IMGPROXY_ALLOW_SECURITY_OPTIONS` | If `true`, allows per-request security options like `max_src_resolution`.                                | `false`       |
| `IMGPROXY_ENCRYPTION_KEY`   | The key for AES encryption of source URLs (Pro feature).                                                   | (none)        |

## Image Processing Configuration

| Environment Variable                 | Description                                                              | Default Value |
| ------------------------------------ | ------------------------------------------------------------------------ | ------------- |
| `IMGPROXY_DEFAULT_QUALITY`           | The default quality for JPEG and WebP images (1-100).                    | `85`          |
| `IMGPROXY_MAX_SRC_RESOLUTION`        | The maximum source image resolution in megapixels.                       | `50`          |
| `IMGPROXY_MAX_SRC_FILE_SIZE`         | The maximum source image file size in bytes.                             | `10485760` (10MB) |
| `IMGPROXY_MAX_ANIMATION_FRAMES`      | The maximum number of frames for animated images.                        | `100`         |
| `IMGPROXY_MAX_ANIMATION_FRAME_RESOLUTION` | The maximum resolution of a single frame in an animated image.         | `2000000` (2MP) |
| `IMGPROXY_ALLOW_SVG`                 | If `true`, allows processing of SVG images.                              | `true`        |
| `IMGPROXY_AUTO_ROTATE`               | If `true`, automatically rotates images based on EXIF data.              | `true`        |

## Presets Configuration

| Environment Variable | Description                                                                                                                                                           | Default Value |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------- |
| `IMGPROXY_PRESETS`   | A JSON object defining named presets for processing options. Example: `{"thumbnail": "resize:fit:100:100", "large": "w:1024"}` | (none)        |

## Example Usage

You can set these variables in your shell:

```sh
export IMGPROXY_KEY="your-secret-key"
export IMGPROXY_SALT="your-secret-salt"
export IMGPROXY_MAX_SRC_RESOLUTION=25
imgforge
```

Or, you can use a `.env` file and a tool like `dotenv` to load them.
