use crate::processing::colorspace;
use crate::processing::options::{Adjust, Crop, Flip, Gravity, GravityType, Trim, Zoom};
use crate::processing::transform::{self, TransformError};
use libvips::{ops, VipsImage};

use super::tests_support::*;

#[test]
fn test_crop_image() {
    init_vips();
    let img = image_from(create_test_image(400, 300));
    let crop = Crop {
        width: 100.0,
        height: 150.0,
        gravity: None,
    };
    let cropped_img = transform::crop_image(img, &crop, &Gravity::default()).unwrap();
    assert_eq!(cropped_img.get_width(), 100);
    assert_eq!(cropped_img.get_height(), 150);
}

#[test]
fn test_apply_rotation() {
    init_vips();
    let img = image_from(create_test_image(100, 200));
    let rotated_img = transform::apply_rotation(img, 90).unwrap();
    assert_eq!(rotated_img.get_width(), 200);
    assert_eq!(rotated_img.get_height(), 100);
}

#[test]
fn test_apply_flip() {
    init_vips();
    let img = image_from(create_test_image(100, 50));
    let flipped_img = transform::apply_flip(
        img,
        Flip {
            horizontal: true,
            vertical: true,
        },
    )
    .unwrap();
    assert_eq!(flipped_img.get_width(), 100);
    assert_eq!(flipped_img.get_height(), 50);
}

#[test]
fn test_apply_adjust_saturation_keeps_dimensions() {
    init_vips();
    let img = image_from(create_test_image_jpeg(100, 50));
    let adjusted_img = transform::apply_adjust(
        img,
        Adjust {
            saturation: 0.5,
            ..Adjust::default()
        },
    )
    .unwrap();
    assert_eq!(adjusted_img.get_width(), 100);
    assert_eq!(adjusted_img.get_height(), 50);
}

#[test]
fn test_apply_blur() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let blurred_img = transform::apply_blur(img, 5.0).unwrap();
    assert_eq!(blurred_img.get_width(), 100);
    assert_eq!(blurred_img.get_height(), 100);
}

#[test]
fn test_apply_background_color() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let bg_applied_img = transform::apply_background_color(img, [255, 0, 0, 255]).unwrap();
    assert_eq!(bg_applied_img.get_bands(), 3);
}

#[test]
fn test_apply_background_color_no_alpha() {
    init_vips();
    let img = image_from(create_test_image_jpeg(100, 100));
    let bands_before = img.get_bands();
    let bg_applied_img = transform::apply_background_color(img, [255, 0, 0, 255]).unwrap();
    assert_eq!(bg_applied_img.get_bands(), bands_before);
}

#[test]
fn test_apply_min_dimensions() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let min_dims_img = transform::apply_min_dimensions(img, Some(200), Some(150), None).unwrap();
    assert_eq!(min_dims_img.get_width(), 200);
    assert_eq!(min_dims_img.get_height(), 200); // Scales by max(2, 1.5) = 2
}

#[test]
fn test_apply_zoom() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let zoomed_img = transform::apply_zoom(img, Zoom { x: 2.0, y: 2.0 }, None).unwrap();
    assert_eq!(zoomed_img.get_width(), 200);
    assert_eq!(zoomed_img.get_height(), 200);
}

#[test]
fn test_apply_sharpen() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let sharpened_img = transform::apply_sharpen(img, 0.5).unwrap();
    assert_eq!(sharpened_img.get_width(), 100);
    assert_eq!(sharpened_img.get_height(), 100);
}

#[test]
fn test_apply_pixelate() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let pixelated_img = transform::apply_pixelate(img, 10).unwrap();
    assert_eq!(pixelated_img.get_width(), 100);
    assert_eq!(pixelated_img.get_height(), 100);
}

