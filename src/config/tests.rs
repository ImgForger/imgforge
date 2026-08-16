//! Configuration tests.

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
fn invalid_worker_count_fails_configuration() {
    let _guard = ENV_LOCK.lock().unwrap();
    let original = env::var(ENV_WORKERS).ok();

    env::set_var(ENV_WORKERS, "invalid");
    let result = Config::from_env();

    assert!(matches!(
        result,
        Err(ConfigError::InvalidWorkerCount {
            name: ENV_WORKERS,
            value,
            ..
        }) if value == "invalid"
    ));
    restore_env_var(ENV_WORKERS, original);
}

#[test]
fn zero_worker_count_selects_the_automatic_default() {
    let _guard = ENV_LOCK.lock().unwrap();
    let original = env::var(ENV_WORKERS).ok();

    env::set_var(ENV_WORKERS, "0");
    let config = Config::from_env().expect("config loads");

    assert_eq!(config.workers, default_worker_count());
    restore_env_var(ENV_WORKERS, original);
}

#[test]
fn explicit_worker_count_is_preserved() {
    let _guard = ENV_LOCK.lock().unwrap();
    let original = env::var(ENV_WORKERS).ok();

    env::set_var(ENV_WORKERS, "3");
    let config = Config::from_env().expect("config loads");

    assert_eq!(config.workers, 3);
    restore_env_var(ENV_WORKERS, original);
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
fn test_config_default_format_from_env() {
    let _guard = ENV_LOCK.lock().unwrap();
    let original = env::var(ENV_DEFAULT_FORMAT).ok();

    env::remove_var(ENV_DEFAULT_FORMAT);
    let config = Config::from_env().expect("config loads");
    assert_eq!(config.default_format, DefaultOutputFormat::Source);

    env::set_var(ENV_DEFAULT_FORMAT, "JPEG");
    let config = Config::from_env().expect("config loads");
    assert_eq!(config.default_format, DefaultOutputFormat::Jpeg);

    env::set_var(ENV_DEFAULT_FORMAT, "heic");
    let config = Config::from_env().expect("config loads");
    assert_eq!(config.default_format, DefaultOutputFormat::Heif);

    env::set_var(ENV_DEFAULT_FORMAT, "bmp");
    assert!(matches!(
        Config::from_env(),
        Err(ConfigError::InvalidDefaultFormat { value, .. }) if value == "bmp"
    ));

    restore_env_var(ENV_DEFAULT_FORMAT, original);
}

#[test]
fn default_output_format_normalizes_aliases() {
    assert_eq!("jpg".parse(), Ok(DefaultOutputFormat::Jpeg));
    assert_eq!(" HEIC ".parse(), Ok(DefaultOutputFormat::Heif));
    assert_eq!(DefaultOutputFormat::Heif.as_str(), "heif");
    assert!("bmp".parse::<DefaultOutputFormat>().is_err());
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

#[test]
fn invalid_max_source_file_size_fails_configuration() {
    let _guard = ENV_LOCK.lock().unwrap();
    let original = env::var(ENV_MAX_SRC_FILE_SIZE).ok();

    env::set_var(ENV_MAX_SRC_FILE_SIZE, "invalid");
    let result = Config::from_env();

    assert!(matches!(
        result,
        Err(ConfigError::InvalidSecurityLimit {
            name: ENV_MAX_SRC_FILE_SIZE,
            ..
        })
    ));
    restore_env_var(ENV_MAX_SRC_FILE_SIZE, original);
}

#[test]
fn non_finite_max_source_resolution_fails_configuration() {
    let _guard = ENV_LOCK.lock().unwrap();
    let original = env::var(ENV_MAX_SRC_RESOLUTION).ok();

    for value in ["NaN", "inf", "-inf"] {
        env::set_var(ENV_MAX_SRC_RESOLUTION, value);
        let result = Config::from_env();
        assert!(matches!(
            result,
            Err(ConfigError::InvalidSecurityLimit {
                name: ENV_MAX_SRC_RESOLUTION,
                ..
            })
        ));
    }

    restore_env_var(ENV_MAX_SRC_RESOLUTION, original);
}

#[test]
fn max_result_dimension_is_validated_at_startup() {
    let _guard = ENV_LOCK.lock().unwrap();
    let original = env::var(ENV_MAX_RESULT_DIMENSION).ok();

    env::set_var(ENV_MAX_RESULT_DIMENSION, "8192");
    let config = Config::from_env().expect("valid dimension");
    assert_eq!(config.max_result_dimension.map(MaxResultDimension::get), Some(8192));

    // A malformed ceiling stops startup rather than silently leaving the
    // result size unbounded.
    for value in ["0", "-1", "huge"] {
        env::set_var(ENV_MAX_RESULT_DIMENSION, value);
        assert!(matches!(
            Config::from_env(),
            Err(ConfigError::InvalidSecurityLimit {
                name: ENV_MAX_RESULT_DIMENSION,
                ..
            })
        ));
    }

    restore_env_var(ENV_MAX_RESULT_DIMENSION, original);
}

#[test]
fn valid_security_limits_are_stored_as_validated_types() {
    let _guard = ENV_LOCK.lock().unwrap();
    let original_file_size = env::var(ENV_MAX_SRC_FILE_SIZE).ok();
    let original_resolution = env::var(ENV_MAX_SRC_RESOLUTION).ok();

    env::set_var(ENV_MAX_SRC_FILE_SIZE, "4096");
    env::set_var(ENV_MAX_SRC_RESOLUTION, "2.5");
    let config = Config::from_env().expect("security limits are valid");

    assert_eq!(config.max_src_file_size.map(MaxSourceFileSize::get), Some(4096));
    assert_eq!(
        config.max_src_resolution.map(MaxSourceResolution::pixels),
        Some(2_500_000)
    );

    restore_env_var(ENV_MAX_SRC_FILE_SIZE, original_file_size);
    restore_env_var(ENV_MAX_SRC_RESOLUTION, original_resolution);
}
