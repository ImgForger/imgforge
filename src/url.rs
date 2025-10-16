use crate::processing::options::ProcessingOption;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use percent_encoding::percent_decode_str;
use sha2::Sha256;

/// Information about the source URL, including its type and extension.
#[derive(Debug)]
pub enum SourceUrlInfo {
    /// A plain (percent-encoded) source URL.
    Plain { url: String },
    /// A Base64-encoded source URL.
    Base64 { encoded_url: String },
}

impl SourceUrlInfo {
    /// Decodes the source URL based on its type.
    /// Returns the decoded URL as a String or an error message.
    pub fn decode(&self) -> Result<String, String> {
        match self {
            SourceUrlInfo::Plain { url, .. } => percent_decode_str(url)
                .decode_utf8()
                .map(|s| s.to_string())
                .map_err(|e| e.to_string()),
            SourceUrlInfo::Base64 { encoded_url, .. } => URL_SAFE_NO_PAD
                .decode(encoded_url)
                .map_err(|e| e.to_string())
                .and_then(|bytes| String::from_utf8(bytes).map_err(|e| e.to_string())),
        }
    }
}

/// Represents the parsed components of an imgforge URL.
#[derive(Debug)]
pub struct ImgforgeUrl {
    /// The signature used for URL validation.
    pub signature: String,
    /// A list of processing options to apply to the image.
    pub processing_options: Vec<ProcessingOption>,
    /// Information about the source image URL.
    pub source_url: SourceUrlInfo,
}

/// Validates the URL signature using HMAC-SHA256.
pub fn validate_signature(key: &[u8], salt: &[u8], signature: &str, path: &str) -> bool {
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(salt);
    mac.update(path.as_bytes());

    let decoded_signature = match URL_SAFE_NO_PAD.decode(signature) {
        Ok(s) => s,
        Err(_) => return false,
    };
    mac.verify_slice(&decoded_signature).is_ok()
}

/// Parses the incoming URL path into its imgforge components.
pub fn parse_path(path: &str) -> Option<ImgforgeUrl> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 2 {
        return None;
    }

    let signature = parts[0].to_string();
    let rest = &parts[1..];

    let source_url_start_index = rest
        .iter()
        .position(|&s| s == "plain" || !s.contains(':'))
        .unwrap_or(rest.len());

    let processing_options_parts = &rest[..source_url_start_index];
    let source_url_parts = &rest[source_url_start_index..];

    let mut processing_options: Vec<ProcessingOption> = processing_options_parts
        .iter()
        .map(|s| {
            let mut parts = s.split(':');
            let name = parts.next().unwrap_or("").to_string();
            let args = parts.map(|s| s.to_string()).collect();
            ProcessingOption { name, args }
        })
        .collect();

    let (source_url, extension) = parse_source_url_path(source_url_parts)?;

    if let Some(ext) = extension {
        processing_options.push(ProcessingOption {
            name: "format".to_string(),
            args: vec![ext.clone()],
        });
    }

    Some(ImgforgeUrl {
        signature,
        processing_options,
        source_url,
    })
}

