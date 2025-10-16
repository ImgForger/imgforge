# Integration Tests for imgforge Handlers

This directory contains comprehensive integration tests for all API handlers in `src/handlers.rs`.

## Test Files

### handlers_integration_tests.rs

Contains 30 integration tests covering:

#### Status Handler (`/status`)
- Basic status endpoint functionality
- Response headers (X-Request-ID)

#### Info Handler (`/info/{*path}`)
- Unsigned URLs with `unsafe` signature
- Signed URLs with HMAC-SHA256 validation
- Invalid signatures
- Unsigned URLs when not allowed
- Bearer token authentication (valid/invalid/missing)
- Invalid URL formats
- Fetch errors for non-existent images

#### Image Forge Handler (`/{*path}`)
- Basic unsigned URL processing
- Resize transformations (fit mode)
- Quality adjustments
- Blur effects
- Sharpen effects
- Raw option (no processing)
- Crop operations
- Rotation
- DPR (Device Pixel Ratio) scaling
- Padding
- Background color
- Format conversion (webp, png)
- Plain URL encoding
- URL with extension markers
- Multiple processing options combined
- Invalid processing options
- Max file size enforcement
- Max resolution enforcement
- MIME type restrictions
- Signed URLs with HMAC validation

### handlers_integration_tests_extended.rs

Contains 10 additional integration tests covering:

#### Advanced Functionality
- **Caching**: Memory cache functionality verification
- **Concurrency**: Multiple simultaneous image processing requests
- **Complex Transformations**: Multiple options applied together
- **Security**: Security options behavior when disabled
- **Large Images**: Processing of high-resolution images
- **Format Conversion**: WebP format conversion
- **Transparency**: PNG images with alpha channel
- **Special Characters**: URLs with spaces and special characters
- **Resize Modes**: Different resize modes (fit, fill, auto)
- **Effects**: Pixelate effect

## Running Tests

### Run all integration tests:
```bash
cargo test --test handlers_integration_tests --test handlers_integration_tests_extended
```

### Run specific test:
```bash
cargo test --test handlers_integration_tests test_status_handler_success
```

### Run with output:
```bash
cargo test --test handlers_integration_tests -- --nocapture
```

### Run all tests (including unit tests):
```bash
cargo test
```

## Test Coverage

The integration tests cover:

✅ All three API handlers (status, info, image_forge)
✅ Authentication mechanisms (HMAC signatures, Bearer tokens)
✅ Security features (unsigned URLs, max file size, max resolution, MIME types)
✅ Image processing operations (resize, crop, rotate, blur, sharpen, etc.)
✅ Format conversions (JPEG, PNG, WebP)
✅ Caching behavior
✅ Error handling and edge cases
✅ Concurrent request handling
✅ URL encoding (plain and base64)

## Dependencies

The integration tests use:
- **wiremock**: Mock HTTP server for testing image fetching
- **tower**: Service trait implementations for Axum testing
- **hyper**: HTTP request/response handling
- **http-body-util**: HTTP body utilities
- **image**: Creating test images in memory
- **futures**: Async test utilities

## Test Structure

Each test follows this pattern:

1. **Setup**: Create mock server, test images, and configuration
2. **Configure**: Set up AppState with appropriate settings
3. **Execute**: Make HTTP request to handler via Axum Router
4. **Assert**: Verify response status, headers, and body

## Notes

- Tests use in-memory test images created with the `image` crate
- Mock servers simulate remote image sources
- Each test is isolated and uses its own mock server
- Tests verify both success and error scenarios
- URL signatures are generated dynamically for testing
- Cache tests verify both cache hits and misses
