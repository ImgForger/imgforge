use crate::processing::options::{Crop, Resize, Watermark};
use crate::processing::transform;
use crate::processing::watermark;
use libvips::VipsImage;

use super::tests_support::*;

#[test]
fn test_crop_then_resize() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 400), "").unwrap();
    let crop = Crop {
        x: 50,
        y: 50,
        width: 200,
        height: 200,
    };
    let cropped = transform::crop_image(img, crop).unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 100,
        height: 100,
    };
    let final_img = transform::apply_resize(cropped, &resize, &None, &None).unwrap();
    assert_eq!(final_img.get_width(), 100);
    assert_eq!(final_img.get_height(), 100);
}

#[test]
fn test_resize_then_blur() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 100,
        height: 100,
    };
    let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
    let blurred = transform::apply_blur(resized, 3.0).unwrap();
    assert_eq!(blurred.get_width(), 100);
    assert_eq!(blurred.get_height(), 100);
}

#[test]
fn test_resize_then_sharpen() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 300,
        height: 300,
    };
    let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
    let sharpened = transform::apply_sharpen(resized, 1.0).unwrap();
    assert_eq!(sharpened.get_width(), 300);
    assert_eq!(sharpened.get_height(), 300);
}

#[test]
fn test_rotation_then_resize() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
    let rotated = transform::apply_rotation(img, 90).unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 100,
        height: 100,
    };
    let resized = transform::apply_resize(rotated, &resize, &None, &None).unwrap();
    assert_eq!(resized.get_width(), 100);
    assert_eq!(resized.get_height(), 50);
}

#[test]
fn test_complex_pipeline_crop_resize_blur_rotate() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 400), "").unwrap();

    let crop = Crop {
        x: 50,
        y: 50,
        width: 300,
        height: 300,
    };
    let img = transform::crop_image(img, crop).unwrap();
    assert_eq!(img.get_width(), 300);

    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 200,
        height: 200,
    };
    let img = transform::apply_resize(img, &resize, &None, &None).unwrap();
    assert_eq!(img.get_width(), 200);

    let img = transform::apply_blur(img, 2.0).unwrap();
    let img = transform::apply_rotation(img, 90).unwrap();
    assert_eq!(img.get_width(), 200);
    assert_eq!(img.get_height(), 200);
}

#[test]
fn test_complex_pipeline_resize_padding_watermark() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();

    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 150,
        height: 150,
    };
    let img = transform::apply_resize(img, &resize, &None, &None).unwrap();

    let img = transform::apply_padding(img, 10, 10, 10, 10, &Some([255, 255, 255, 255])).unwrap();
    assert_eq!(img.get_width(), 170);
    assert_eq!(img.get_height(), 170);

    let watermark = cached_watermark_from_bytes(create_test_image(30, 30));
    let watermark_opts = Watermark {
        opacity: 0.7,
        position: "south_east".to_string(),
    };
    let img = watermark::apply_watermark(img, &watermark, &watermark_opts, &None).unwrap();
    assert_eq!(img.get_width(), 170);
}
