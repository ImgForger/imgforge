use crate::constants::*;
use std::collections::HashMap;
use std::env;

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
    pub presets: HashMap<String, String>,
    pub only_presets: bool,
}

fn normalize_bind_address(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.parse::<u16>().is_ok() {
        format!("0.0.0.0:{}", trimmed)
    } else {
        trimmed.to_string()
    }
}

fn parse_presets(presets_str: &str) -> HashMap<String, String> {
    let mut presets = HashMap::new();
    if presets_str.is_empty() {
        return presets;
    }

    for preset_def in presets_str.split(',') {
        if let Some((name, options)) = preset_def.split_once('=') {
            let name = name.trim().to_string();
            let options = options.trim().to_string();
            if !name.is_empty() && !options.is_empty() {
                presets.insert(name, options);
            }
        }
    }

    presets
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let workers = env::var(ENV_WORKERS)
            .unwrap_or_else(|_| "0".to_string())
            .parse()
            .unwrap_or(0);
        let workers = if workers == 0 { num_cpus::get() * 2 } else { workers };

        let bind_address_raw = env::var(ENV_BIND).unwrap_or_else(|_| "0.0.0.0:3000".to_string());
        let bind_address = normalize_bind_address(&bind_address_raw);
        let prometheus_bind_address = env::var(ENV_PROMETHEUS_BIND)
            .ok()
            .map(|value| normalize_bind_address(&value));
        let timeout = env::var(ENV_TIMEOUT)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);

        let key_str = env::var(ENV_KEY).unwrap_or_default();
        let salt_str = env::var(ENV_SALT).unwrap_or_default();
        let key = hex::decode(key_str).map_err(|_| "Invalid IMGFORGE_KEY")?;
        let salt = hex::decode(salt_str).map_err(|_| "Invalid IMGFORGE_SALT")?;

        let allow_unsigned = env::var(ENV_ALLOW_UNSIGNED).unwrap_or_default().to_lowercase() == "true";
        let allow_security_options = env::var(ENV_ALLOW_SECURITY_OPTIONS).unwrap_or_default().to_lowercase() == "true";

        let max_src_file_size = env::var(ENV_MAX_SRC_FILE_SIZE).ok().and_then(|s| s.parse().ok());
        let max_src_resolution = env::var(ENV_MAX_SRC_RESOLUTION).ok().and_then(|s| s.parse().ok());
        let allowed_mime_types = env::var(ENV_ALLOWED_MIME_TYPES)
            .ok()
            .map(|s| s.split(',').map(|s| s.to_string()).collect());
        let download_timeout = env::var(ENV_DOWNLOAD_TIMEOUT)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(10);
        let secret = env::var(ENV_SECRET).ok();

        let presets = parse_presets(&env::var(ENV_PRESETS).unwrap_or_default());
        let only_presets = env::var(ENV_ONLY_PRESETS).unwrap_or_default().to_lowercase() == "true";

        Ok(Self {
            workers,
            bind_address,
            prometheus_bind_address,
            timeout,
            key,
            salt,
            allow_unsigned,
            allow_security_options,
            max_src_file_size,
            max_src_resolution,
            allowed_mime_types,
            download_timeout,
            secret,
            presets,
            only_presets,
        })
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
        let presets = parse_presets(presets_str);
        assert_eq!(presets.len(), 1);
        assert_eq!(
            presets.get("thumbnail"),
            Some(&"resize:fit:150:150/quality:80".to_string())
        );
    }

    #[test]
    fn test_parse_presets_multiple() {
        let presets_str = "thumbnail=resize:fit:150:150/quality:80,small=resize:fit:300:300/quality:85";
        let presets = parse_presets(presets_str);
        assert_eq!(presets.len(), 2);
        assert_eq!(
            presets.get("thumbnail"),
            Some(&"resize:fit:150:150/quality:80".to_string())
        );
        assert_eq!(presets.get("small"), Some(&"resize:fit:300:300/quality:85".to_string()));
    }

    #[test]
    fn test_parse_presets_empty() {
        let presets_str = "";
        let presets = parse_presets(presets_str);
        assert_eq!(presets.len(), 0);
    }

    #[test]
    fn test_parse_presets_with_spaces() {
        let presets_str = "thumbnail = resize:fit:150:150/quality:80 , small = resize:fit:300:300";
        let presets = parse_presets(presets_str);
        assert_eq!(presets.len(), 2);
        assert_eq!(
            presets.get("thumbnail"),
            Some(&"resize:fit:150:150/quality:80".to_string())
        );
        assert_eq!(presets.get("small"), Some(&"resize:fit:300:300".to_string()));
    }

    #[test]
    fn test_parse_presets_default() {
        let presets_str = "default=quality:90/dpr:2";
        let presets = parse_presets(presets_str);
        assert_eq!(presets.len(), 1);
        assert_eq!(presets.get("default"), Some(&"quality:90/dpr:2".to_string()));
    }

    #[test]
    fn test_parse_presets_invalid_format() {
        let presets_str = "thumbnail:resize:fit:150:150";
        let presets = parse_presets(presets_str);
        assert_eq!(presets.len(), 0);
    }

    #[test]
    fn test_parse_presets_missing_name() {
        let presets_str = "=resize:fit:150:150";
        let presets = parse_presets(presets_str);
        assert_eq!(presets.len(), 0);
    }

    #[test]
    fn test_parse_presets_missing_options() {
        let presets_str = "thumbnail=";
        let presets = parse_presets(presets_str);
        assert_eq!(presets.len(), 0);
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
        assert_eq!(config.presets.get("thumbnail"), Some(&"resize:fit:150:150".to_string()));
        assert_eq!(config.presets.get("default"), Some(&"quality:90".to_string()));
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
