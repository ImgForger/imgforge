use crate::processing::options::{Gravity, Resize};
use crate::processing::transform::{self, TransformError};

use super::tests_support::*;

#[test]
fn test_apply_resize_fit() {
    init_vips();
    let img = image_from(create_test_image(400, 300));
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 200,
        height: 150,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 150);
}

#[test]
fn test_apply_resize_fill() {
    init_vips();
    let img = image_from(create_test_image(400, 300));
    let resize = Resize {
        resizing_type: "fill".to_string(),
        width: 200,
        height: 200,
    };
    let resized_img = transform::apply_resize(img, &resize, &Some(Gravity::Center), None, true).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 200);
}

#[test]
fn test_apply_resize_fill_width_only() {
    init_vips();
    let img = image_from(create_test_image(400, 300));
    let resize = Resize {
        resizing_type: "fill".to_string(),
        width: 200,
        height: 0,
    };
    let resized_img = transform::apply_resize(img, &resize, &Some(Gravity::Center), None, true).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 150);
}

#[test]
fn test_apply_resize_fill_height_only() {
    init_vips();
    let img = image_from(create_test_image(400, 300));
    let resize = Resize {
        resizing_type: "fill".to_string(),
        width: 0,
        height: 150,
    };
    let resized_img = transform::apply_resize(img, &resize, &Some(Gravity::Center), None, true).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 150);
}

#[test]
fn test_apply_resize_force_width_only() {
    init_vips();
    let img = image_from(create_test_image(400, 300));
    let resize = Resize {
        resizing_type: "force".to_string(),
        width: 200,
        height: 0,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 300);
}

#[test]
fn test_apply_resize_force_height_only() {
    init_vips();
    let img = image_from(create_test_image(400, 300));
    let resize = Resize {
        resizing_type: "force".to_string(),
        width: 0,
        height: 150,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!(resized_img.get_width(), 400);
    assert_eq!(resized_img.get_height(), 150);
}

#[test]
fn test_apply_resize_force_zero_dimensions_error() {
    init_vips();
    let img = image_from(create_test_image(400, 300));
    let resize = Resize {
        resizing_type: "force".to_string(),
        width: 0,
        height: 0,
    };
    let result = transform::apply_resize(img, &resize, &None, None, true);
    assert!(result.is_err());
}

#[test]
fn test_apply_resize_unknown_type_error() {
    init_vips();
    let img = image_from(create_test_image(400, 300));
    let resize = Resize {
        resizing_type: "bogus".to_string(),
        width: 200,
        height: 100,
    };
    let result = transform::apply_resize(img, &resize, &None, None, true);
    assert!(matches!(
        result,
        Err(TransformError::InvalidArgument {
            operation: "resize",
            ref message,
        }) if message.contains("Unknown resize type")
    ));
}

#[test]
fn test_resolve_resize_dimensions_rejects_both_zero() {
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 0,
        height: 0,
    };
    let result = transform::resolve_resize_dimensions(&resize, 400, 300);
    assert!(matches!(
        result,
        Err(TransformError::InvalidArgument {
            operation: "resize",
            ref message,
        }) if message.contains("at least one non-zero")
    ));
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
    let img = image_from(create_test_image(10, 10));
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 5,
        height: 5,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!(resized_img.get_width(), 5);
    assert_eq!(resized_img.get_height(), 5);
}

#[test]
fn test_resize_extreme_scale_up() {
    init_vips();
    let img = image_from(create_test_image(10, 10));
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 1000,
        height: 1000,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!(resized_img.get_width(), 1000);
    assert_eq!(resized_img.get_height(), 1000);
}

#[test]
fn test_resize_extreme_aspect_ratio() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let resize = Resize {
        resizing_type: "fill".to_string(),
        width: 1000,
        height: 10,
    };
    let resized_img = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!(resized_img.get_width(), 1000);
    assert_eq!(resized_img.get_height(), 10);
}

#[test]
fn test_resize_fill_with_different_gravities() {
    init_vips();
    for gravity in [
        Gravity::North,
        Gravity::South,
        Gravity::East,
        Gravity::West,
        Gravity::Center,
    ] {
        let img = image_from(create_test_image(200, 100));
        let resize = Resize {
            resizing_type: "fill".to_string(),
            width: 100,
            height: 100,
        };
        let resized = transform::apply_resize(img, &resize, &Some(gravity), None, true).unwrap();
        assert_eq!(resized.get_width(), 100);
        assert_eq!(resized.get_height(), 100);
    }
}

