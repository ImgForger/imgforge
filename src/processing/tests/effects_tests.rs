use crate::processing::options::Crop;
use crate::processing::transform;
use libvips::VipsImage;

use super::tests_support::*;

#[test]
fn test_crop_image() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
    let crop = Crop {
        x: 10,
        y: 20,
        width: 100,
        height: 150,
    };
    let cropped_img = transform::crop_image(img, crop).unwrap();
    assert_eq!(cropped_img.get_width(), 100);
    assert_eq!(cropped_img.get_height(), 150);
}

#[test]
fn test_apply_rotation() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
    let rotated_img = transform::apply_rotation(img, 90).unwrap();
    assert_eq!(rotated_img.get_width(), 200);
    assert_eq!(rotated_img.get_height(), 100);
}

#[test]
fn test_apply_blur() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let blurred_img = transform::apply_blur(img, 5.0).unwrap();
    assert_eq!(blurred_img.get_width(), 100);
    assert_eq!(blurred_img.get_height(), 100);
}

#[test]
fn test_apply_background_color() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let bg_applied_img = transform::apply_background_color(img, [255, 0, 0, 255]).unwrap();
    assert_eq!(bg_applied_img.get_bands(), 3);
}

#[test]
fn test_apply_background_color_no_alpha() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image_jpeg(100, 100), "").unwrap();
    let bands_before = img.get_bands();
    let bg_applied_img = transform::apply_background_color(img, [255, 0, 0, 255]).unwrap();
    assert_eq!(bg_applied_img.get_bands(), bands_before);
}

#[test]
fn test_apply_min_dimensions() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let min_dims_img = transform::apply_min_dimensions(img, Some(200), Some(150), &None).unwrap();
    assert_eq!(min_dims_img.get_width(), 200);
    assert_eq!(min_dims_img.get_height(), 200); // Scales by max(2, 1.5) = 2
}

#[test]
fn test_apply_zoom() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let zoomed_img = transform::apply_zoom(img, 2.0, &None).unwrap();
    assert_eq!(zoomed_img.get_width(), 200);
    assert_eq!(zoomed_img.get_height(), 200);
}

#[test]
fn test_apply_sharpen() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let sharpened_img = transform::apply_sharpen(img, 0.5).unwrap();
    assert_eq!(sharpened_img.get_width(), 100);
    assert_eq!(sharpened_img.get_height(), 100);
}

#[test]
fn test_apply_pixelate() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let pixelated_img = transform::apply_pixelate(img, 10, &None).unwrap();
    assert_eq!(pixelated_img.get_width(), 100);
    assert_eq!(pixelated_img.get_height(), 100);
}

#[test]
fn test_crop_at_edge() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let crop = Crop {
        x: 0,
        y: 0,
        width: 50,
        height: 50,
    };
    let cropped_img = transform::crop_image(img, crop).unwrap();
    assert_eq!(cropped_img.get_width(), 50);
    assert_eq!(cropped_img.get_height(), 50);
}

#[test]
fn test_crop_bottom_right_corner() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let crop = Crop {
        x: 50,
        y: 50,
        width: 50,
        height: 50,
    };
    let cropped_img = transform::crop_image(img, crop).unwrap();
    assert_eq!(cropped_img.get_width(), 50);
    assert_eq!(cropped_img.get_height(), 50);
}

#[test]
fn test_rotation_on_non_square() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(150, 100), "").unwrap();
    let rotated_img = transform::apply_rotation(img, 90).unwrap();
    assert_eq!(rotated_img.get_width(), 100);
    assert_eq!(rotated_img.get_height(), 150);
}

#[test]
fn test_rotation_180_degrees() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
    let rotated_img = transform::apply_rotation(img, 180).unwrap();
    assert_eq!(rotated_img.get_width(), 100);
    assert_eq!(rotated_img.get_height(), 200);
}

