//! Server configuration, assembled from the environment at startup.

mod env_vars;

use crate::constants::*;
use crate::limits::{
    MaxAnimationFrameResolution, MaxAnimationFrames, MaxResultDimension, MaxSourceFileSize, MaxSourceResolution,
    SecurityLimitError,
};
use crate::processing::options::{OptionDefaults, ProcessingOption};
use crate::processing::presets::{parse_options_string, PresetError};
use env_vars::{bool_var, optional_var, parsed_var, security_limit_var};
use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid IMGFORGE_KEY")]
    InvalidKey(#[source] hex::FromHexError),
    #[error("invalid IMGFORGE_SALT")]
    InvalidSalt(#[source] hex::FromHexError),
    #[error("{name} contains invalid Unicode")]
    InvalidUnicode {
        name: &'static str,
        #[source]
        source: env::VarError,
    },
    #[error("invalid value for {name} ({value:?}): {source}")]
    InvalidSecurityLimit {
        name: &'static str,
        value: String,
        #[source]
        source: SecurityLimitError,
    },
    #[error("invalid presets: {0}")]
    InvalidPresets(#[source] PresetConfigError),
    #[error("invalid value for {name} ({value:?}): {source}")]
    InvalidDefaultFormat {
        name: &'static str,
        value: String,
        #[source]
        source: DefaultOutputFormatParseError,
    },
    #[error("invalid value for {name} ({value:?}): {source}")]
    InvalidWorkerCount {
        name: &'static str,
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("image-processing worker count must be greater than zero")]
    ZeroWorkers,
    #[error("image-processing worker count {value} exceeds the supported maximum of {max}")]
    WorkerCountTooLarge { value: usize, max: usize },
    #[error("invalid value for {name} ({value:?}): {reason}")]
    InvalidValue {
        name: &'static str,
        value: String,
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DefaultOutputFormat {
    #[default]
    Source,
    Jpeg,
    Png,
    Webp,
    Gif,
    Tiff,
    Avif,
    Heif,
}

impl DefaultOutputFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Gif => "gif",
            Self::Tiff => "tiff",
            Self::Avif => "avif",
            Self::Heif => "heif",
        }
    }

    pub const fn fixed_format(self) -> Option<&'static str> {
        match self {
            Self::Source => None,
            format => Some(format.as_str()),
        }
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("expected source, jpeg, png, webp, gif, tiff, avif, or heif")]
pub struct DefaultOutputFormatParseError;

impl FromStr for DefaultOutputFormat {
    type Err = DefaultOutputFormatParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "source" => Ok(Self::Source),
            "jpeg" | "jpg" => Ok(Self::Jpeg),
            "png" => Ok(Self::Png),
            "webp" => Ok(Self::Webp),
            "gif" => Ok(Self::Gif),
            "tiff" => Ok(Self::Tiff),
            "avif" => Ok(Self::Avif),
            "heif" | "heic" => Ok(Self::Heif),
            _ => Err(DefaultOutputFormatParseError),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PresetConfigError {
    #[error("invalid preset definition: {definition}")]
    InvalidDefinition { definition: String },
    #[error("invalid preset definition {name:?}: {source}")]
    InvalidOptions {
        name: String,
        #[source]
        source: PresetError,
    },
}

#[derive(Clone, Debug)]
pub struct Config {
    pub workers: usize,
    pub bind_address: String,
    pub prometheus_bind_address: Option<String>,
    pub timeout: u64,
    pub key: Vec<u8>,
    pub salt: Vec<u8>,
    pub allow_unsigned: bool,
    pub allow_security_options: bool,
    pub max_src_file_size: Option<MaxSourceFileSize>,
    pub max_src_resolution: Option<MaxSourceResolution>,
    pub max_result_dimension: Option<MaxResultDimension>,
    pub allowed_mime_types: Option<Vec<String>>,
    pub download_timeout: u64,
    pub secret: Option<String>,
    pub presets: HashMap<String, Vec<ProcessingOption>>,
    pub only_presets: bool,
    pub watermark_path: Option<String>,
    pub default_format: DefaultOutputFormat,
    pub rate_limit_per_minute: Option<u32>,
    /// Ceiling on how many frames of an animated source are decoded.
    pub max_animation_frames: Option<MaxAnimationFrames>,
    /// Ceiling on the pixel count of a single animation frame.
    pub max_animation_frame_resolution: Option<MaxAnimationFrameResolution>,
    /// Starting values for the processing options a URL may override.
    pub option_defaults: OptionDefaults,

    /// `max-age` for the `Cache-Control` header, in seconds.
    pub ttl: Option<u64>,
    /// Send the source's own `Cache-Control` instead of the configured TTL.
    pub cache_control_passthrough: bool,
    /// Emit an `ETag` and honour `If-None-Match`.
    pub use_etag: bool,
    /// Pass the source's `Last-Modified` through and honour `If-Modified-Since`.
    pub last_modified_enabled: bool,
    /// Emit a `Link: <source>; rel="canonical"` header.
    pub set_canonical_header: bool,
    /// Value for `Access-Control-Allow-Origin`, when cross-origin use is wanted.
    pub allow_origin: Option<String>,
    /// Path segment every route is mounted under.
    pub path_prefix: String,
    /// Path the liveness endpoint answers on.
    pub health_check_path: String,
    /// Return the underlying error text to the client.
    pub development_errors_mode: bool,
    /// Emit `X-Origin-*` headers describing the source image.
    pub enable_debug_headers: bool,

    /// `User-Agent` sent when fetching a source image.
    pub user_agent: String,
    /// How many redirects a source fetch may follow.
    pub max_redirects: usize,

    /// Serve WebP when the client's `Accept` says it can read it.
    pub enable_webp_detection: bool,
    /// Serve WebP to a client that accepts it even when the URL asks otherwise.
    pub enforce_webp: bool,
    /// Serve AVIF when the client's `Accept` says it can read it.
    pub enable_avif_detection: bool,
    /// Serve AVIF to a client that accepts it even when the URL asks otherwise.
    pub enforce_avif: bool,
    /// Honour the `Width` and `DPR` client hints.
    pub enable_client_hints: bool,
}

fn normalize_bind_address(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.parse::<u16>().is_ok() {
        format!("0.0.0.0:{}", trimmed)
    } else {
        trimmed.to_string()
    }
}

fn parse_presets(presets_str: &str) -> Result<HashMap<String, Vec<ProcessingOption>>, PresetConfigError> {
    let mut presets = HashMap::new();
    if presets_str.is_empty() {
        return Ok(presets);
    }

    for preset_def in presets_str.split(',') {
        let preset_def = preset_def.trim();
        if preset_def.is_empty() {
            continue;
        }

        let Some((name, options)) = preset_def.split_once('=') else {
            return Err(PresetConfigError::InvalidDefinition {
                definition: preset_def.to_string(),
            });
        };

        let name = name.trim();
        let options = options.trim();
        if name.is_empty() || options.is_empty() {
            return Err(PresetConfigError::InvalidDefinition {
                definition: preset_def.to_string(),
            });
        }

        let parsed_options = parse_options_string(options).map_err(|source| PresetConfigError::InvalidOptions {
            name: name.to_string(),
            source,
        })?;

        presets.insert(name.to_string(), parsed_options);
    }

    Ok(presets)
}

fn parse_optional_security_limit<T>(name: &'static str) -> Result<Option<T>, ConfigError>
where
    T: FromStr<Err = SecurityLimitError>,
{
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map(Some)
            .map_err(|source| ConfigError::InvalidSecurityLimit { name, value, source }),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(source @ env::VarError::NotUnicode(_)) => Err(ConfigError::InvalidUnicode { name, source }),
    }
}

fn default_worker_count() -> usize {
    num_cpus::get().max(1).saturating_mul(2)
}

fn parse_worker_count_from_env() -> Result<usize, ConfigError> {
    match env::var(ENV_WORKERS) {
        Ok(value) => {
            let configured = value
                .parse::<usize>()
                .map_err(|source| ConfigError::InvalidWorkerCount {
                    name: ENV_WORKERS,
                    value,
                    source,
                })?;
            Ok(if configured == 0 {
                default_worker_count()
            } else {
                configured
            })
        }
        Err(env::VarError::NotPresent) => Ok(default_worker_count()),
        Err(source @ env::VarError::NotUnicode(_)) => Err(ConfigError::InvalidUnicode {
            name: ENV_WORKERS,
            source,
        }),
    }
}

impl Config {
    /// Create a configuration with default values using raw key and salt bytes.
    pub fn new(key: Vec<u8>, salt: Vec<u8>) -> Self {
        Self {
            workers: default_worker_count(),
            bind_address: "0.0.0.0:3000".to_string(),
            prometheus_bind_address: None,
            timeout: 30,
            key,
            salt,
            allow_unsigned: false,
            allow_security_options: false,
            max_src_file_size: None,
            max_src_resolution: None,
            max_result_dimension: None,
            allowed_mime_types: None,
            download_timeout: 10,
            secret: None,
            presets: HashMap::new(),
            only_presets: false,
            watermark_path: None,
            default_format: DefaultOutputFormat::default(),
            rate_limit_per_minute: None,
            max_animation_frames: None,
            max_animation_frame_resolution: None,
            option_defaults: OptionDefaults::default(),
            ttl: None,
            cache_control_passthrough: false,
            use_etag: false,
            last_modified_enabled: false,
            set_canonical_header: false,
            allow_origin: None,
            path_prefix: String::new(),
            health_check_path: "/health".to_string(),
            development_errors_mode: false,
            enable_debug_headers: false,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            max_redirects: 10,
            enable_webp_detection: false,
            enforce_webp: false,
            enable_avif_detection: false,
            enforce_avif: false,
            enable_client_hints: false,
        }
    }

    /// The processing-option defaults a request starts from.
    pub fn option_defaults(&self) -> OptionDefaults {
        self.option_defaults
    }

    /// Create a configuration from hexadecimal key and salt strings.
    pub fn with_hex_keys(key_hex: &str, salt_hex: &str) -> Result<Self, ConfigError> {
        let key = hex::decode(key_hex).map_err(ConfigError::InvalidKey)?;
        let salt = hex::decode(salt_hex).map_err(ConfigError::InvalidSalt)?;
        Ok(Self::new(key, salt))
    }

    pub fn from_env() -> Result<Self, ConfigError> {
        let key_str = env::var(ENV_KEY).unwrap_or_default();
        let salt_str = env::var(ENV_SALT).unwrap_or_default();
        let mut config = Config::with_hex_keys(&key_str, &salt_str)?;

        config.workers = parse_worker_count_from_env()?;

        let bind_address_raw = env::var(ENV_BIND).unwrap_or_else(|_| "0.0.0.0:3000".to_string());
        config.bind_address = normalize_bind_address(&bind_address_raw);
        config.prometheus_bind_address = env::var(ENV_PROMETHEUS_BIND)
            .ok()
            .map(|value| normalize_bind_address(&value));
        config.timeout = env::var(ENV_TIMEOUT)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);

        config.allow_unsigned = env::var(ENV_ALLOW_UNSIGNED).unwrap_or_default().to_lowercase() == "true";
        config.allow_security_options =
            env::var(ENV_ALLOW_SECURITY_OPTIONS).unwrap_or_default().to_lowercase() == "true";

        config.max_src_file_size = parse_optional_security_limit(ENV_MAX_SRC_FILE_SIZE)?;
        config.max_src_resolution = parse_optional_security_limit(ENV_MAX_SRC_RESOLUTION)?;
        config.max_result_dimension = parse_optional_security_limit(ENV_MAX_RESULT_DIMENSION)?;
        config.allowed_mime_types = env::var(ENV_ALLOWED_MIME_TYPES)
            .ok()
            .map(|s| s.split(',').map(|s| s.to_string()).collect());
        config.download_timeout = env::var(ENV_DOWNLOAD_TIMEOUT)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(10);
        config.secret = env::var(ENV_SECRET).ok();

        config.presets =
            parse_presets(&env::var(ENV_PRESETS).unwrap_or_default()).map_err(ConfigError::InvalidPresets)?;
        config.only_presets = env::var(ENV_ONLY_PRESETS).unwrap_or_default().to_lowercase() == "true";

        config.watermark_path = env::var(ENV_WATERMARK_PATH).ok();
        match env::var(ENV_DEFAULT_FORMAT) {
            Ok(value) => {
                config.default_format = value.parse().map_err(|source| ConfigError::InvalidDefaultFormat {
                    name: ENV_DEFAULT_FORMAT,
                    value,
                    source,
                })?;
            }
            Err(env::VarError::NotPresent) => {}
            Err(source @ env::VarError::NotUnicode(_)) => {
                return Err(ConfigError::InvalidUnicode {
                    name: ENV_DEFAULT_FORMAT,
                    source,
                });
            }
        }
        config.rate_limit_per_minute = env::var(ENV_RATE_LIMIT_PER_MINUTE)
            .ok()
            .and_then(|s| s.parse::<u32>().ok());

        config.max_animation_frames = security_limit_var(ENV_MAX_ANIMATION_FRAMES)?;
        config.max_animation_frame_resolution = security_limit_var(ENV_MAX_ANIMATION_FRAME_RESOLUTION)?;

        config.option_defaults = OptionDefaults {
            auto_rotate: bool_var(ENV_AUTO_ROTATE, true)?,
            strip_metadata: bool_var(ENV_STRIP_METADATA, false)?,
            keep_copyright: bool_var(ENV_KEEP_COPYRIGHT, false)?,
            strip_color_profile: bool_var(ENV_STRIP_COLOR_PROFILE, false)?,
            preserve_hdr: bool_var(ENV_PRESERVE_HDR, false)?,
            enforce_thumbnail: bool_var(ENV_ENFORCE_THUMBNAIL, false)?,
            return_attachment: bool_var(ENV_RETURN_ATTACHMENT, false)?,
            quality: parsed_var::<u8>(ENV_QUALITY)?.map(|quality| quality.clamp(1, 100)),
        };

        config.ttl = parsed_var::<u64>(ENV_TTL)?;
        config.cache_control_passthrough = bool_var(ENV_CACHE_CONTROL_PASSTHROUGH, false)?;
        config.use_etag = bool_var(ENV_USE_ETAG, false)?;
        config.last_modified_enabled = bool_var(ENV_LAST_MODIFIED_ENABLED, false)?;
        config.set_canonical_header = bool_var(ENV_SET_CANONICAL_HEADER, false)?;
        config.allow_origin = optional_var(ENV_ALLOW_ORIGIN)?.filter(|value| !value.trim().is_empty());
        config.path_prefix = normalize_path_prefix(optional_var(ENV_PATH_PREFIX)?.as_deref());
        config.health_check_path = normalize_health_check_path(optional_var(ENV_HEALTH_CHECK_PATH)?.as_deref());
        config.development_errors_mode = bool_var(ENV_DEVELOPMENT_ERRORS_MODE, false)?;
        config.enable_debug_headers = bool_var(ENV_ENABLE_DEBUG_HEADERS, false)?;

        config.user_agent = optional_var(ENV_USER_AGENT)?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_USER_AGENT.to_string());
        config.max_redirects = parsed_var::<usize>(ENV_MAX_REDIRECTS)?.unwrap_or(10);

        config.enable_webp_detection = bool_var(ENV_ENABLE_WEBP_DETECTION, false)?;
        config.enforce_webp = bool_var(ENV_ENFORCE_WEBP, false)?;
        config.enable_avif_detection = bool_var(ENV_ENABLE_AVIF_DETECTION, false)?;
        config.enforce_avif = bool_var(ENV_ENFORCE_AVIF, false)?;
        config.enable_client_hints = bool_var(ENV_ENABLE_CLIENT_HINTS, false)?;

        Ok(config)
    }
}

/// Normalises a mount prefix to either empty or `/segment` with no trailing
/// slash, so routes can be built by concatenation without doubling separators.
fn normalize_path_prefix(prefix: Option<&str>) -> String {
    let Some(prefix) = prefix else {
        return String::new();
    };
    let trimmed = prefix.trim().trim_matches('/');
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("/{trimmed}")
    }
}

fn normalize_health_check_path(path: Option<&str>) -> String {
    let Some(path) = path else {
        return "/health".to_string();
    };
    let trimmed = path.trim().trim_matches('/');
    if trimmed.is_empty() {
        "/health".to_string()
    } else {
        format!("/{trimmed}")
    }
}

#[cfg(test)]
mod tests;
