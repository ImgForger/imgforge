use crate::processing::options::Resize;
use crate::processing::transform;
use libvips::VipsImage;

use super::tests_support::*;

#[test]
fn test_apply_resize_fit() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 200,
        height: 150,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 150);
}

#[test]
fn test_apply_resize_fill() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
    let resize = Resize {
        resizing_type: "fill".to_string(),
        width: 200,
        height: 200,
    };
    let resized_img = transform::apply_resize(img, &resize, &Some("center".to_string()), &None).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 200);
}

#[test]
fn test_apply_resize_fill_width_only() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
    let resize = Resize {
        resizing_type: "fill".to_string(),
        width: 200,
        height: 0,
    };
    let resized_img = transform::apply_resize(img, &resize, &Some("center".to_string()), &None).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 150);
}

#[test]
fn test_apply_resize_fill_height_only() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
    let resize = Resize {
        resizing_type: "fill".to_string(),
        width: 0,
        height: 150,
    };
    let resized_img = transform::apply_resize(img, &resize, &Some("center".to_string()), &None).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 150);
}

#[test]
fn test_apply_resize_force_width_only() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
    let resize = Resize {
        resizing_type: "force".to_string(),
        width: 200,
        height: 0,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 300);
}

#[test]
fn test_apply_resize_force_height_only() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
    let resize = Resize {
        resizing_type: "force".to_string(),
        width: 0,
        height: 150,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
    assert_eq!(resized_img.get_width(), 400);
    assert_eq!(resized_img.get_height(), 150);
}

#[test]
fn test_apply_resize_force_zero_dimensions_error() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
    let resize = Resize {
        resizing_type: "force".to_string(),
        width: 0,
        height: 0,
    };
    let result = transform::apply_resize(img, &resize, &None, &None);
    assert!(result.is_err());
}

#[test]
fn test_apply_resize_unknown_type_error() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
    let resize = Resize {
        resizing_type: "bogus".to_string(),
        width: 200,
        height: 100,
    };
    let result = transform::apply_resize(img, &resize, &None, &None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown resize type"));
}

#[test]
fn test_resolve_resize_dimensions_rejects_both_zero() {
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 0,
        height: 0,
    };
    let result = transform::resolve_resize_dimensions(&resize, 400, 300);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("at least one non-zero"));
}

#[test]
fn test_resolve_resize_dimensions_fills_missing_side_for_fit() {
    let resize_w_only = Resize {
        resizing_type: "fit".to_string(),
        width: 200,
        height: 0,
    };
    let dims = transform::resolve_resize_dimensions(&resize_w_only, 400, 300).unwrap();
    assert_eq!(dims, (200, 150));

    let resize_h_only = Resize {
        resizing_type: "fit".to_string(),
        width: 0,
        height: 150,
    };
    let dims = transform::resolve_resize_dimensions(&resize_h_only, 400, 300).unwrap();
    assert_eq!(dims, (200, 150));
}

#[test]
fn test_resolve_resize_dimensions_force_uses_source_for_missing_side() {
    let resize_w_only = Resize {
        resizing_type: "force".to_string(),
        width: 200,
        height: 0,
    };
    let dims = transform::resolve_resize_dimensions(&resize_w_only, 400, 300).unwrap();
    assert_eq!(dims, (200, 300));

    let resize_h_only = Resize {
        resizing_type: "force".to_string(),
        width: 0,
        height: 150,
    };
    let dims = transform::resolve_resize_dimensions(&resize_h_only, 400, 300).unwrap();
    assert_eq!(dims, (400, 150));
}

// Edge case tests
#[test]
fn test_resize_very_small_image() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(10, 10), "").unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 5,
        height: 5,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
    assert_eq!(resized_img.get_width(), 5);
    assert_eq!(resized_img.get_height(), 5);
}

#[test]
fn test_resize_extreme_scale_up() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(10, 10), "").unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 1000,
        height: 1000,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
    assert_eq!(resized_img.get_width(), 1000);
    assert_eq!(resized_img.get_height(), 1000);
}