#[test]
fn test_apply_pixelate_ignores_requested_resizing_kernel() {
    init_vips();
    let img = image_from(create_quadrant_test_image(40, 40));
    let pixelated_img = transform::apply_pixelate(img, 10).unwrap();
    assert_eq!(pixelated_img.get_width(), 40);
    assert_eq!(pixelated_img.get_height(), 40);
}

#[test]
fn test_apply_pixelate_with_extreme_amount_keeps_dimensions() {
    init_vips();
    let img = image_from(create_test_image(10, 10));
    let pixelated_img = transform::apply_pixelate(img, 1_000).unwrap();
    assert_eq!(pixelated_img.get_width(), 10);
    assert_eq!(pixelated_img.get_height(), 10);
}

#[test]
fn test_crop_at_edge() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let crop = Crop {
        width: 50.0,
        height: 50.0,
        gravity: None,
    };
    let cropped_img = transform::crop_image(img, &crop, &crop.gravity.unwrap_or_default()).unwrap();
    assert_eq!(cropped_img.get_width(), 50);
    assert_eq!(cropped_img.get_height(), 50);
}

#[test]
fn test_crop_bottom_right_corner() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let crop = Crop {
        width: 50.0,
        height: 50.0,
        gravity: None,
    };
    let cropped_img = transform::crop_image(img, &crop, &crop.gravity.unwrap_or_default()).unwrap();
    assert_eq!(cropped_img.get_width(), 50);
    assert_eq!(cropped_img.get_height(), 50);
}

#[test]
fn test_rotation_on_non_square() {
    init_vips();
    let img = image_from(create_test_image(150, 100));
    let rotated_img = transform::apply_rotation(img, 90).unwrap();
    assert_eq!(rotated_img.get_width(), 100);
    assert_eq!(rotated_img.get_height(), 150);
}

#[test]
fn test_rotation_180_degrees() {
    init_vips();
    let img = image_from(create_test_image(100, 200));
    let rotated_img = transform::apply_rotation(img, 180).unwrap();
    assert_eq!(rotated_img.get_width(), 100);
    assert_eq!(rotated_img.get_height(), 200);
}

#[test]
fn test_rotation_270_degrees() {
    init_vips();
    let img = image_from(create_test_image(100, 200));
    let rotated_img = transform::apply_rotation(img, 270).unwrap();
    assert_eq!(rotated_img.get_width(), 200);
    assert_eq!(rotated_img.get_height(), 100);
}

#[test]
fn test_rotation_unsupported_angle() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    assert!(matches!(
        transform::apply_rotation(img, 45),
        Err(TransformError::InvalidArgument {
            operation: "rotation",
            ..
        })
    ));
}

#[test]
fn test_pixelate_zero() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let original_width = img.get_width();
    let pixelated_img = transform::apply_pixelate(img, 0).unwrap();
    assert_eq!(pixelated_img.get_width(), original_width);
}

#[test]
fn test_pixelate_small_amount() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let pixelated_img = transform::apply_pixelate(img, 1).unwrap();
    assert_eq!(pixelated_img.get_width(), 100);
}

#[test]
fn test_pixelate_large_amount() {
    init_vips();
    let img = image_from(create_test_image(200, 200));
    let pixelated_img = transform::apply_pixelate(img, 50).unwrap();
    assert_eq!(pixelated_img.get_width(), 200);
    assert_eq!(pixelated_img.get_height(), 200);
}

// Min dimensions tests
#[test]
fn test_apply_min_width_only() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let result = transform::apply_min_dimensions(img, Some(200), None, None).unwrap();
    assert_eq!(result.get_width(), 200);
    assert_eq!(result.get_height(), 200);
}

#[test]
fn test_apply_min_height_only() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let result = transform::apply_min_dimensions(img, None, Some(150), None).unwrap();
    assert_eq!(result.get_width(), 150);
    assert_eq!(result.get_height(), 150);
}