/// Parses the source URL path segment into `SourceUrlInfo`.
fn parse_source_url_path(parts: &[&str]) -> Option<(SourceUrlInfo, Option<String>)> {
    if parts.is_empty() {
        return None;
    }

    if parts[0] == "plain" {
        if parts.len() < 2 {
            return None;
        }
        let path = parts[1..].join("/");
        let (url, extension) = match path.rsplit_once('@') {
            Some((url, ext)) => (url.to_string(), Some(ext.to_string())),
            None => (path.to_string(), None),
        };
        Some((SourceUrlInfo::Plain { url }, extension))
    } else {
        let path = parts.join("/");
        let (encoded_url, extension) = match path.rsplit_once('.') {
            Some((url, ext)) => (url.to_string(), Some(ext.to_string())),
            None => (path.to_string(), None),
        };
        Some((SourceUrlInfo::Base64 { encoded_url }, extension))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_url_info_decode_plain() {
        let source = SourceUrlInfo::Plain {
            url: "https%3A%2F%2Fexample.com%2Fimage.jpg".to_string(),
        };
        let decoded = source.decode().unwrap();
        assert_eq!(decoded, "https://example.com/image.jpg");
    }

    #[test]
    fn test_source_url_info_decode_plain_no_encoding() {
        let source = SourceUrlInfo::Plain {
            url: "https://example.com/image.jpg".to_string(),
        };
        let decoded = source.decode().unwrap();
        assert_eq!(decoded, "https://example.com/image.jpg");
    }

    #[test]
    fn test_source_url_info_decode_base64() {
        let url = "https://example.com/image.jpg";
        let encoded = URL_SAFE_NO_PAD.encode(url.as_bytes());
        let source = SourceUrlInfo::Base64 {
            encoded_url: encoded,
        };
        let decoded = source.decode().unwrap();
        assert_eq!(decoded, url);
    }

    #[test]
    fn test_source_url_info_decode_base64_invalid() {
        let source = SourceUrlInfo::Base64 {
            encoded_url: "invalid!!!base64".to_string(),
        };
        assert!(source.decode().is_err());
    }

    #[test]
    fn test_validate_signature_valid() {
        let key = b"test_key";
        let salt = b"test_salt";
        let path = "/resize:fill:300:200/plain/https://example.com/image.jpg";
        
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(key).unwrap();
        mac.update(salt);
        mac.update(path.as_bytes());
        let signature_bytes = mac.finalize().into_bytes();
        let signature = URL_SAFE_NO_PAD.encode(&signature_bytes);

        assert!(validate_signature(key, salt, &signature, path));
    }

    #[test]
    fn test_validate_signature_invalid() {
        let key = b"test_key";
        let salt = b"test_salt";
        let path = "/resize:fill:300:200/plain/https://example.com/image.jpg";
        let invalid_signature = "invalid_signature";

        assert!(!validate_signature(key, salt, invalid_signature, path));
    }

    #[test]
    fn test_validate_signature_wrong_path() {
        let key = b"test_key";
        let salt = b"test_salt";
        let path = "/resize:fill:300:200/plain/https://example.com/image.jpg";
        
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(key).unwrap();
        mac.update(salt);
        mac.update(path.as_bytes());
        let signature_bytes = mac.finalize().into_bytes();
        let signature = URL_SAFE_NO_PAD.encode(&signature_bytes);

        let wrong_path = "/resize:fill:300:200/plain/https://example.com/other.jpg";
        assert!(!validate_signature(key, salt, &signature, wrong_path));
    }

    #[test]
    fn test_parse_path_with_resize_and_plain_url() {
        let path = "signature123/resize:fill:300:200/plain/https://example.com/image.jpg";
        let parsed = parse_path(path).unwrap();

        assert_eq!(parsed.signature, "signature123");
        assert_eq!(parsed.processing_options.len(), 1);
        assert_eq!(parsed.processing_options[0].name, "resize");
        assert_eq!(parsed.processing_options[0].args, vec!["fill", "300", "200"]);

        match parsed.source_url {
            SourceUrlInfo::Plain { url } => {
                assert_eq!(url, "https://example.com/image.jpg");
            }
            _ => panic!("Expected Plain source URL"),
        }
    }

    #[test]
    fn test_parse_path_with_plain_url_and_extension() {
        let path = "sig/resize:fill:300:200/plain/https://example.com/image.jpg@webp";
        let parsed = parse_path(path).unwrap();

        assert_eq!(parsed.processing_options.len(), 2);
        assert_eq!(parsed.processing_options[0].name, "resize");
        assert_eq!(parsed.processing_options[1].name, "format");
        assert_eq!(parsed.processing_options[1].args, vec!["webp"]);
    }

    #[test]
    fn test_parse_path_with_base64_url() {
        let url = "https://example.com/image.jpg";
        let encoded = URL_SAFE_NO_PAD.encode(url.as_bytes());
        let path = format!("sig/resize:fill:300:200/{}", encoded);
        let parsed = parse_path(&path).unwrap();

        assert_eq!(parsed.signature, "sig");
        assert_eq!(parsed.processing_options.len(), 1);
        match parsed.source_url {
            SourceUrlInfo::Base64 { encoded_url } => {
                assert_eq!(encoded_url, encoded);
            }
            _ => panic!("Expected Base64 source URL"),
        }
    }

    #[test]
    fn test_parse_path_with_base64_url_and_extension() {
        let url = "https://example.com/image.jpg";
        let encoded = URL_SAFE_NO_PAD.encode(url.as_bytes());
        let path = format!("sig/resize:fill:300:200/{}.webp", encoded);
        let parsed = parse_path(&path).unwrap();

        assert_eq!(parsed.processing_options.len(), 2);
        assert_eq!(parsed.processing_options[0].name, "resize");
        assert_eq!(parsed.processing_options[1].name, "format");
        assert_eq!(parsed.processing_options[1].args, vec!["webp"]);
    }

    #[test]
    fn test_parse_path_with_multiple_options() {
        let path = "sig/resize:fill:300:200/quality:90/blur:5/plain/https://example.com/image.jpg";
        let parsed = parse_path(path).unwrap();

        assert_eq!(parsed.processing_options.len(), 3);
        assert_eq!(parsed.processing_options[0].name, "resize");
        assert_eq!(parsed.processing_options[1].name, "quality");
        assert_eq!(parsed.processing_options[2].name, "blur");
    }

    #[test]
    fn test_parse_path_no_options() {
        let path = "sig/plain/https://example.com/image.jpg";
        let parsed = parse_path(path).unwrap();

        assert_eq!(parsed.signature, "sig");
        assert_eq!(parsed.processing_options.len(), 0);
    }

    #[test]
    fn test_parse_path_too_short() {
        let path = "sig";
        assert!(parse_path(path).is_none());
    }

    #[test]
    fn test_parse_path_empty() {
        let path = "";
        assert!(parse_path(path).is_none());
    }

    #[test]
    fn test_parse_source_url_path_plain_with_extension() {
        let parts = vec!["plain", "https://example.com/image.jpg@webp"];
        let (source, ext) = parse_source_url_path(&parts).unwrap();

        match source {
            SourceUrlInfo::Plain { url } => {
                assert_eq!(url, "https://example.com/image.jpg");
            }
            _ => panic!("Expected Plain source URL"),
        }
        assert_eq!(ext, Some("webp".to_string()));
    }

    #[test]
    fn test_parse_source_url_path_plain_no_extension() {
        let parts = vec!["plain", "https://example.com/image.jpg"];
        let (source, ext) = parse_source_url_path(&parts).unwrap();

        match source {
            SourceUrlInfo::Plain { url } => {
                assert_eq!(url, "https://example.com/image.jpg");
            }
            _ => panic!("Expected Plain source URL"),
        }
        assert_eq!(ext, None);
    }

    #[test]
    fn test_parse_source_url_path_plain_multipart() {
        let parts = vec!["plain", "https://example.com", "path", "to", "image.jpg"];
        let (source, ext) = parse_source_url_path(&parts).unwrap();

        match source {
            SourceUrlInfo::Plain { url } => {
                assert_eq!(url, "https://example.com/path/to/image.jpg");
            }
            _ => panic!("Expected Plain source URL"),
        }
        assert_eq!(ext, None);
    }

    #[test]
    fn test_parse_source_url_path_plain_only() {
        let parts = vec!["plain"];
        assert!(parse_source_url_path(&parts).is_none());
    }

    #[test]
    fn test_parse_source_url_path_base64_with_extension() {
        let parts = vec!["encoded123.webp"];
        let (source, ext) = parse_source_url_path(&parts).unwrap();

        match source {
            SourceUrlInfo::Base64 { encoded_url } => {
                assert_eq!(encoded_url, "encoded123");
            }
            _ => panic!("Expected Base64 source URL"),
        }
        assert_eq!(ext, Some("webp".to_string()));
    }

    #[test]
    fn test_parse_source_url_path_base64_no_extension() {
        let parts = vec!["encoded123"];
        let (source, ext) = parse_source_url_path(&parts).unwrap();

        match source {
            SourceUrlInfo::Base64 { encoded_url } => {
                assert_eq!(encoded_url, "encoded123");
            }
            _ => panic!("Expected Base64 source URL"),
        }
        assert_eq!(ext, None);
    }

    #[test]
    fn test_parse_source_url_path_empty() {
        let parts: Vec<&str> = vec![];
        assert!(parse_source_url_path(&parts).is_none());
    }
}
