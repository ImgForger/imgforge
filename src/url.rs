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
