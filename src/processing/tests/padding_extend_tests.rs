use crate::processing::transform;
use libvips::VipsImage;

use super::tests_support::*;

#[test]
fn test_extend_image() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let extended_img =
        transform::extend_image(img, 200, 200, &Some("center".to_string()), &Some([0, 0, 0, 0])).unwrap();
    assert_eq!(extended_img.get_width(), 200);
    assert_eq!(extended_img.get_height(), 200);
}

#[test]
fn test_apply_padding() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let padded_img = transform::apply_padding(img, 10, 20, 30, 40, &Some([0, 0, 0, 0])).unwrap();
    assert_eq!(padded_img.get_width(), 160);
    assert_eq!(padded_img.get_height(), 140);
}

#[test]
fn test_apply_padding_position_and_background_color() {
    init_vips();
    let source_bytes = create_quadrant_test_image(4, 4);
    let img = VipsImage::new_from_buffer(&source_bytes, "").unwrap();
    let padded = transform::apply_padding(img, 1, 2, 3, 4, &Some([255, 255, 255, 255])).unwrap();
    assert_eq!(padded.get_width(), 10);
    assert_eq!(padded.get_height(), 8);

    let decoded = decode_rgba(&padded);
    assert_eq!(rgba_pixel(&decoded, 0, 0), [255, 255, 255, 255]);
    assert_eq!(rgba_pixel(&decoded, 9, 7), [255, 255, 255, 255]);
    assert_eq!(rgba_pixel(&decoded, 4, 1), [255, 0, 0, 255]);
    assert_eq!(rgba_pixel(&decoded, 7, 1), [0, 255, 0, 255]);
    assert_eq!(rgba_pixel(&decoded, 4, 4), [0, 0, 255, 255]);
    assert_eq!(rgba_pixel(&decoded, 7, 4), [255, 255, 0, 255]);
}

#[test]
fn test_extend_image_background_and_gravity_positions() {
    init_vips();
    let cases = [
        ("center", 2, 2),
        ("north", 2, 0),
        ("south", 2, 4),
        ("east", 4, 2),
        ("west", 0, 2),
    ];

    for (gravity, origin_x, origin_y) in cases {
        let source_bytes = create_quadrant_test_image(4, 4);
        let img = VipsImage::new_from_buffer(&source_bytes, "").unwrap();
        let extended =
            transform::extend_image(img, 8, 8, &Some(gravity.to_string()), &Some([10, 20, 30, 255])).unwrap();
        assert_eq!(extended.get_width(), 8);
        assert_eq!(extended.get_height(), 8);

        let decoded = decode_rgba(&extended);
        assert_eq!(rgba_pixel(&decoded, origin_x, origin_y), [255, 0, 0, 255]);

        let bg_probe = match gravity {
            "north" => (0, 7),
            "south" => (0, 0),
            "east" => (0, 0),
            "west" => (7, 0),
            _ => (0, 0),
        };
        assert_eq!(rgba_pixel(&decoded, bg_probe.0, bg_probe.1), [10, 20, 30, 255]);
    }
}

#[test]
fn test_extend_image_returns_error_when_target_smaller_than_source() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 80), "").unwrap();
    let result = transform::extend_image(img, 90, 120, &Some("center".to_string()), &Some([0, 0, 0, 0]));
    assert!(result.is_err());
    assert!(
        result.unwrap_err().contains("must be at least source"),
        "unexpected error message for extend guard"
    );
}

#[test]
fn test_padding_with_background_color() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let padded = transform::apply_padding(img, 20, 30, 40, 50, &Some([255, 255, 255, 255])).unwrap();
    assert_eq!(padded.get_width(), 180);
    assert_eq!(padded.get_height(), 160);
}

#[test]
fn test_extend_with_different_gravities() {
    init_vips();
    for gravity in &["north", "south", "east", "west", "center"] {
        let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
        let extended = transform::extend_image(img, 200, 200, &Some(gravity.to_string()), &Some([0, 0, 0, 0])).unwrap();
        assert_eq!(extended.get_width(), 200);
        assert_eq!(extended.get_height(), 200);
    }
}