#[test]
fn test_resize_extreme_aspect_ratio() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 100), "").unwrap();
    let resize = Resize {
        resizing_type: "fill".to_string(),
        width: 1000,
        height: 10,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, &None).unwrap();
    assert_eq!(resized_img.get_width(), 1000);
    assert_eq!(resized_img.get_height(), 10);
}

#[test]
fn test_resize_fill_with_different_gravities() {
    init_vips();
    for gravity in &["north", "south", "east", "west", "center"] {
        let img = VipsImage::new_from_buffer(&create_test_image(200, 100), "").unwrap();
        let resize = Resize {
            resizing_type: "fill".to_string(),
            width: 100,
            height: 100,
        };
        let resized = transform::apply_resize(img, &resize, &Some(gravity.to_string()), &None).unwrap();
        assert_eq!(resized.get_width(), 100);
        assert_eq!(resized.get_height(), 100);
    }
}

#[test]
fn test_resize_fill_with_lanczos2_kernel() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(800, 600), "").unwrap();
    let resize = Resize {
        resizing_type: "fill".to_string(),
        width: 300,
        height: 400,
    };
    let resized =
        transform::apply_resize(img, &resize, &Some("center".to_string()), &Some("lanczos2".to_string())).unwrap();
    assert_eq!(resized.get_width(), 300);
    assert_eq!(resized.get_height(), 400);
}

#[test]
fn test_resize_fit_with_nearest_kernel() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(800, 600), "").unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 300,
        height: 400,
    };
    let resized = transform::apply_resize(img, &resize, &None, &Some("nearest".to_string())).unwrap();
    assert_eq!(resized.get_width(), 300);
    assert_eq!(resized.get_height(), 225);
}

// Resize type tests
#[test]
fn test_resize_fit_width_only() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 100), "").unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 100,
        height: 0,
    };
    let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
    assert_eq!(resized.get_width(), 100);
    assert_eq!(resized.get_height(), 50);
}

#[test]
fn test_resize_fit_height_only() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 100), "").unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 0,
        height: 50,
    };
    let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
    assert_eq!(resized.get_width(), 100);
    assert_eq!(resized.get_height(), 50);
}

#[test]
fn test_resize_auto_portrait_to_portrait() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
    let resize = Resize {
        resizing_type: "auto".to_string(),
        width: 50,
        height: 100,
    };
    let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
    assert_eq!(resized.get_width(), 50);
    assert_eq!(resized.get_height(), 100);
}

#[test]
fn test_resize_auto_landscape_to_landscape() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 100), "").unwrap();
    let resize = Resize {
        resizing_type: "auto".to_string(),
        width: 100,
        height: 50,
    };
    let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
    assert_eq!(resized.get_width(), 100);
    assert_eq!(resized.get_height(), 50);
}

#[test]
fn test_resize_auto_portrait_to_landscape() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(100, 200), "").unwrap();
    let resize = Resize {
        resizing_type: "auto".to_string(),
        width: 150,
        height: 100,
    };
    let resized = transform::apply_resize(img, &resize, &None, &None).unwrap();
    // Uses fit mode when orientations differ, fitting within 150x100 while keeping aspect.
    assert_eq!(resized.get_width(), 50);
    assert_eq!(resized.get_height(), 100);
}

#[test]
fn test_apply_resize_with_cubic_algorithm() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 200,
        height: 150,
    };

    // Test with cubic - should also work
    let resized_img2 = transform::apply_resize(img, &resize, &None, &Some("cubic".to_string())).unwrap();
    assert_eq!(resized_img2.get_width(), 200);
    assert_eq!(resized_img2.get_height(), 150);
}

#[test]
fn test_apply_resize_with_invalid_kernel_falls_back_to_default() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(400, 300), "").unwrap();
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 200,
        height: 150,
    };

    let resized_img = transform::apply_resize(img, &resize, &None, &Some("not-a-kernel".to_string())).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 150);
}