#[test]
fn test_resize_fill_with_lanczos2_kernel() {
    init_vips();
    let img = image_from(create_test_image(800, 600));
    let resize = Resize {
        resizing_type: "fill".to_string(),
        width: 300,
        height: 400,
    };
    let resized = transform::apply_resize(img, &resize, &Some(Gravity::Center), Some("lanczos2"), true).unwrap();
    assert_eq!(resized.get_width(), 300);
    assert_eq!(resized.get_height(), 400);
}

#[test]
fn test_resize_fit_with_nearest_kernel() {
    init_vips();
    let img = image_from(create_test_image(800, 600));
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 300,
        height: 400,
    };
    let resized = transform::apply_resize(img, &resize, &None, Some("nearest"), true).unwrap();
    assert_eq!(resized.get_width(), 300);
    assert_eq!(resized.get_height(), 225);
}

// Resize type tests
#[test]
fn test_resize_fit_width_only() {
    init_vips();
    let img = image_from(create_test_image(200, 100));
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 100,
        height: 0,
    };
    let resized = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!(resized.get_width(), 100);
    assert_eq!(resized.get_height(), 50);
}

#[test]
fn test_resize_fit_height_only() {
    init_vips();
    let img = image_from(create_test_image(200, 100));
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 0,
        height: 50,
    };
    let resized = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!(resized.get_width(), 100);
    assert_eq!(resized.get_height(), 50);
}

#[test]
fn test_resize_auto_portrait_to_portrait() {
    init_vips();
    let img = image_from(create_test_image(100, 200));
    let resize = Resize {
        resizing_type: "auto".to_string(),
        width: 50,
        height: 100,
    };
    let resized = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!(resized.get_width(), 50);
    assert_eq!(resized.get_height(), 100);
}

#[test]
fn test_resize_auto_landscape_to_landscape() {
    init_vips();
    let img = image_from(create_test_image(200, 100));
    let resize = Resize {
        resizing_type: "auto".to_string(),
        width: 100,
        height: 50,
    };
    let resized = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!(resized.get_width(), 100);
    assert_eq!(resized.get_height(), 50);
}

#[test]
fn test_resize_auto_portrait_to_landscape() {
    init_vips();
    let img = image_from(create_test_image(100, 200));
    let resize = Resize {
        resizing_type: "auto".to_string(),
        width: 150,
        height: 100,
    };
    let resized = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    // Uses fit mode when orientations differ, fitting within 150x100 while keeping aspect.
    assert_eq!(resized.get_width(), 50);
    assert_eq!(resized.get_height(), 100);
}

#[test]
fn test_apply_resize_with_cubic_algorithm() {
    init_vips();
    let img = image_from(create_test_image(400, 300));
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 200,
        height: 150,
    };

    // Test with cubic - should also work
    let resized_img2 = transform::apply_resize(img, &resize, &None, Some("cubic"), true).unwrap();
    assert_eq!(resized_img2.get_width(), 200);
    assert_eq!(resized_img2.get_height(), 150);
}

#[test]
fn test_apply_resize_with_invalid_kernel_falls_back_to_default() {
    init_vips();
    let img = image_from(create_test_image(400, 300));
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 200,
        height: 150,
    };

    let resized_img = transform::apply_resize(img, &resize, &None, Some("not-a-kernel"), true).unwrap();
    assert_eq!(resized_img.get_width(), 200);
    assert_eq!(resized_img.get_height(), 150);
}

/// `enlarge:false` means "do not scale up", not "do not scale". These cases all
/// need only downscaling, and previously returned the untouched source because
/// the gate compared the requested box against the source and skipped the whole
/// operation when either side was larger.
///
/// Expected values follow imgproxy, which settles the resizing type first and
/// only then caps enlargement (processing/prepare.go).
#[test]
fn test_fit_downscales_even_when_the_box_is_taller_than_the_source() {
    init_vips();
    let cases = [
        // (src w, src h, target w, target h, expected w, expected h)
        (1000, 100, 500, 200, 500, 50),
        (1000, 100, 800, 800, 800, 80),
        (100, 1000, 2000, 50, 5, 50),
        // Control: box smaller on both axes, unchanged by the fix.
        (2000, 400, 300, 300, 300, 60),
    ];

    for (sw, sh, tw, th, ew, eh) in cases {
        let img = image_from(create_test_image(sw, sh));
        let resize = Resize {
            resizing_type: "fit".to_string(),
            width: tw,
            height: th,
        };
        let out = transform::apply_resize(img, &resize, &None, None, false).unwrap();
        assert_eq!(
            (out.get_width(), out.get_height()),
            (ew, eh),
            "fit {tw}x{th} on {sw}x{sh} with enlarge:false"
        );
    }
}