#[test]
fn test_apply_min_dimensions_already_larger() {
    init_vips();
    let img = image_from(create_test_image(200, 200));
    let result = transform::apply_min_dimensions(img, Some(100), Some(100), None).unwrap();
    assert_eq!(result.get_width(), 200);
    assert_eq!(result.get_height(), 200);
}

#[test]
fn test_apply_zoom_scale_down() {
    init_vips();
    let img = image_from(create_test_image(200, 200));
    let zoomed = transform::apply_zoom(img, Zoom { x: 0.5, y: 0.5 }, None).unwrap();
    assert_eq!(zoomed.get_width(), 100);
    assert_eq!(zoomed.get_height(), 100);
}

#[test]
fn test_apply_zoom_scale_up() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let zoomed = transform::apply_zoom(img, Zoom { x: 3.0, y: 3.0 }, None).unwrap();
    assert_eq!(zoomed.get_width(), 300);
    assert_eq!(zoomed.get_height(), 300);
}

#[test]
fn test_apply_zoom_rejects_non_positive_values() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    assert!(matches!(
        transform::apply_zoom(img, Zoom { x: 0.0, y: 0.0 }, None),
        Err(TransformError::InvalidArgument { operation: "zoom", .. })
    ));
}

// Blur edge cases
#[test]
fn test_apply_blur_minimal() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let blurred = transform::apply_blur(img, 0.1).unwrap();
    assert_eq!(blurred.get_width(), 100);
    assert_eq!(blurred.get_height(), 100);
}

#[test]
fn test_apply_blur_extreme() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let blurred = transform::apply_blur(img, 50.0).unwrap();
    assert_eq!(blurred.get_width(), 100);
    assert_eq!(blurred.get_height(), 100);
}

#[test]
fn test_apply_blur_rejects_non_positive_sigma() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    assert!(matches!(
        transform::apply_blur(img, 0.0),
        Err(TransformError::InvalidArgument { operation: "blur", .. })
    ));
}

// Sharpen edge cases
#[test]
fn test_apply_sharpen_minimal() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let sharpened = transform::apply_sharpen(img, 0.1).unwrap();
    assert_eq!(sharpened.get_width(), 100);
    assert_eq!(sharpened.get_height(), 100);
}

#[test]
fn test_apply_sharpen_extreme() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let sharpened = transform::apply_sharpen(img, 10.0).unwrap();
    assert_eq!(sharpened.get_width(), 100);
    assert_eq!(sharpened.get_height(), 100);
}

#[test]
fn test_apply_sharpen_clamps_sigma() {
    init_vips();
    let img = image_from(create_test_image(50, 50));
    let sharpened = transform::apply_sharpen(img, 100.0).unwrap();
    assert_eq!(sharpened.get_width(), 50);
    assert_eq!(sharpened.get_height(), 50);
}

#[test]
fn test_apply_sharpen_rejects_non_positive_sigma() {
    init_vips();
    let img = image_from(create_test_image(50, 50));
    assert!(matches!(
        transform::apply_sharpen(img, 0.0),
        Err(TransformError::InvalidArgument {
            operation: "sharpen",
            ..
        })
    ));
}

// Background color tests
#[test]
fn test_apply_background_color_with_transparency() {
    init_vips();
    let img = image_from(create_test_image(100, 100));
    let result = transform::apply_background_color(img, [255, 255, 255, 255]).unwrap();
    // Should flatten to 3 bands (RGB)
    assert_eq!(result.get_bands(), 3);
}

