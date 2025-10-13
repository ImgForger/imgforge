# Core Features

ImgForge provides a rich set of features for image processing, security, and optimization.

## URL Parsing & Fetching

ImgForge can fetch images from any remote URL. It supports both signed and unsigned URLs.

*   **Signed URLs**: In a production environment, all URLs should be signed. This prevents third parties from using your ImgForge instance to process arbitrary images. See the [URL Format](./url-format.md) documentation for how to generate signatures.
*   **Unsigned URLs**: For development, you can enable unsigned URLs by setting `ALLOW_UNSIGNED=true`. This allows you to use `unsafe` as the signature.

**Example `curl` request:**

```sh
curl "http://localhost:8080/unsafe/resize:fit:300:200/plain/https%3A%2F%2Fwww.rust-lang.org%2Fstatic%2Fimages%2Frust-logo-blk.svg" -o output.svg
```

## Basic Processing

These are the fundamental processing options available in ImgForge.

| Option          | Arguments                            | Description                                                                                             | Example                               |
| --------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| **resize**      | `type:width:height:enlarge:extend`   | Resizes the image. `type` can be `fit`, `fill`, `force`, or `auto`. `width` and `height` are in pixels. `enlarge` and `extend` are booleans (0 or 1). | `resize:fit:300:200:0:0`              |
| **size**        | `width:height:enlarge:extend`        | An alias for `resize` with the type defaulting to `fit`.                                                | `size:300:0:1:0`                      |
| **resizing_type** | `type`                               | Sets the resizing type.                                                                                 | `rt:fill`                             |
| **width**       | `pixels`                             | Sets the output width, preserving the aspect ratio.                                                     | `w:300`                               |
| **height**      | `pixels`                             | Sets the output height, preserving the aspect ratio.                                                    | `h:400`                               |
| **crop**        | `width:height:x_offset:y_offset`     | Crops the image to a specific size and position.                                                        | `crop:200:300:100:50`                 |
| **format**      | `format`                             | Specifies the output format.                                                                            | `format:webp`                         |
| **quality**     | `level`                              | Sets the compression quality for JPEG and WebP images (1-100).                                          | `q:80`                                |
| **auto_rotate** | `0` or `1`                           | Automatically rotates the image based on EXIF orientation data.                                         | `ar:1`                                |
| **orientation** | `angle`                              | Manually rotates the image. Angle can be `90`, `180`, or `270`.                                         | `orientation:90`                      |
| **background**  | `color`                              | Sets the background color for transparent images or when using `extend`. Color can be a hex code.       | `bg:FFFFFF`                           |

**Example URLs:**

*   **Resize to fit 500px width:**
    `http://localhost:8080/unsafe/w:500/plain/{source_url}`
*   **Fill a 300x300 square and convert to JPEG with 75% quality:**
    `http://localhost:8080/unsafe/resize:fill:300:300/format:jpg/q:75/plain/{source_url}`

## Advanced Processing

ImgForge also supports more advanced processing features.

| Option              | Arguments                               | Description                                                                                                                            | Example                                           |
| ------------------- | --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| **gravity**         | `type:x_offset:y_offset`                | Sets the anchor point for resizing and cropping. `type` can be `no` (north), `so` (south), `ea` (east), `we` (west), `ce` (center), `sm` (smart), or `faces` (Pro). | `g:sm`                                            |
| **blur**            | `sigma`                                 | Applies a Gaussian blur to the image. `sigma` controls the blur radius.                                                                | `blur:5`                                          |
| **dpr**             | `factor`                                | Applies a device pixel ratio to the image, scaling it up. `factor` is a float between 1.0 and 5.0.                                      | `dpr:2`                                           |
| **watermark**       | `opacity:position:x_offset:y_offset`    | (Pro) Adds a watermark to the image. The watermark itself is configured on the server.                                                 | `wm:0.5:ce:10:10`                                 |
| **video_thumbnail** | `time_in_seconds:step:keyframe`         | (Pro) Generates a thumbnail from a video file.                                                                                         | `video_thumbnail:5:1:0`                           |

**Example URLs:**

*   **Smart crop and apply a 2x DPR:**
    `http://localhost:8080/unsafe/gravity:sm/dpr:2/plain/{source_url}`
*   **Add a watermark with 50% opacity in the center:**
    `http://localhost:8080/unsafe/wm:0.5:ce/plain/{source_url}`

## Security

ImgForge includes several features to protect your server from abuse.

*   **Image Bomb Protection**: ImgForge validates incoming images to prevent "image bombs" (e.g., zip bombs disguised as images). You can configure limits for:
    *   `MAX_SRC_RESOLUTION`: Maximum source image resolution in megapixels.
    *   `MAX_SRC_FILE_SIZE`: Maximum source image file size in bytes.
    *   `MAX_ANIMATION_FRAMES`: Maximum number of frames for animated images.
*   **Per-Request Limits**: You can allow clients to specify security-related limits on a per-request basis by setting `ALLOW_SECURITY_OPTIONS=true`.
    *   Example: `max_src_resolution:10` (limits the source image to 10 megapixels for this request).

## Encryption

For maximum security, you can encrypt the source URL in your requests.

*   **AES Encryption**: Set an encryption key and use `enc/{aes_encrypted_source_url}` to pass the source URL. This is a Pro feature.

## Presets & Info Endpoint

*   **Presets**: Define reusable sets of processing options in your configuration.
    *   Example: If you have a preset named `thumbnail`, you can use `preset:thumbnail` in the URL.
*   **Info Endpoint**: Get metadata about a source image by using the `/info` endpoint.
    *   `http://localhost:8080/info/{signature}/{options}/plain/{source_url}`
    *   This returns a JSON object with information like width, height, format, etc.
