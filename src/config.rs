use crate::constants::*;
use crate::processing::options::ProcessingOption;
use crate::processing::presets::parse_options_string;
use std::collections::HashMap;
use std::env;

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
    pub max_src_file_size: Option<usize>,
    pub max_src_resolution: Option<f32>,
    pub allowed_mime_types: Option<Vec<String>>,
    pub download_timeout: u64,
    pub secret: Option<String>,
    pub presets: HashMap<String, Vec<ProcessingOption>>,
    pub only_presets: bool,
    pub watermark_path: Option<String>,
    pub rate_limit_per_minute: Option<u32>,
}

fn normalize_bind_address(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.parse::<u16>().is_ok() {
        format!("0.0.0.0:{}", trimmed)
    } else {
        trimmed.to_string()
    }
}

fn parse_presets(presets_str: &str) -> Result<HashMap<String, Vec<ProcessingOption>>, String> {
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
            return Err(format!("invalid preset definition: {}", preset_def));
        };

        let name = name.trim();
        let options = options.trim();
        if name.is_empty() || options.is_empty() {
            return Err(format!("invalid preset definition: {}", preset_def));
        }

        let parsed_options =
            parse_options_string(options).map_err(|e| format!("invalid preset definition '{}': {}", name, e))?;

        presets.insert(name.to_string(), parsed_options);
    }

    Ok(presets)
}

impl Config {
    /// Create a configuration with default values using raw key and salt bytes.
    pub fn new(key: Vec<u8>, salt: Vec<u8>) -> Self {
        Self {
            workers: num_cpus::get() * 2,
            bind_address: "0.0.0.0:3000".to_string(),
            prometheus_bind_address: None,
            timeout: 30,
            key,
            salt,
            allow_unsigned: false,
            allow_security_options: false,
            max_src_file_size: None,
            max_src_resolution: None,
            allowed_mime_types: None,
            download_timeout: 10,
            secret: None,
            presets: HashMap::new(),
            only_presets: false,
            watermark_path: None,
            rate_limit_per_minute: None,
        }
    }

    /// Create a configuration from hexadecimal key and salt strings.
    pub fn with_hex_keys(key_hex: &str, salt_hex: &str) -> Result<Self, String> {
        let key = hex::decode(key_hex).map_err(|_| "Invalid IMGFORGE_KEY".to_string())?;
        let salt = hex::decode(salt_hex).map_err(|_| "Invalid IMGFORGE_SALT".to_string())?;
        Ok(Self::new(key, salt))
    }

