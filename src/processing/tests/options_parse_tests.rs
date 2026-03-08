use crate::processing::options::{parse_all_options, ProcessingOption};
use crate::processing::utils;

#[test]
fn test_parse_all_options_empty() {
    let options = vec![];
    let parsed = parse_all_options(options).unwrap();
    assert!(parsed.resize.is_none());
    assert!(parsed.blur.is_none());
    assert!(parsed.crop.is_none());
}

#[test]
fn test_parse_resize_option() {
    let options = vec![ProcessingOption {
        name: "resize".to_string(),
        args: vec!["fill".to_string(), "300".to_string(), "200".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    let resize = parsed.resize.unwrap();
    assert_eq!(resize.resizing_type, "fill");
    assert_eq!(resize.width, 300);
    assert_eq!(resize.height, 200);
}

#[test]
fn test_parse_quality_option() {
    let options = vec![ProcessingOption {
        name: "quality".to_string(),
        args: vec!["90".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.quality, Some(90));
}

#[test]
fn test_parse_blur_option() {
    let options = vec![ProcessingOption {
        name: "blur".to_string(),
        args: vec!["5".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.blur, Some(5.0));
}

#[test]
fn test_parse_background_option() {
    let options = vec![ProcessingOption {
        name: "background".to_string(),
        args: vec!["ff0000".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.background, Some([255, 0, 0, 255]));
}

#[test]
fn test_parse_padding_option() {
    let options = vec![ProcessingOption {
        name: "padding".to_string(),
        args: vec!["10".to_string(), "20".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.padding, Some((10, 20, 10, 20)));
}

#[test]
fn test_parse_rotation_option() {
    let options = vec![ProcessingOption {
        name: "rotate".to_string(),
        args: vec!["90".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.rotation, Some(90));
}

#[test]
fn test_parse_enlarge_option() {
    let options = vec![ProcessingOption {
        name: "enlarge".to_string(),
        args: vec!["true".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert!(parsed.enlarge);
}

#[test]
fn test_parse_extend_option() {
    let options = vec![ProcessingOption {
        name: "extend".to_string(),
        args: vec!["1".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert!(parsed.extend);
}

#[test]
fn test_parse_gravity_option() {
    let options = vec![ProcessingOption {
        name: "gravity".to_string(),
        args: vec!["north".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.gravity, Some("north".to_string()));
}

#[test]
fn test_parse_crop_option() {
    let options = vec![ProcessingOption {
        name: "crop".to_string(),
        args: vec!["10".to_string(), "20".to_string(), "100".to_string(), "150".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    let crop = parsed.crop.unwrap();
    assert_eq!(crop.x, 10);
    assert_eq!(crop.y, 20);
    assert_eq!(crop.width, 100);
    assert_eq!(crop.height, 150);
}

#[test]
fn test_parse_format_option() {
    let options = vec![ProcessingOption {
        name: "format".to_string(),
        args: vec!["webp".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.format, Some("webp".to_string()));
}

#[test]
fn test_parse_dpr_option() {
    let options = vec![ProcessingOption {
        name: "dpr".to_string(),
        args: vec!["2.5".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.dpr, Some(2.5));
}

#[test]
fn test_parse_auto_rotate_option() {
    let options = vec![ProcessingOption {
        name: "auto_rotate".to_string(),
        args: vec!["false".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert!(!parsed.auto_rotate);
}

#[test]
fn test_parse_raw_option() {
    let options = vec![ProcessingOption {
        name: "raw".to_string(),
        args: vec![],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert!(parsed.raw);
}

#[test]
fn test_parse_max_src_resolution_option() {
    let options = vec![ProcessingOption {
        name: "max_src_resolution".to_string(),
        args: vec!["10.5".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.max_src_resolution, Some(10.5));
}

#[test]
fn test_parse_max_src_file_size_option() {
    let options = vec![ProcessingOption {
        name: "max_src_file_size".to_string(),
        args: vec!["1024".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.max_src_file_size, Some(1024));
}

#[test]
fn test_parse_cache_buster_option() {
    let options = vec![ProcessingOption {
        name: "cache_buster".to_string(),
        args: vec!["12345".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.cache_buster, Some("12345".to_string()));
}

#[test]
fn test_parse_min_width_option() {
    let options = vec![ProcessingOption {
        name: "min_width".to_string(),
        args: vec!["500".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.min_width, Some(500));
}

#[test]
fn test_parse_min_height_option() {
    let options = vec![ProcessingOption {
        name: "min_height".to_string(),
        args: vec!["600".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.min_height, Some(600));
}

#[test]
fn test_parse_zoom_option() {
    let options = vec![ProcessingOption {
        name: "zoom".to_string(),
        args: vec!["1.5".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.zoom, Some(1.5));
}

#[test]
fn test_parse_sharpen_option() {
    let options = vec![ProcessingOption {
        name: "sharpen".to_string(),
        args: vec!["0.5".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.sharpen, Some(0.5));
}

#[test]
fn test_parse_pixelate_option() {
    let options = vec![ProcessingOption {
        name: "pixelate".to_string(),
        args: vec!["10".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.pixelate, Some(10));
}

#[test]
fn test_parse_watermark_option() {
    let options = vec![ProcessingOption {
        name: "watermark".to_string(),
        args: vec!["0.5".to_string(), "center".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    let watermark = parsed.watermark.unwrap();
    assert_eq!(watermark.opacity, 0.5);
    assert_eq!(watermark.position, "center");
}

// Error handling tests
#[test]
fn test_parse_resize_type_only() {
    let options = vec![ProcessingOption {
        name: "resize".to_string(),
        args: vec!["fill".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    let resize = parsed.resize.unwrap();
    assert_eq!(resize.resizing_type, "fill");
    assert_eq!(resize.width, 0);
    assert_eq!(resize.height, 0);
}

#[test]
fn test_parse_resize_meta_enlarge_extend() {
    let options = vec![ProcessingOption {
        name: "resize".to_string(),
        args: vec![
            "fit".to_string(),
            "640".to_string(),
            "480".to_string(),
            "true".to_string(),
            "true".to_string(),
        ],
    }];
    let parsed = parse_all_options(options).unwrap();
    let resize = parsed.resize.unwrap();
    assert_eq!(resize.resizing_type, "fit");
    assert_eq!(resize.width, 640);
    assert_eq!(resize.height, 480);
    assert!(parsed.enlarge);
    assert!(parsed.extend);
}

#[test]
fn test_parse_resize_meta_enlarge_only() {
    let options = vec![ProcessingOption {
        name: "resize".to_string(),
        args: vec!["".to_string(), "".to_string(), "".to_string(), "true".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert!(parsed.resize.is_none());
    assert!(parsed.enlarge);
    assert!(!parsed.extend);
}

#[test]
fn test_parse_resize_invalid_width() {
    let options = vec![ProcessingOption {
        name: "resize".to_string(),
        args: vec!["fill".to_string(), "abc".to_string(), "200".to_string()],
    }];
    assert!(parse_all_options(options).is_err());
}

#[test]
fn test_parse_background_invalid_hex() {
    let options = vec![ProcessingOption {
        name: "background".to_string(),
        args: vec!["gggggg".to_string()],
    }];
    assert!(parse_all_options(options).is_err());
}

#[test]
fn test_parse_background_short_hex() {
    let options = vec![ProcessingOption {
        name: "background".to_string(),
        args: vec!["fff".to_string()],
    }];
    assert!(parse_all_options(options).is_err());
}

#[test]
fn test_parse_quality_clamping() {
    let options = vec![ProcessingOption {
        name: "quality".to_string(),
        args: vec!["150".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.quality, Some(100));
}

#[test]
fn test_parse_quality_zero() {
    let options = vec![ProcessingOption {
        name: "quality".to_string(),
        args: vec!["0".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.quality, Some(1));
}

#[test]
fn test_parse_dpr_out_of_range() {
    let options = vec![ProcessingOption {
        name: "dpr".to_string(),
        args: vec!["10.0".to_string()],
    }];
    assert!(parse_all_options(options).is_err());
}

#[test]
fn test_parse_dpr_below_minimum() {
    let options = vec![ProcessingOption {
        name: "dpr".to_string(),
        args: vec!["0.5".to_string()],
    }];
    assert!(parse_all_options(options).is_err());
}

#[test]
fn test_parse_crop_invalid_args() {
    let options = vec![ProcessingOption {
        name: "crop".to_string(),
        args: vec!["10".to_string(), "20".to_string()],
    }];
    assert!(parse_all_options(options).is_err());
}

#[test]
fn test_parse_padding_invalid_count() {
    let options = vec![ProcessingOption {
        name: "padding".to_string(),
        args: vec!["10".to_string(), "20".to_string(), "30".to_string()],
    }];
    assert!(parse_all_options(options).is_err());
}

// Utils tests
#[test]
fn test_parse_hex_color_with_hash() {
    let color = utils::parse_hex_color("#ff0000").unwrap();
    assert_eq!(color, [255, 0, 0, 255]);
}

#[test]
fn test_parse_hex_color_without_hash() {
    let color = utils::parse_hex_color("00ff00").unwrap();
    assert_eq!(color, [0, 255, 0, 255]);
}

#[test]
fn test_parse_hex_color_invalid() {
    assert!(utils::parse_hex_color("gg0000").is_err());
}

#[test]
fn test_parse_hex_color_wrong_length() {
    assert!(utils::parse_hex_color("fff").is_err());
    assert!(utils::parse_hex_color("fffffff").is_err());
}

#[test]
fn test_parse_boolean_true_variants() {
    assert!(utils::parse_boolean("1"));
    assert!(utils::parse_boolean("true"));
}

#[test]
fn test_parse_boolean_false_variants() {
    assert!(!utils::parse_boolean("0"));
    assert!(!utils::parse_boolean("false"));
    assert!(!utils::parse_boolean(""));
    assert!(!utils::parse_boolean("yes"));
}

#[test]
fn test_is_portrait() {
    assert!(utils::is_portrait(100, 200));
    assert!(!utils::is_portrait(200, 100));
    assert!(!utils::is_portrait(100, 100));
}

// Shorthand option tests
#[test]
fn test_parse_resize_short() {
    let options = vec![ProcessingOption {
        name: "rs".to_string(),
        args: vec!["fill".to_string(), "300".to_string(), "200".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert!(parsed.resize.is_some());
}

#[test]
fn test_parse_quality_short() {
    let options = vec![ProcessingOption {
        name: "q".to_string(),
        args: vec!["80".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.quality, Some(80));
}

#[test]
fn test_parse_blur_short() {
    let options = vec![ProcessingOption {
        name: "bl".to_string(),
        args: vec!["3.5".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.blur, Some(3.5));
}

#[test]
fn test_parse_watermark_short() {
    let options = vec![ProcessingOption {
        name: "wm".to_string(),
        args: vec!["0.8".to_string(), "south".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert!(parsed.watermark.is_some());
}

// Combined options test
#[test]
fn test_parse_multiple_options() {
    let options = vec![
        ProcessingOption {
            name: "resize".to_string(),
            args: vec!["fill".to_string(), "300".to_string(), "200".to_string()],
        },
        ProcessingOption {
            name: "quality".to_string(),
            args: vec!["90".to_string()],
        },
        ProcessingOption {
            name: "blur".to_string(),
            args: vec!["2.0".to_string()],
        },
        ProcessingOption {
            name: "format".to_string(),
            args: vec!["webp".to_string()],
        },
    ];
    let parsed = parse_all_options(options).unwrap();
    assert!(parsed.resize.is_some());
    assert_eq!(parsed.quality, Some(90));
    assert_eq!(parsed.blur, Some(2.0));
    assert_eq!(parsed.format, Some("webp".to_string()));
}

// Size option test
#[test]
fn test_parse_size_option() {
    let options = vec![ProcessingOption {
        name: "size".to_string(),
        args: vec!["640".to_string(), "480".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert!(parsed.resize.is_some());
    let resize = parsed.resize.unwrap();
    assert_eq!(resize.resizing_type, "fit");
    assert_eq!(resize.width, 640);
    assert_eq!(resize.height, 480);
}

#[test]
fn test_parse_size_short() {
    let options = vec![ProcessingOption {
        name: "sz".to_string(),
        args: vec!["800".to_string(), "600".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert!(parsed.resize.is_some());
}

#[test]
fn test_parse_size_meta_full() {
    let options = vec![ProcessingOption {
        name: "size".to_string(),
        args: vec![
            "320".to_string(),
            "240".to_string(),
            "true".to_string(),
            "true".to_string(),
        ],
    }];
    let parsed = parse_all_options(options).unwrap();
    let resize = parsed.resize.unwrap();
    assert_eq!(resize.resizing_type, "fit");
    assert_eq!(resize.width, 320);
    assert_eq!(resize.height, 240);
    assert!(parsed.enlarge);
    assert!(parsed.extend);
}

#[test]
fn test_parse_size_meta_enlarge_only() {
    let options = vec![ProcessingOption {
        name: "size".to_string(),
        args: vec!["".to_string(), "".to_string(), "true".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert!(parsed.resize.is_none());
    assert!(parsed.enlarge);
    assert!(!parsed.extend);
}

#[test]
fn test_parse_size_short_alias_s() {
    let options = vec![ProcessingOption {
        name: "s".to_string(),
        args: vec![
            "1024".to_string(),
            "".to_string(),
            "true".to_string(),
            "true".to_string(),
        ],
    }];
    let parsed = parse_all_options(options).unwrap();
    let resize = parsed.resize.unwrap();
    assert_eq!(resize.resizing_type, "fit");
    assert_eq!(resize.width, 1024);
    assert_eq!(resize.height, 0);
    assert!(parsed.extend);
    assert!(parsed.enlarge);
}

#[test]
fn test_parse_width_default_zero() {
    let options = vec![ProcessingOption {
        name: "width".to_string(),
        args: vec![],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.width, Some(0));
    let resize = parsed.resize.unwrap();
    assert_eq!(resize.resizing_type, "fit");
    assert_eq!(resize.width, 0);
    assert_eq!(resize.height, 0);
}

#[test]
fn test_parse_width_blank_argument() {
    let options = vec![ProcessingOption {
        name: "width".to_string(),
        args: vec!["".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.width, Some(0));
}

#[test]
fn test_parse_width_short_blank_defaults() {
    let options = vec![ProcessingOption {
        name: "w".to_string(),
        args: vec![],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.width, Some(0));
}

#[test]
fn test_parse_height_default_zero() {
    let options = vec![ProcessingOption {
        name: "height".to_string(),
        args: vec![],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.height, Some(0));
}

#[test]
fn test_parse_height_blank_argument() {
    let options = vec![ProcessingOption {
        name: "h".to_string(),
        args: vec!["".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.height, Some(0));
}

#[test]
fn test_parse_resizing_algorithm_full() {
    let options = vec![ProcessingOption {
        name: "resizing_algorithm".to_string(),
        args: vec!["cubic".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.resizing_algorithm, Some("cubic".to_string()));
}

#[test]
fn test_parse_resizing_algorithm_short() {
    let options = vec![ProcessingOption {
        name: "ra".to_string(),
        args: vec!["linear".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.resizing_algorithm, Some("linear".to_string()));
}

#[test]
fn test_parse_resizing_algorithm_case_insensitive() {
    let options = vec![ProcessingOption {
        name: "ra".to_string(),
        args: vec!["LANCZOS3".to_string()],
    }];
    let parsed = parse_all_options(options).unwrap();
    assert_eq!(parsed.resizing_algorithm, Some("lanczos3".to_string()));
}

#[test]
fn test_parse_resizing_algorithm_invalid() {
    let options = vec![ProcessingOption {
        name: "resizing_algorithm".to_string(),
        args: vec!["invalid".to_string()],
    }];
    let result = parse_all_options(options);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid resizing algorithm"));
}
