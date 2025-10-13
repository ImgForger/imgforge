# Quick Start

Welcome to ImgForge! This guide will help you get started with our fast and secure image processing server.

## Installation

ImgForge is a Rust-based application. You can install it using `cargo`:

```bash
cargo install imgforge
```

## Running ImgForge

After installation, you can run ImgForge as a standalone server. By default, it will listen on port `3000`.

```bash
imgforge --bind :8080
```

This command starts the server, making it accessible at `http://localhost:8080` (or your specified bind address).

## Basic Usage: Image Processing via URL

ImgForge processes images based on parameters embedded in the URL. A typical URL structure looks like this:

```
http://your-server/{signature}/{processing_options}/plain/{encoded_source_url}@{extension}
```

### Example: Resizing and Converting an Image

Let's say you want to resize an image from `http://example.com/images/photo.jpg` to `300x200` pixels and convert it to `webp` format.

1.  **Source URL**: `http://example.com/images/photo.jpg`
2.  **Encoded Source URL**: For `plain` URLs, you need to percent-encode the source URL. 
    `http%3A%2F%2Fexample.com%2Fimages%2Fphoto.jpg`
3.  **Processing Options**: To resize to `300x200` using `fit` mode, and set quality to `80`, you'd use `resize:fit:300:200/quality:80`.
4.  **Output Extension**: `@webp`
5.  **Signature**: For simplicity in this quick start, we'll use `unsafe` (only recommended for development or if `ALLOW_UNSIGNED=true` is set in your environment configuration).

Combining these, your URL would look like:

```
http://localhost:8080/unsafe/resize:fit:300:200/quality:80/plain/http%3A%2F%2Fexample.com%2Fimages%2Fphoto.jpg@webp
```

You can access this URL in your browser or using `curl`:

```bash
curl -o output.webp "http://localhost:8080/unsafe/resize:fit:300:200/quality:80/plain/http%3A%2F%2Fexample.com%2Fimages%2Fphoto.jpg@webp"
```

This command will download the processed image as `output.webp`.

## Next Steps

-   **URL Format**: Learn more about the detailed URL structure and various ways to encode source URLs in [URL Format](url-format.md).
-   **Core Features**: Explore all available image processing options in [Core Features](core-features.md).
-   **Configuration**: Understand how to configure ImgForge using environment variables in [Configuration](configuration.md).
-   **Security**: Implement signed URLs for production environments as described in the [Security](#security) section (within Core Features).