    pub fn from_env() -> Result<Self, String> {
        let key_str = env::var(ENV_KEY).unwrap_or_default();
        let salt_str = env::var(ENV_SALT).unwrap_or_default();
        let mut config = Config::with_hex_keys(&key_str, &salt_str)?;

        let workers = env::var(ENV_WORKERS)
            .unwrap_or_else(|_| "0".to_string())
            .parse()
            .unwrap_or(0);
        config.workers = if workers == 0 { num_cpus::get() * 2 } else { workers };

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

        config.max_src_file_size = env::var(ENV_MAX_SRC_FILE_SIZE).ok().and_then(|s| s.parse().ok());
        config.max_src_resolution = env::var(ENV_MAX_SRC_RESOLUTION).ok().and_then(|s| s.parse().ok());
        config.allowed_mime_types = env::var(ENV_ALLOWED_MIME_TYPES)
            .ok()
            .map(|s| s.split(',').map(|s| s.to_string()).collect());
        config.download_timeout = env::var(ENV_DOWNLOAD_TIMEOUT)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(10);
        config.secret = env::var(ENV_SECRET).ok();

        config.presets = parse_presets(&env::var(ENV_PRESETS).unwrap_or_default())?;
        config.only_presets = env::var(ENV_ONLY_PRESETS).unwrap_or_default().to_lowercase() == "true";

        config.watermark_path = env::var(ENV_WATERMARK_PATH).ok();
        config.rate_limit_per_minute = env::var(ENV_RATE_LIMIT_PER_MINUTE)
            .ok()
            .and_then(|s| s.parse::<u32>().ok());

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    lazy_static::lazy_static! {
        static ref ENV_LOCK: Mutex<()> = Mutex::new(());
    }

    fn restore_env_var(key: &str, original: Option<String>) {
        if let Some(value) = original {
            env::set_var(key, value);
        } else {
            env::remove_var(key);
        }
    }

    #[test]
    fn prometheus_numeric_port_maps_to_default_host() {
        let _guard = ENV_LOCK.lock().unwrap();
        let original_prometheus = env::var(ENV_PROMETHEUS_BIND).ok();

        env::set_var(ENV_PROMETHEUS_BIND, "3005");
        let config = Config::from_env().expect("config loads");

        assert_eq!(config.prometheus_bind_address.as_deref(), Some("0.0.0.0:3005"));

        restore_env_var(ENV_PROMETHEUS_BIND, original_prometheus);
    }

    #[test]
    fn bind_numeric_port_maps_to_default_host() {
        let _guard = ENV_LOCK.lock().unwrap();
        let original_bind = env::var(ENV_BIND).ok();
        let original_prometheus = env::var(ENV_PROMETHEUS_BIND).ok();

        env::set_var(ENV_BIND, "3456");
        env::remove_var(ENV_PROMETHEUS_BIND);

        let config = Config::from_env().expect("config loads");

        assert_eq!(config.bind_address, "0.0.0.0:3456");
        assert_eq!(config.prometheus_bind_address, None);

        restore_env_var(ENV_BIND, original_bind);
        restore_env_var(ENV_PROMETHEUS_BIND, original_prometheus);
    }

    #[test]
    fn test_parse_presets_single() {
        let presets_str = "thumbnail=resize:fit:150:150/quality:80";
        let presets = parse_presets(presets_str).expect("parses");
        assert_eq!(presets.len(), 1);
        assert_eq!(presets.get("thumbnail").map(|opts| opts.len()), Some(2));
    }

    #[test]
    fn test_parse_presets_multiple() {
        let presets_str = "thumbnail=resize:fit:150:150/quality:80,small=resize:fit:300:300/quality:85";
        let presets = parse_presets(presets_str).expect("parses");
        assert_eq!(presets.len(), 2);
        assert_eq!(presets.get("thumbnail").map(|opts| opts.len()), Some(2));
        assert_eq!(presets.get("small").map(|opts| opts.len()), Some(2));
    }

    #[test]
    fn test_parse_presets_empty() {
        let presets_str = "";
        let presets = parse_presets(presets_str).expect("parses");
        assert_eq!(presets.len(), 0);
    }

    #[test]
    fn test_parse_presets_with_spaces() {
        let presets_str = "thumbnail = resize:fit:150:150/quality:80 , small = resize:fit:300:300";
        let presets = parse_presets(presets_str).expect("parses");
        assert_eq!(presets.len(), 2);
        assert_eq!(presets.get("thumbnail").map(|opts| opts.len()), Some(2));
        assert_eq!(presets.get("small").map(|opts| opts.len()), Some(1));
    }

    #[test]
    fn test_parse_presets_default() {
        let presets_str = "default=quality:90/dpr:2";
        let presets = parse_presets(presets_str).expect("parses");
        assert_eq!(presets.len(), 1);
        assert_eq!(presets.get("default").map(|opts| opts.len()), Some(2));
    }

    #[test]
    fn test_parse_presets_invalid_format() {
        let presets_str = "thumbnail:resize:fit:150:150";
        assert!(parse_presets(presets_str).is_err());
    }

    #[test]
    fn test_parse_presets_missing_name() {
        let presets_str = "=resize:fit:150:150";
        assert!(parse_presets(presets_str).is_err());
    }

    #[test]
    fn test_parse_presets_missing_options() {
        let presets_str = "thumbnail=";
        assert!(parse_presets(presets_str).is_err());
    }

    #[test]
    fn test_config_presets_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        let original_presets = env::var(ENV_PRESETS).ok();
        let original_only_presets = env::var(ENV_ONLY_PRESETS).ok();

        env::set_var(ENV_PRESETS, "thumbnail=resize:fit:150:150,default=quality:90");
        env::set_var(ENV_ONLY_PRESETS, "true");

        let config = Config::from_env().expect("config loads");

        assert_eq!(config.presets.len(), 2);
        assert_eq!(config.presets.get("thumbnail").map(|opts| opts.len()), Some(1));
        assert_eq!(config.presets.get("default").map(|opts| opts.len()), Some(1));
        assert!(config.only_presets);

        restore_env_var(ENV_PRESETS, original_presets);
        restore_env_var(ENV_ONLY_PRESETS, original_only_presets);
    }

    #[test]
    fn test_config_only_presets_false_by_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        let original_only_presets = env::var(ENV_ONLY_PRESETS).ok();

        env::remove_var(ENV_ONLY_PRESETS);

        let config = Config::from_env().expect("config loads");

        assert!(!config.only_presets);

        restore_env_var(ENV_ONLY_PRESETS, original_only_presets);
    }
}
