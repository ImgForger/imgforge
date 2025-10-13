# Advanced Topics

This section covers advanced topics for users who want to get the most out of ImgForge.

## Processing Pipeline

Understanding the order in which ImgForge applies transformations is crucial for achieving the desired results. The processing pipeline is as follows:

1.  **Load Image**: The source image is fetched and decoded into memory.
2.  **Gravity & Crop**: If `gravity` or `crop` is specified, the image is cropped accordingly.
3.  **Resize**: The image is resized based on the `resize`, `size`, `width`, or `height` options.
4.  **Background & Padding**: If a `background` color or `padding` is specified, it is applied to the image.
5.  **Watermark**: (Pro) The watermark is applied.
6.  **Effects**: Other effects like `blur` are applied.
7.  **Encode**: The final image is encoded into the desired output format (`format` or extension) with the specified `quality`.

## Performance Tips

ImgForge is designed for high performance, but there are several ways you can optimize it further.

*   **Use a CDN**: Place a CDN in front of your ImgForge server to cache processed images. This will significantly reduce the load on your server for popular images.
*   **Adjust Worker Threads**: The `IMGPROXY_WORKERS` environment variable controls the number of worker threads. The default is the number of CPU cores, which is a good starting point. You may need to adjust this value based on your workload.
*   **Choose the Right Format**: Use modern image formats like WebP and AVIF to reduce file sizes and improve loading times.
*   **Optimize Quality**: The `quality` option can have a significant impact on file size. Experiment with different values to find the right balance between quality and size.

## Testing

ImgForge has a comprehensive test suite that covers URL parsing, signing, and image processing.

### Unit Tests

You can run the unit tests with the following command:

```sh
cargo test
```

### Integration Testing

For integration testing, you can use `curl` or any other HTTP client to make requests to a running ImgForge instance.

**Example test script:**

```sh
#!/bin/bash

BASE_URL="http://localhost:8080"
SOURCE_URL="https%3A%2F%2Fwww.rust-lang.org%2Fstatic%2Fimages%2Frust-logo-blk.svg"

# Test resize
curl -f "$BASE_URL/unsafe/w:100/plain/$SOURCE_URL" -o /dev/null
if [ $? -ne 0 ]; then
    echo "Resize test failed"
    exit 1
fi

# Test format conversion
curl -f "$BASE_URL/unsafe/format:png/plain/$SOURCE_URL" -o /dev/null
if [ $? -ne 0 ]; then
    echo "Format conversion test failed"
    exit 1
fi

echo "All tests passed"
```

This script tests a simple resize and a format conversion. You can expand it to cover all the features you use.