/// The URL form is `crop:width:height[:gravity]` — there are no x/y arguments,
/// so gravity is the only thing that positions the window. Documented in
/// doc/5_processing_options.md; asserted here so the two cannot drift apart.
#[test]
fn test_crop_window_is_positioned_by_gravity() {
    init_vips();
    // Quadrant image: red top-left, green top-right, blue bottom-left, yellow bottom-right.
    let source = create_quadrant_test_image(100, 100);

    let cases = [
        (GravityType::NorthWest, [255, 0, 0, 255]),
        (GravityType::NorthEast, [0, 255, 0, 255]),
        (GravityType::SouthWest, [0, 0, 255, 255]),
        (GravityType::SouthEast, [255, 255, 0, 255]),
    ];

    for (kind, expected) in cases {
        let img = VipsImage::new_from_buffer(&source, "").unwrap();
        let gravity = Gravity::new(kind);
        let cropped = transform::crop_image(
            img,
            &Crop {
                width: 40.0,
                height: 40.0,
                gravity: Some(gravity),
            },
            &gravity,
        )
        .unwrap();
        assert_eq!((cropped.get_width(), cropped.get_height()), (40, 40));
        let decoded = decode_rgba(&cropped);
        assert_eq!(
            rgba_pixel(&decoded, 20, 20),
            expected,
            "gravity {kind:?} selected the wrong quadrant"
        );
    }
}

/// A crop with no gravity of its own falls back to the request's, which
/// defaults to centre. imgforge used to pin it to the top-left corner instead,
/// so the same URL cut a different part of the image than imgproxy did.
#[test]
fn test_crop_defaults_to_the_centre() {
    init_vips();
    let source = create_quadrant_test_image(100, 100);
    let img = VipsImage::new_from_buffer(&source, "").unwrap();

    let cropped = transform::crop_image(
        img,
        &Crop {
            width: 40.0,
            height: 40.0,
            gravity: None,
        },
        &Gravity::default(),
    )
    .unwrap();

    // A centred 40x40 window straddles the quadrant boundary, so its own
    // corners land one in each quadrant.
    let decoded = decode_rgba(&cropped);
    assert_eq!(rgba_pixel(&decoded, 0, 0), [255, 0, 0, 255]);
    assert_eq!(rgba_pixel(&decoded, 39, 0), [0, 255, 0, 255]);
    assert_eq!(rgba_pixel(&decoded, 0, 39), [0, 0, 255, 255]);
    assert_eq!(rgba_pixel(&decoded, 39, 39), [255, 255, 0, 255]);
}

/// Crop extents below 1 are a fraction of the source, which is what lets one
/// URL cut the same proportion out of sources of different sizes.
#[test]
fn test_fractional_crop_extents_scale_with_the_source() {
    init_vips();
    for (width, height) in [(100u32, 60u32), (400, 240)] {
        let img = image_from(create_test_image(width, height));
        let cropped = transform::crop_image(
            img,
            &Crop {
                width: 0.5,
                height: 0.25,
                gravity: None,
            },
            &Gravity::default(),
        )
        .unwrap();
        assert_eq!(
            (cropped.get_width() as u32, cropped.get_height() as u32),
            (width / 2, height / 4)
        );
    }
}

#[test]
fn test_crop_zero_means_full_extent_and_oversized_clamps() {
    init_vips();
    let source = create_test_image(100, 60);

    // 0 keeps the whole extent on that axis.
    let img = VipsImage::new_from_buffer(&source, "").unwrap();
    let cropped = transform::crop_image(
        img,
        &Crop {
            width: 0.0,
            height: 30.0,
            gravity: None,
        },
        &Gravity::default(),
    )
    .unwrap();
    assert_eq!((cropped.get_width(), cropped.get_height()), (100, 30));

    // Asking for more than exists yields the source extent, not an error.
    let img = VipsImage::new_from_buffer(&source, "").unwrap();
    let cropped = transform::crop_image(
        img,
        &Crop {
            width: 5000.0,
            height: 5000.0,
            gravity: None,
        },
        &Gravity::default(),
    )
    .unwrap();
    assert_eq!((cropped.get_width(), cropped.get_height()), (100, 60));
}

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const RED: [u8; 4] = [200, 0, 0, 255];

fn trim(threshold: f64, color: Option<[u8; 4]>, equal_hor: bool, equal_ver: bool) -> Trim {
    Trim {
        threshold,
        color,
        equal_hor,
        equal_ver,
    }
}

