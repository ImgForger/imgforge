use crate::processing::options::{Crop, ParsedOptions, Resize, Watermark};
use crate::processing::process_image;
use crate::processing::transform;
use crate::processing::watermark;
use bytes::Bytes;
use image::GenericImageView;
use libvips::VipsImage;

use super::tests_support::*;

#[test]
fn test_crop_then_resize() {
    init_vips();
    let img = image_from(create_test_image(400, 400));
    let crop = Crop {
        x: 50,
        y: 50,
        width: 200,
        height: 200,
        gravity: None,
    };
    let cropped = transform::crop_image(img, crop).unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 100,
        height: 100,
    };
    let final_img = transform::apply_resize(cropped, &resize, &None, None, true).unwrap();
    assert_eq!(final_img.get_width(), 100);
    assert_eq!(final_img.get_height(), 100);
}

#[test]
fn test_resize_then_blur() {
    init_vips();
    let img = image_from(create_test_image(200, 200));
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 100,
        height: 100,
    };
    let resized = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    let blurred = transform::apply_blur(resized, 3.0).unwrap();
    assert_eq!(blurred.get_width(), 100);
    assert_eq!(blurred.get_height(), 100);
}

#[test]
fn test_resize_then_sharpen() {
    init_vips();
    let img = image_from(create_test_image(200, 200));
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 300,
        height: 300,
    };
    let resized = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    let sharpened = transform::apply_sharpen(resized, 1.0).unwrap();
    assert_eq!(sharpened.get_width(), 300);
    assert_eq!(sharpened.get_height(), 300);
}

#[test]
fn test_rotation_then_resize() {
    init_vips();
    let img = image_from(create_test_image(100, 200));
    let rotated = transform::apply_rotation(img, 90).unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 100,
        height: 100,
    };
    let resized = transform::apply_resize(rotated, &resize, &None, None, true).unwrap();
    assert_eq!(resized.get_width(), 100);
    assert_eq!(resized.get_height(), 50);
}

#[test]
fn test_complex_pipeline_crop_resize_blur_rotate() {
    init_vips();
    let img = image_from(create_test_image(400, 400));

    let crop = Crop {
        x: 50,
        y: 50,
        width: 300,
        height: 300,
        gravity: None,
    };
    let img = transform::crop_image(img, crop).unwrap();
    assert_eq!(img.get_width(), 300);

    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 200,
        height: 200,
    };
    let img = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!(img.get_width(), 200);

    let img = transform::apply_blur(img, 2.0).unwrap();
    let img = transform::apply_rotation(img, 90).unwrap();
    assert_eq!(img.get_width(), 200);
    assert_eq!(img.get_height(), 200);
}

#[test]
fn test_complex_pipeline_resize_padding_watermark() {
    init_vips();
    let img = image_from(create_test_image(200, 200));

    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 150,
        height: 150,
    };
    let img = transform::apply_resize(img, &resize, &None, None, true).unwrap();

    let img = transform::apply_padding(img, 10, 10, 10, 10, &Some([255, 255, 255, 255])).unwrap();
    assert_eq!(img.get_width(), 170);
    assert_eq!(img.get_height(), 170);

    let watermark = cached_watermark_from_bytes(create_test_image(30, 30));
    let watermark_opts = Watermark {
        opacity: 0.7,
        position: "soea".to_string(),
    };
    let img = watermark::apply_watermark(img, &watermark, &watermark_opts, None).unwrap();
    assert_eq!(img.get_width(), 170);
}

#[test]
fn test_process_image_extend_uses_current_dimensions_after_min_height() {
    init_vips();
    let source_bytes = Bytes::from(create_test_image(200, 100));
    let img = VipsImage::new_from_buffer(&source_bytes, "").unwrap();
    let parsed_options = ParsedOptions {
        resize: Some(Resize {
            resizing_type: "fit".to_string(),
            width: 100,
            height: 200,
        }),
        format: Some("png".to_string()),
        enlarge: true,
        extend: true,
        min_height: Some(150),
        ..ParsedOptions::default()
    };

    let output = process_image(img, parsed_options, &source_bytes, None).unwrap();
    let decoded = image::load_from_memory(&output).unwrap();

    assert_eq!(decoded.dimensions(), (300, 200));
}

#[test]
fn test_max_result_dimension_rejects_oversized_output() {
    init_vips();
    // Nothing else bounds the requested output: a small source with
    // enlarge:true will happily be told to become enormous, and the source
    // guards only cover what was read in.
    let source_bytes = Bytes::from(create_test_image(64, 64));
    let img = VipsImage::new_from_buffer(&source_bytes, "").unwrap();
    let parsed_options = ParsedOptions {
        resize: Some(Resize {
            resizing_type: "fit".to_string(),
            width: 4000,
            height: 4000,
        }),
        format: Some("png".to_string()),
        enlarge: true,
        max_result_dimension: Some("2000".parse().unwrap()),
        ..ParsedOptions::default()
    };

    let err = process_image(img, parsed_options, &source_bytes, None).expect_err("should be rejected");
    let message = err.to_string();
    assert!(
        message.contains("4000") && message.contains("2000"),
        "error should name the result and the limit, got: {message}"
    );
}

#[test]
fn test_max_result_dimension_allows_output_within_limit() {
    init_vips();
    let source_bytes = Bytes::from(create_test_image(64, 64));
    let img = VipsImage::new_from_buffer(&source_bytes, "").unwrap();
    let parsed_options = ParsedOptions {
        resize: Some(Resize {
            resizing_type: "fit".to_string(),
            width: 500,
            height: 500,
        }),
        format: Some("png".to_string()),
        enlarge: true,
        max_result_dimension: Some("500".parse().unwrap()),
        ..ParsedOptions::default()
    };

    // The limit is inclusive: a result exactly at the ceiling is allowed.
    let output = process_image(img, parsed_options, &source_bytes, None).unwrap();
    let decoded = image::load_from_memory(&output).unwrap();
    assert_eq!(decoded.dimensions(), (500, 500));
}

/// The minimums are a floor, not a request: they upscale even with
/// `enlarge:false`, and they scale both axes by the same factor. Documented in
/// doc/5_processing_options.md, which previously claimed the opposite.
#[test]
fn test_min_dimensions_upscale_regardless_of_enlarge() {
    init_vips();
    let source_bytes = Bytes::from(create_test_image(100, 100));
    let img = VipsImage::new_from_buffer(&source_bytes, "").unwrap();
    let parsed_options = ParsedOptions {
        min_width: Some(500),
        format: Some("png".to_string()),
        enlarge: false,
        ..ParsedOptions::default()
    };

    let output = process_image(img, parsed_options, &source_bytes, None).unwrap();
    let decoded = image::load_from_memory(&output).unwrap();
    // Aspect preserved: min-width raises the height too.
    assert_eq!(decoded.dimensions(), (500, 500));
}

/// End to end: the request a user actually sends. A wide banner asked to fit a
/// square box must come back inside that box, not at full size.
#[test]
fn test_fit_inside_a_square_box_downscales_a_wide_source() {
    init_vips();
    let source_bytes = Bytes::from(create_test_image(1600, 400));
    let img = VipsImage::new_from_buffer(&source_bytes, "").unwrap();
    let parsed_options = ParsedOptions {
        resize: Some(Resize {
            resizing_type: "fit".to_string(),
            width: 500,
            height: 500,
        }),
        format: Some("png".to_string()),
        enlarge: false,
        ..ParsedOptions::default()
    };

    let output = process_image(img, parsed_options, &source_bytes, None).unwrap();
    assert_eq!(image::load_from_memory(&output).unwrap().dimensions(), (500, 125));
}