#[test]
fn test_rotation_270_degrees() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
    let rotated_img = transform::apply_rotation(img, 270).unwrap();
    assert_eq!(rotated_img.get_width(), 200);
    assert_eq!(rotated_img.get_height(), 100);
}

#[test]
fn test_rotation_unsupported_angle() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let rotated_img = transform::apply_rotation(img, 45).unwrap();
    // Should return original image unchanged
    assert_eq!(rotated_img.get_width(), 100);
    assert_eq!(rotated_img.get_height(), 100);
}

#[test]
fn test_pixelate_zero() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let original_width = img.get_width();
    let pixelated_img = transform::apply_pixelate(img, 0, &None).unwrap();
    assert_eq!(pixelated_img.get_width(), original_width);
}

#[test]
fn test_pixelate_small_amount() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let pixelated_img = transform::apply_pixelate(img, 1, &None).unwrap();
    assert_eq!(pixelated_img.get_width(), 100);
}

#[test]
fn test_pixelate_large_amount() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
    let pixelated_img = transform::apply_pixelate(img, 50, &None).unwrap();
    assert_eq!(pixelated_img.get_width(), 200);
    assert_eq!(pixelated_img.get_height(), 200);
}

// Min dimensions tests
#[test]
fn test_apply_min_width_only() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let result = transform::apply_min_dimensions(img, Some(200), None, &None).unwrap();
    assert_eq!(result.get_width(), 200);
    assert_eq!(result.get_height(), 200);
}

#[test]
fn test_apply_min_height_only() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let result = transform::apply_min_dimensions(img, None, Some(150), &None).unwrap();
    assert_eq!(result.get_width(), 150);
    assert_eq!(result.get_height(), 150);
}

#[test]
fn test_apply_min_dimensions_already_larger() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
    let result = transform::apply_min_dimensions(img, Some(100), Some(100), &None).unwrap();
    assert_eq!(result.get_width(), 200);
    assert_eq!(result.get_height(), 200);
}

#[test]
fn test_apply_zoom_scale_down() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
    let zoomed = transform::apply_zoom(img, 0.5, &None).unwrap();
    assert_eq!(zoomed.get_width(), 100);
    assert_eq!(zoomed.get_height(), 100);
}

#[test]
fn test_apply_zoom_scale_up() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let zoomed = transform::apply_zoom(img, 3.0, &None).unwrap();
    assert_eq!(zoomed.get_width(), 300);
    assert_eq!(zoomed.get_height(), 300);
}

// Blur edge cases
#[test]
fn test_apply_blur_minimal() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let blurred = transform::apply_blur(img, 0.1).unwrap();
    assert_eq!(blurred.get_width(), 100);
    assert_eq!(blurred.get_height(), 100);
}

#[test]
fn test_apply_blur_extreme() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let blurred = transform::apply_blur(img, 50.0).unwrap();
    assert_eq!(blurred.get_width(), 100);
    assert_eq!(blurred.get_height(), 100);
}

// Sharpen edge cases
#[test]
fn test_apply_sharpen_minimal() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let sharpened = transform::apply_sharpen(img, 0.1).unwrap();
    assert_eq!(sharpened.get_width(), 100);
    assert_eq!(sharpened.get_height(), 100);
}

#[test]
fn test_apply_sharpen_extreme() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let sharpened = transform::apply_sharpen(img, 10.0).unwrap();
    assert_eq!(sharpened.get_width(), 100);
    assert_eq!(sharpened.get_height(), 100);
}

#[test]
fn test_apply_sharpen_clamps_sigma() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(50, 50), "").unwrap();
    let sharpened = transform::apply_sharpen(img, 100.0).unwrap();
    assert_eq!(sharpened.get_width(), 50);
    assert_eq!(sharpened.get_height(), 50);
}

// Background color tests
#[test]
fn test_apply_background_color_with_transparency() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let result = transform::apply_background_color(img, [255, 255, 255, 255]).unwrap();
    // Should flatten to 3 bands (RGB)
    assert_eq!(result.get_bands(), 3);
}