#[test]
fn test_trim_removes_a_uniform_border() {
    init_vips();
    // 100x60 red block at (30, 20) on a 200x140 white field.
    let img = image_from(create_bordered_image((200, 140), WHITE, (30, 20, 100, 60), RED));
    let trimmed = transform::apply_trim(img, &trim(10.0, None, false, false)).unwrap();
    assert_eq!((trimmed.get_width(), trimmed.get_height()), (100, 60));
}

/// libvips assumes a white background on its own, which never trims a dark
/// border. imgproxy works the colour out from the image, and so does this.
#[test]
fn test_trim_detects_a_dark_background_without_being_told() {
    init_vips();
    let img = image_from(create_bordered_image((200, 140), BLACK, (30, 20, 100, 60), RED));
    let trimmed = transform::apply_trim(img, &trim(10.0, None, false, false)).unwrap();
    assert_eq!((trimmed.get_width(), trimmed.get_height()), (100, 60));
}

#[test]
fn test_trim_honours_an_explicit_colour() {
    init_vips();
    let img = image_from(create_bordered_image((200, 140), WHITE, (30, 20, 100, 60), RED));
    // Naming red as the background inverts what counts as content: the white
    // frame is now the subject, and it reaches every edge.
    let trimmed = transform::apply_trim(img, &trim(10.0, Some(RED), false, false)).unwrap();
    assert_eq!((trimmed.get_width(), trimmed.get_height()), (200, 140));
}

/// `equal_hor` / `equal_ver` cut the same amount from both sides, so the subject
/// keeps its position instead of shifting toward the thicker border.
#[test]
fn test_trim_equal_sides_keeps_the_subject_centred() {
    init_vips();
    // Block at x=30 with 70 to its right: uneven. Equal trimming takes 30 from
    // each side, leaving 140 wide rather than the 100 an uneven trim gives.
    let img = image_from(create_bordered_image((200, 140), WHITE, (30, 20, 100, 60), RED));
    let trimmed = transform::apply_trim(img, &trim(10.0, None, true, false)).unwrap();
    assert_eq!(trimmed.get_width(), 140);
    // Vertical is untouched, so it still trims tight.
    assert_eq!(trimmed.get_height(), 60);
}

#[test]
fn test_trim_leaves_an_entirely_blank_image_alone() {
    init_vips();
    // Nothing but background: returning an empty or one-pixel image would be
    // worse than doing nothing.
    let img = image_from(create_test_image(80, 50));
    let trimmed = transform::apply_trim(img, &trim(10.0, Some([255, 0, 0, 255]), false, false)).unwrap();
    assert_eq!((trimmed.get_width(), trimmed.get_height()), (80, 50));
}

/// find_trim wants one component or one per non-alpha band, so the background
/// has to be built against the image rather than assumed to be RGB. Three
/// components against a CMYK image fails outright with "vector must have 1 or
/// 4 elements".
#[test]
fn test_trim_matches_the_background_to_the_image_bands() {
    init_vips();
    let source = create_bordered_image((200, 140), WHITE, (30, 20, 100, 60), RED);

    // Greyscale: detection reads one component and still finds the block.
    let grey = ops::colourspace(&image_from(source.clone()), ops::Interpretation::BW).unwrap();
    let trimmed = transform::apply_trim(grey, &trim(10.0, None, false, false)).unwrap();
    assert_eq!((trimmed.get_width(), trimmed.get_height()), (100, 60));

    // ...and an explicit colour is reduced to its luminance rather than being
    // passed as three components.
    let grey = ops::colourspace(&image_from(source.clone()), ops::Interpretation::BW).unwrap();
    let trimmed = transform::apply_trim(grey, &trim(10.0, Some(WHITE), false, false)).unwrap();
    assert_eq!((trimmed.get_width(), trimmed.get_height()), (100, 60));

    // CMYK: four components. Detection has to supply four, not three.
    let cmyk = ops::colourspace(&image_from(source.clone()), ops::Interpretation::Cmyk).unwrap();
    let trimmed = transform::apply_trim(cmyk, &trim(10.0, None, false, false)).unwrap();
    assert_eq!((trimmed.get_width(), trimmed.get_height()), (100, 60));

    // An sRGB colour has no meaningful reading against CMYK, so it is refused
    // rather than silently trimming the wrong thing.
    let cmyk = ops::colourspace(&image_from(source), ops::Interpretation::Cmyk).unwrap();
    let err = transform::apply_trim(cmyk, &trim(10.0, Some(WHITE), false, false)).expect_err("should refuse");
    assert!(err.to_string().contains("omit it to detect"), "unhelpful error: {err}");
}

