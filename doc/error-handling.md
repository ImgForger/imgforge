# Error Handling

When ImgForge encounters an error, it returns an appropriate HTTP status code and a descriptive error message. This helps you diagnose and troubleshoot issues with your image processing requests.

## Common HTTP Status Codes

Here are some of the most common HTTP status codes you might encounter when using ImgForge:

| Status Code | Meaning                  | Description                                                                                                                               |
| ----------- | ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `200 OK`      | Success                  | The image was processed successfully and is returned in the response body.                                                                |
| `400 Bad Request` | Invalid Options          | The processing options in the URL are invalid or malformed. For example, `resize:foo` where `foo` is not a valid resizing type.         |
| `403 Forbidden` | Invalid Signature        | The signature in the URL is missing or invalid. This can also happen if you try to use an unsigned URL when `ALLOW_UNSIGNED` is `false`. |
| `404 Not Found` | Source Image Not Found | The source image could not be fetched from the remote URL. This could be because the URL is incorrect or the image does not exist.      |
| `500 Internal Server Error` | Processing Error         | An unexpected error occurred on the server while processing the image. Check the ImgForge logs for more details.                |

## Error Responses

When an error occurs, ImgForge returns a plain text response with a descriptive error message.

**Example 400 Bad Request:**

```
HTTP/1.1 400 Bad Request
Content-Type: text/plain

Invalid resizing type: foo
```

**Example 403 Forbidden:**

```
HTTP/1.1 403 Forbidden
Content-Type: text/plain

Invalid signature
```

## Troubleshooting

*   **Check the URL:** The most common source of errors is a malformed URL. Double-check your processing options, encoding, and signature.
*   **Check the Source Image:** Ensure that the source image URL is correct and that the image is accessible from your ImgForge server.
*   **Check the Logs:** ImgForge provides detailed logs that can help you diagnose server-side errors. Increase the `LOG_LEVEL` to `debug` or `trace` for more information.
