use crate::processing::options::ProcessingOption;
use std::collections::HashMap;
use tracing::debug;

const PRESET: &str = "preset";
const PRESET_SHORT: &str = "pr";

/// Expands preset references in processing options.
///
/// This function takes a list of processing options and expands any preset references
/// by looking them up in the presets map and replacing them with the preset's options.
/// If a "default" preset exists, it is applied first.
///
/// # Arguments
///
/// * `options` - The original processing options from the URL
/// * `presets` - Map of preset name to preset options string
/// * `only_presets` - If true, only preset references are allowed
///
/// # Returns
///
/// A `Result` containing the expanded processing options, or an error message.
pub fn expand_presets(
    options: Vec<ProcessingOption>,
    presets: &HashMap<String, String>,
    only_presets: bool,
) -> Result<Vec<ProcessingOption>, String> {
    let mut expanded = Vec::new();
    let mut has_preset_reference = false;

    // First, apply the default preset if it exists
    if let Some(default_options) = presets.get("default") {
        debug!("Applying default preset: {}", default_options);
        let default_opts = parse_options_string(default_options)?;
        expanded.extend(default_opts);
    }

    // Then process the URL options
    for option in options {
        if option.name == PRESET || option.name == PRESET_SHORT {
            has_preset_reference = true;
            if option.args.is_empty() {
                return Err("preset option requires a preset name".to_string());
            }
            let preset_name = &option.args[0];
            let preset_options = presets
                .get(preset_name)
                .ok_or_else(|| format!("unknown preset: {}", preset_name))?;
            debug!("Expanding preset '{}': {}", preset_name, preset_options);
            let preset_opts = parse_options_string(preset_options)?;
            expanded.extend(preset_opts);
        } else if only_presets {
            return Err(format!(
                "only preset references are allowed in only_presets mode, found: {}",
                option.name
            ));
        } else {
            expanded.push(option);
        }
    }

    // If only_presets is enabled and we have options but no preset reference,
    // and no default preset, reject the request
    if only_presets && !has_preset_reference && !presets.contains_key("default") && !expanded.is_empty() {
        return Err("only preset references are allowed in only_presets mode".to_string());
    }

    Ok(expanded)
}

/// Parses a preset options string into a vector of ProcessingOption.
///
/// Preset options are separated by '/' and follow the same format as URL options.
/// Example: "resize:fit:300:300/dpr:3/quality:85"
fn parse_options_string(options_str: &str) -> Result<Vec<ProcessingOption>, String> {
    let mut options = Vec::new();

    for part in options_str.split('/') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let mut segments = part.split(':');
        let name = segments
            .next()
            .ok_or_else(|| format!("invalid preset option: {}", part))?
            .to_string();
        let args: Vec<String> = segments.map(|s| s.to_string()).collect();

        options.push(ProcessingOption { name, args });
    }

    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_options_string() {
        let options_str = "resize:fit:300:300/dpr:3/quality:85";
        let options = parse_options_string(options_str).unwrap();

        assert_eq!(options.len(), 3);
        assert_eq!(options[0].name, "resize");
        assert_eq!(options[0].args, vec!["fit", "300", "300"]);
        assert_eq!(options[1].name, "dpr");
        assert_eq!(options[1].args, vec!["3"]);
        assert_eq!(options[2].name, "quality");
        assert_eq!(options[2].args, vec!["85"]);
    }

    #[test]
    fn test_parse_options_string_empty() {
        let options = parse_options_string("").unwrap();
        assert_eq!(options.len(), 0);
    }

    #[test]
    fn test_parse_options_string_trailing_slash() {
        let options_str = "resize:fit:300:300/quality:85/";
        let options = parse_options_string(options_str).unwrap();
        assert_eq!(options.len(), 2);
    }

    #[test]
    fn test_expand_presets_simple() {
        let mut presets = HashMap::new();
        presets.insert("thumbnail".to_string(), "resize:fit:150:150/quality:80".to_string());

        let options = vec![ProcessingOption {
            name: "preset".to_string(),
            args: vec!["thumbnail".to_string()],
        }];

        let expanded = expand_presets(options, &presets, false).unwrap();
        assert_eq!(expanded.len(), 2);
        assert_eq!(expanded[0].name, "resize");
        assert_eq!(expanded[1].name, "quality");
    }

    #[test]
    fn test_expand_presets_with_default() {
        let mut presets = HashMap::new();
        presets.insert("default".to_string(), "quality:90".to_string());
        presets.insert("thumbnail".to_string(), "resize:fit:150:150".to_string());

        let options = vec![ProcessingOption {
            name: "preset".to_string(),
            args: vec!["thumbnail".to_string()],
        }];

        let expanded = expand_presets(options, &presets, false).unwrap();
        assert_eq!(expanded.len(), 2);
        assert_eq!(expanded[0].name, "quality");
        assert_eq!(expanded[1].name, "resize");
    }

    #[test]
    fn test_expand_presets_default_only() {
        let mut presets = HashMap::new();
        presets.insert("default".to_string(), "quality:90/dpr:2".to_string());

        let options = vec![ProcessingOption {
            name: "blur".to_string(),
            args: vec!["5".to_string()],
        }];

        let expanded = expand_presets(options, &presets, false).unwrap();
        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].name, "quality");
        assert_eq!(expanded[1].name, "dpr");
        assert_eq!(expanded[2].name, "blur");
    }

    #[test]
    fn test_expand_presets_unknown_preset() {
        let presets = HashMap::new();
        let options = vec![ProcessingOption {
            name: "preset".to_string(),
            args: vec!["unknown".to_string()],
        }];

        let result = expand_presets(options, &presets, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown preset"));
    }

    #[test]
    fn test_expand_presets_only_presets_mode_allows_presets() {
        let mut presets = HashMap::new();
        presets.insert("thumbnail".to_string(), "resize:fit:150:150".to_string());

        let options = vec![ProcessingOption {
            name: "preset".to_string(),
            args: vec!["thumbnail".to_string()],
        }];

        let expanded = expand_presets(options, &presets, true).unwrap();
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0].name, "resize");
    }

    #[test]
    fn test_expand_presets_only_presets_mode_rejects_non_presets() {
        let presets = HashMap::new();
        let options = vec![ProcessingOption {
            name: "blur".to_string(),
            args: vec!["5".to_string()],
        }];

        let result = expand_presets(options, &presets, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("only preset references are allowed"));
    }

    #[test]
    fn test_expand_presets_only_presets_mode_allows_default() {
        let mut presets = HashMap::new();
        presets.insert("default".to_string(), "quality:90".to_string());

        let options = vec![];

        let expanded = expand_presets(options, &presets, true).unwrap();
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0].name, "quality");
    }

    #[test]
    fn test_expand_presets_preset_short() {
        let mut presets = HashMap::new();
        presets.insert("thumb".to_string(), "resize:fit:100:100".to_string());

        let options = vec![ProcessingOption {
            name: "pr".to_string(),
            args: vec!["thumb".to_string()],
        }];

        let expanded = expand_presets(options, &presets, false).unwrap();
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0].name, "resize");
    }

    #[test]
    fn test_expand_presets_empty_preset_name() {
        let presets = HashMap::new();
        let options = vec![ProcessingOption {
            name: "preset".to_string(),
            args: vec![],
        }];

        let result = expand_presets(options, &presets, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires a preset name"));
    }
}
