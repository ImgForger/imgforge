use crate::processing::options::Gravity;
use crate::processing::transform::{self, TransformError};
use libvips::VipsImage;

use super::tests_support::*;

#[test]
fn test_extend_image() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let extended_img = transform::extend_image(img, 200, 200, &Some(Gravity::Center), &Some([0, 0, 0, 0])).unwrap();
    assert_eq!(extended_img.get_width(), 200);
    assert_eq!(extended_img.get_height(), 200);
}

#[test]
fn test_apply_padding() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
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
        (Gravity::Center, 2, 2),
        (Gravity::North, 2, 0),
        (Gravity::South, 2, 4),
        (Gravity::East, 4, 2),
        (Gravity::West, 0, 2),
    ];

    for (gravity, origin_x, origin_y) in cases {
        let source_bytes = create_quadrant_test_image(4, 4);
        let img = VipsImage::new_from_buffer(&source_bytes, "").unwrap();
        let extended = transform::extend_image(img, 8, 8, &Some(gravity), &Some([10, 20, 30, 255])).unwrap();
        assert_eq!(extended.get_width(), 8);
        assert_eq!(extended.get_height(), 8);

        let decoded = decode_rgba(&extended);
        assert_eq!(rgba_pixel(&decoded, origin_x, origin_y), [255, 0, 0, 255]);

        let bg_probe = match gravity {
            Gravity::North => (0, 7),
            Gravity::South => (0, 0),
            Gravity::East => (0, 0),
            Gravity::West => (7, 0),
            _ => (0, 0),
        };
        assert_eq!(rgba_pixel(&decoded, bg_probe.0, bg_probe.1), [10, 20, 30, 255]);
    }
}

#[test]
fn test_extend_image_returns_error_when_target_smaller_than_source() {
    init_vips();
    let img = image_from(create_test_image(100, 80));
    let result = transform::extend_image(img, 90, 120, &Some(Gravity::Center), &Some([0, 0, 0, 0]));
    assert!(matches!(
        result,
        Err(TransformError::InvalidArgument {
            operation: "extend",
            ref message,
        }) if message.contains("must be at least source")
    ));
}

#[test]
fn test_padding_with_background_color() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let padded = transform::apply_padding(img, 20, 30, 40, 50, &Some([255, 255, 255, 255])).unwrap();
    assert_eq!(padded.get_width(), 180);
    assert_eq!(padded.get_height(), 160);
}

#[test]
fn test_extend_with_different_gravities() {
    init_vips();
    for gravity in [
        Gravity::North,
        Gravity::South,
        Gravity::East,
        Gravity::West,
        Gravity::Center,
    ] {
        let img = image_from(create_test_image(100, 100));
        let extended = transform::extend_image(img, 200, 200, &Some(gravity), &Some([0, 0, 0, 0])).unwrap();
        assert_eq!(extended.get_width(), 200);
        assert_eq!(extended.get_height(), 200);
    }
}

#[test]
fn test_padding_rejects_canvas_beyond_vips_limits() {
    init_vips();
    // Padding parses as an unbounded u32. Summing the canvas in i32 wrapped:
    // 4_294_967_268 is -28 as i32, so a 64-wide image came back 36 wide — a
    // silently cropped result served with a 200 — and debug builds panicked
    // on the overflow instead.
    let img = image_from(create_test_image(64, 64));
    let err = transform::apply_padding(img, 0, 4_294_967_268, 0, 0, &None)
        .expect_err("a canvas this size must be refused, not silently wrapped");

    assert!(
        matches!(
            err,
            TransformError::InvalidArgument {
                operation: "padding",
                ..
            }
        ),
        "expected an invalid-argument error, got: {err:?}"
    );
    assert!(
        err.to_string().contains("exceeds the maximum"),
        "error should name the limit, got: {err}"
    );
}

#[test]
fn test_padding_rejects_values_that_do_not_wrap_but_are_still_too_large() {
    init_vips();
    let img = image_from(create_test_image(64, 64));
    // Well inside u32 and inside i32, but far past what libvips will embed.
    let err = transform::apply_padding(img, 2_000_000_000, 0, 0, 0, &None).expect_err("must be refused");
    assert!(matches!(
        err,
        TransformError::InvalidArgument {
            operation: "padding",
            ..
        }
    ));
}

#[test]
fn test_padding_still_accepts_a_large_but_valid_canvas() {
    init_vips();
    // Just under the ceiling on one side: the guard must not reject usable work.
    let img = image_from(create_test_image(64, 64));
    let padded = transform::apply_padding(img, 0, 4_000, 0, 4_000, &None).expect("valid padding");
    assert_eq!(padded.get_width(), 8_064);
    assert_eq!(padded.get_height(), 64);
}
