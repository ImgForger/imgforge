//! Reading and validating environment variables.

use super::ConfigError;
use crate::limits::SecurityLimitError;
use std::env;
use std::str::FromStr;

/// Reads a variable, distinguishing "absent" from "present but not Unicode".
///
/// A variable that cannot be decoded is a configuration mistake, not an absent
/// setting, and silently treating it as absent would start the server with the
/// operator's intent quietly discarded.
pub(super) fn optional_var(name: &'static str) -> Result<Option<String>, ConfigError> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(source @ env::VarError::NotUnicode(_)) => Err(ConfigError::InvalidUnicode { name, source }),
    }
}

/// Reads a boolean setting, accepting the same spellings the URL options do.
pub(super) fn bool_var(name: &'static str, default: bool) -> Result<bool, ConfigError> {
    Ok(optional_var(name)?
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "t" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default))
}

/// Reads a setting parsed by `FromStr`, failing the startup on a bad value.
pub(super) fn parsed_var<T>(name: &'static str) -> Result<Option<T>, ConfigError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let Some(value) = optional_var(name)? else {
        return Ok(None);
    };
    value
        .trim()
        .parse()
        .map(Some)
        .map_err(|source: T::Err| ConfigError::InvalidValue {
            name,
            value: value.clone(),
            reason: source.to_string(),
        })
}

/// Reads a validated security limit.
pub(super) fn security_limit_var<T>(name: &'static str) -> Result<Option<T>, ConfigError>
where
    T: FromStr<Err = SecurityLimitError>,
{
    let Some(value) = optional_var(name)? else {
        return Ok(None);
    };
    value
        .parse()
        .map(Some)
        .map_err(|source| ConfigError::InvalidSecurityLimit { name, value, source })
}

/// Reads a comma-separated list, dropping empty entries.
pub(super) fn list_var(name: &'static str) -> Result<Option<Vec<String>>, ConfigError> {
    Ok(optional_var(name)?.map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string)
            .collect()
    }))
}