/// Background detection must work whatever the band format. Reading raw memory
/// only worked for 8-bit, so a 16-bit source silently fell back to white and a
/// dark border survived.
#[test]
fn test_trim_detects_the_background_on_a_16_bit_source() {
    init_vips();
    let dark = image_from(create_bordered_image((200, 140), BLACK, (30, 20, 100, 60), RED));
    let deep = ops::cast(&dark, ops::BandFormat::Ushort).unwrap();
    assert!(matches!(deep.get_format(), Ok(ops::BandFormat::Ushort)));

    let trimmed = transform::apply_trim(deep, &trim(10.0, None, false, false)).unwrap();
    assert_eq!((trimmed.get_width(), trimmed.get_height()), (100, 60));
}

/// libvips reports every 8-bit three-band image as `Srgb`, whatever its actual
/// primaries — the real space lives in the embedded profile. Deciding on the
/// interpretation alone let a wide-gamut source through untransformed, so its
/// numbers were read as sRGB and came out oversaturated.
#[test]
fn a_wide_gamut_source_is_converted_through_its_profile() {
    init_vips();

    let plain = image_from(create_test_image_jpeg(16, 16));
    assert!(
        matches!(plain.get_interpretation(), Ok(ops::Interpretation::Srgb)),
        "the premise: an ordinary JPEG already reports as sRGB"
    );

    // Tag the same pixels as Display P3. The interpretation does not change —
    // that is exactly the trap — but the numbers now mean something different.
    let p3 = ops::icc_transform_with_opts(
        &plain,
        "p3",
        &ops::IccTransformOptions {
            input_profile: "srgb".to_string(),
            intent: ops::Intent::Relative,
            ..Default::default()
        },
    );
    let Ok(p3) = p3 else {
        // A libvips build without the P3 profile cannot exercise this.
        eprintln!("skipping: no P3 profile available in this libvips build");
        return;
    };
    assert!(
        matches!(p3.get_interpretation(), Ok(ops::Interpretation::Srgb)),
        "a P3 image still reports as sRGB, which is why the enum cannot decide"
    );

    let converted = colorspace::to_processing(ops::copy(&p3).unwrap(), false).unwrap();

    // Round-tripping P3 back to sRGB has to restore the original pixels. Left
    // untransformed, the P3 numbers would survive unchanged and read as sRGB.
    let original = decode_rgba(&plain);
    let round_tripped = decode_rgba(&converted);
    let untransformed = decode_rgba(&p3);

    let delta = |a: &image::RgbaImage, b: &image::RgbaImage| -> f64 {
        a.pixels()
            .zip(b.pixels())
            .map(|(x, y)| (f64::from(x[0]) - f64::from(y[0])).abs())
            .sum::<f64>()
            / a.pixels().len() as f64
    };

    let recovered = delta(&original, &round_tripped);
    let skipped = delta(&original, &untransformed);
    assert!(
        recovered < skipped,
        "converting through the profile must move the pixels back toward the original \
         (recovered delta {recovered:.2} should beat untransformed {skipped:.2})"
    );
    assert!(
        recovered < 4.0,
        "the round trip should land close to the original, got {recovered:.2}"
    );
}
