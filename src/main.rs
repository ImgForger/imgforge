use axum::{extract::Path, routing::get, Router};
use base64::{engine::general_purpose, Engine as _};
use percent_encoding::percent_decode_str;

#[derive(Debug)]
enum SourceUrlInfo {
    Plain {
        url: String,
        extension: Option<String>,
    },
    Base64 {
        encoded_url: String,
        extension: Option<String>,
    },
}

impl SourceUrlInfo {
    fn decode(&self) -> Result<String, String> {
        match self {
            SourceUrlInfo::Plain { url, .. } => percent_decode_str(url)
                .decode_utf8()
                .map(|s| s.to_string())
                .map_err(|e| e.to_string()),
            SourceUrlInfo::Base64 { encoded_url, .. } => general_purpose::URL_SAFE_NO_PAD
                .decode(encoded_url)
                .map_err(|e| e.to_string())
                .and_then(|bytes| String::from_utf8(bytes).map_err(|e| e.to_string())),
        }
    }
}

#[derive(Debug)]
struct ImgproxyUrl {
    signature: String,
    processing_options: String,
    source_url: SourceUrlInfo,
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/{*path}", get(image_proxy_handler));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn image_proxy_handler(Path(path): Path<String>) -> String {
    println!("Full path captured: {}", path);

    match parse_path(&path) {
        Some(url_parts) => {
            println!("Parsed URL parts: {:?}", url_parts);
            match url_parts.source_url.decode() {
                Ok(decoded_url) => {
                    format!("Decoded URL: {}", decoded_url)
                }
                Err(e) => {
                    format!("Error decoding URL: {}", e)
                }
            }
        }
        None => "Invalid URL format".to_string(),
    }
}

fn parse_path(path: &str) -> Option<ImgproxyUrl> {
    let parts: Vec<&str> = path.splitn(3, '/').collect();
    if parts.len() < 3 {
        return None;
    }

    let signature = parts[0].to_string();
    let processing_options = parts[1].to_string();
    let source_url_path = parts[2];

    let source_url = parse_source_url_path(source_url_path)?;

    Some(ImgproxyUrl {
        signature,
        processing_options,
        source_url,
    })
}

fn parse_source_url_path(path: &str) -> Option<SourceUrlInfo> {
    if let Some(plain_path) = path.strip_prefix("plain/") {
        let (url, extension) = match plain_path.rsplit_once('@') {
            Some((url, ext)) => (url.to_string(), Some(ext.to_string())),
            None => (plain_path.to_string(), None),
        };
        Some(SourceUrlInfo::Plain { url, extension })
    } else {
        let (encoded_url, extension) = match path.rsplit_once('.') {
            Some((url, ext)) => (url.to_string(), Some(ext.to_string())),
            None => (path.to_string(), None),
        };
        Some(SourceUrlInfo::Base64 {
            encoded_url,
            extension,
        })
    }
}
