use crate::constants::ENV_WATERMARK_PATH;
use crate::processing::options::Watermark;
use crate::processing::watermark;
use libvips::VipsImage;
use std::io::Write;
use tempfile::NamedTempFile;

use super::tests_support::*;

#[test]
fn test_apply_watermark() {
    init_vips();
    let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
    let mut watermark_file = NamedTempFile::new().unwrap();
    watermark_file.write_all(&watermark.bytes).unwrap();
    std::env::set_var(ENV_WATERMARK_PATH, watermark_file.path());

    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
    let watermark_opts = Watermark {
        opacity: 0.5,
        position: "center".to_string(),
    };
    let watermarked_img = watermark::apply_watermark(img, &watermark, &watermark_opts, &None).unwrap();

    assert_eq!(watermarked_img.get_width(), 200);
    assert_eq!(watermarked_img.get_height(), 200);
    std::env::remove_var(ENV_WATERMARK_PATH);
}

#[test]
fn test_watermark_all_positions() {
    init_vips();
    let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
    let positions = vec![
        "north",
        "south",
        "east",
        "west",
        "center",
        "north_west",
        "north_east",
        "south_west",
        "south_east",
    ];

    for position in positions {
        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
        let watermark_opts = Watermark {
            opacity: 0.5,
            position: position.to_string(),
        };
        let watermarked = watermark::apply_watermark(img, &watermark, &watermark_opts, &None).unwrap();
        assert_eq!(watermarked.get_width(), 200);
        assert_eq!(watermarked.get_height(), 200);
    }
}

#[test]
fn test_watermark_full_opacity() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
    let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
    let watermark_opts = Watermark {
        opacity: 1.0,
        position: "center".to_string(),
    };
    let watermarked = watermark::apply_watermark(img, &watermark, &watermark_opts, &None).unwrap();
    assert_eq!(watermarked.get_width(), 200);
    assert_eq!(watermarked.get_height(), 200);
}

#[test]
fn test_watermark_zero_opacity() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
    let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
    let watermark_opts = Watermark {
        opacity: 0.0,
        position: "center".to_string(),
    };
    let watermarked = watermark::apply_watermark(img, &watermark, &watermark_opts, &None).unwrap();
    assert_eq!(watermarked.get_width(), 200);
    assert_eq!(watermarked.get_height(), 200);
}