#[test]
fn test_fit_still_refuses_to_enlarge() {
    init_vips();
    // Both axes would grow: the cap leaves the image alone.
    let img = image_from(create_test_image(100, 100));
    let resize = Resize {
        resizing_type: "fit".to_string(),
        width: 500,
        height: 500,
    };
    let out = transform::apply_resize(img, &resize, &None, None, false).unwrap();
    assert_eq!((out.get_width(), out.get_height()), (100, 100));

    // ...and enlarges when asked to.
    let img = image_from(create_test_image(100, 100));
    let out = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!((out.get_width(), out.get_height()), (500, 500));
}

#[test]
fn test_fill_crops_to_what_is_available_when_capped() {
    init_vips();
    // Covering a 500x200 box from 1000x100 would need a 2x upscale. Capped, the
    // image is not scaled, and the crop takes what exists: 500x100, not an error.
    let img = image_from(create_test_image(1000, 100));
    let resize = Resize {
        resizing_type: "fill".to_string(),
        width: 500,
        height: 200,
    };
    let out = transform::apply_resize(img, &resize, &Some(Gravity::Center), None, false).unwrap();
    assert_eq!((out.get_width(), out.get_height()), (500, 100));

    // With enlargement allowed the box is filled exactly.
    let img = image_from(create_test_image(1000, 100));
    let out = transform::apply_resize(img, &resize, &Some(Gravity::Center), None, true).unwrap();
    assert_eq!((out.get_width(), out.get_height()), (500, 200));
}

#[test]
fn test_force_caps_enlargement_while_keeping_the_requested_distortion() {
    init_vips();
    // force:2000:50 on 1000x100 asks to double the width and halve the height.
    // Capping divides both scales by the largest, so the width lands at 1x
    // rather than 2x and the height keeps its relative proportion: 1000x25.
    // This is imgproxy's rule — it caps the scales together rather than
    // clamping each axis independently, which would change the aspect the
    // caller asked for.
    let img = image_from(create_test_image(1000, 100));
    let resize = Resize {
        resizing_type: "force".to_string(),
        width: 2000,
        height: 50,
    };
    let out = transform::apply_resize(img, &resize, &None, None, false).unwrap();
    assert_eq!((out.get_width(), out.get_height()), (1000, 25));

    let img = image_from(create_test_image(1000, 100));
    let out = transform::apply_resize(img, &resize, &None, None, true).unwrap();
    assert_eq!((out.get_width(), out.get_height()), (2000, 50));
}

/// libvips does not premultiply inside `vips_resize` — its own docs say the
/// caller must do it — so without this the kernel averages the colour of
/// invisible pixels into visible ones and transparent edges pick up a halo.
#[test]
fn test_resize_does_not_bleed_transparent_colour_into_visible_pixels() {
    init_vips();
    let img = image_from(create_transparent_edge_image(100, 100));
    let resize = Resize {
        resizing_type: "force".to_string(),
        width: 10,
        height: 10,
    };
    let out = transform::apply_resize(img, &resize, &None, None, false).unwrap();
    let decoded = decode_rgba(&out);

    // Across the whole boundary, partially transparent pixels must still be
    // white. Unpremultiplied resizing drags them toward the black that sits in
    // the transparent half.
    for y in 0..10u32 {
        for x in 0..10u32 {
            let [r, g, b, a] = rgba_pixel(&decoded, x, y);
            if a == 0 {
                continue; // fully transparent: colour is meaningless
            }
            assert!(
                r > 250 && g > 250 && b > 250,
                "pixel ({x},{y}) has alpha {a} but colour {r},{g},{b} — transparent black bled in"
            );
        }
    }
}

#[test]
fn test_resize_without_alpha_is_unaffected() {
    init_vips();
    // The premultiply round trip must not disturb opaque images.
    let img = image_from(create_test_image_jpeg(100, 100));
    let resize = Resize {
        resizing_type: "force".to_string(),
        width: 50,
        height: 50,
    };
    let out = transform::apply_resize(img, &resize, &None, None, false).unwrap();
    assert_eq!((out.get_width(), out.get_height()), (50, 50));
    let decoded = decode_rgba(&out);
    let [r, g, b, a] = rgba_pixel(&decoded, 25, 25);
    // JPEG is lossy, so red decodes near but not exactly 255.
    assert!(
        r > 250 && g < 5 && b < 5 && a == 255,
        "opaque red should survive unchanged, got {r},{g},{b},{a}"
    );
}
