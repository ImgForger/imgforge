use crate::processing::colorspace;
use crate::processing::options::{
    Adjust, Colorize, Crop, CropAspectRatio, Duotone, Flip, Gravity, GravityType, Monochrome, Resize, ResizingType,
    Trim, Zoom,
};
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
    let cropped_img = transform::crop_image(img, &crop, &Gravity::default(), None).unwrap();
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
    let cropped_img = transform::crop_image(img, &crop, &crop.gravity.unwrap_or_default(), None).unwrap();
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
    let cropped_img = transform::crop_image(img, &crop, &crop.gravity.unwrap_or_default(), None).unwrap();
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
            None,
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
        None,
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
            None,
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
        None,
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
        None,
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

/// Smart gravity asks libvips which part of the image a viewer would look at,
/// which is the one thing a geometric anchor cannot do. The subject here sits
/// in the bottom-right, so a centre crop misses it entirely and any fixed
/// anchor would only be right for images built like this one.
#[test]
fn test_smart_gravity_finds_the_subject_a_centre_crop_would_miss() {
    init_vips();
    let source = create_image_with_subject_at((120, 120), (86, 86, 24, 24));

    let smart = transform::crop_image(
        image_from(source.clone()),
        &Crop {
            width: 48.0,
            height: 48.0,
            gravity: Some(Gravity::new(GravityType::Smart)),
        },
        &Gravity::new(GravityType::Smart),
        None,
    )
    .unwrap();
    let centred = transform::crop_image(
        image_from(source),
        &Crop {
            width: 48.0,
            height: 48.0,
            gravity: None,
        },
        &Gravity::default(),
        None,
    )
    .unwrap();

    assert_eq!((smart.get_width(), smart.get_height()), (48, 48));

    // The subject is dark on a light field, so the crop that found it is the
    // darker one by a wide margin.
    let smart_luminance = mean_luminance(&decode_rgba(&smart));
    let centred_luminance = mean_luminance(&decode_rgba(&centred));
    assert!(
        smart_luminance < centred_luminance - 20.0,
        "smart crop should have found the subject: smart {smart_luminance:.1}, centred {centred_luminance:.1}"
    );
}

/// The same applies to the implicit crop a `fill` resize performs.
#[test]
fn test_smart_gravity_positions_the_fill_window() {
    init_vips();
    let source = create_image_with_subject_at((200, 100), (160, 30, 32, 40));

    let resize = Resize {
        resizing_type: ResizingType::Fill,
        width: 60,
        height: 60,
    };
    let smart = transform::apply_resize(
        image_from(source.clone()),
        &resize,
        &Gravity::new(GravityType::Smart),
        None,
        false,
        1.0,
    )
    .unwrap();
    let centred = transform::apply_resize(image_from(source), &resize, &Gravity::default(), None, false, 1.0).unwrap();

    assert_eq!((smart.get_width(), smart.get_height()), (60, 60));
    assert!(
        mean_luminance(&decode_rgba(&smart)) < mean_luminance(&decode_rgba(&centred)) - 10.0,
        "the fill window should have moved toward the subject"
    );
}

/// Monochrome keeps the image's tonal structure and loses only its hue, so a
/// red/green/blue quadrant image comes back with every quadrant a shade of the
/// base colour rather than its original hue.
#[test]
fn test_monochrome_reduces_every_hue_to_the_base_colour() {
    init_vips();
    let img = image_from(create_quadrant_test_image(40, 40));
    let toned = transform::apply_monochrome(
        img,
        Monochrome {
            intensity: 1.0,
            color: [0, 0, 255, 255],
        },
    )
    .unwrap();

    let decoded = decode_rgba(&toned);
    for (x, y) in [(5, 5), (35, 5), (5, 35), (35, 35)] {
        let [r, g, b, a] = rgba_pixel(&decoded, x, y);
        assert_eq!((r, g), (0, 0), "the base colour has no red or green at ({x}, {y})");
        assert_eq!(a, 255, "alpha must survive untouched");
        // Each quadrant had a different luminance, so each keeps a different
        // amount of blue — the point of a monochrome rather than a flat fill.
        assert!(b > 0, "a lit pixel should keep some of the base colour");
    }

    // An intensity of zero is a no-op rather than a full conversion.
    let untouched = transform::apply_monochrome(
        image_from(create_quadrant_test_image(40, 40)),
        Monochrome {
            intensity: 0.0,
            ..Monochrome::default()
        },
    )
    .unwrap();
    assert_eq!(rgba_pixel(&decode_rgba(&untouched), 5, 5), [255, 0, 0, 255]);
}

/// Duotone maps the darkest pixels to one colour and the brightest to another.
#[test]
fn test_duotone_maps_the_tonal_range_between_two_colours() {
    init_vips();
    // Black on the left, white on the right.
    let img = image_from(create_transparent_edge_image(40, 20));
    let toned = transform::apply_duotone(
        img,
        Duotone {
            intensity: 1.0,
            shadow: [255, 0, 0, 255],
            highlight: [0, 0, 255, 255],
        },
    )
    .unwrap();

    let decoded = decode_rgba(&toned);
    // The white half is the highlight colour; the transparent half carries
    // black RGB, so it lands on the shadow colour.
    let [lit_r, _, lit_b, _] = rgba_pixel(&decoded, 5, 10);
    let [dark_r, _, dark_b, _] = rgba_pixel(&decoded, 35, 10);
    assert!(lit_b > lit_r, "the bright side should reach the highlight colour");
    assert!(dark_r > dark_b, "the dark side should reach the shadow colour");
}

/// Colorize is a flat wash, so it ignores luminance entirely: every pixel moves
/// the same distance toward the colour.
#[test]
fn test_colorize_washes_a_flat_colour_over_the_image() {
    init_vips();
    let img = image_from(create_quadrant_test_image(40, 40));
    let washed = transform::apply_colorize(
        img,
        Colorize {
            opacity: 0.5,
            color: [0, 0, 0, 255],
            keep_alpha: true,
        },
    )
    .unwrap();

    let decoded = decode_rgba(&washed);
    // Half-way to black from full red.
    let [r, g, b, a] = rgba_pixel(&decoded, 5, 5);
    assert!((126..=129).contains(&r), "red should be halved, got {r}");
    assert_eq!((g, b), (0, 0));
    assert_eq!(a, 255, "keep_alpha should preserve the alpha channel");
}

/// `crop_aspect_ratio` reshapes the crop window without moving it, shrinking
/// the long axis by default and growing the short one when asked.
#[test]
fn test_crop_aspect_ratio_reshapes_the_window() {
    init_vips();
    let source = create_test_image(200, 200);

    let square = Crop {
        width: 100.0,
        height: 100.0,
        gravity: None,
    };

    // 2:1 by reduction takes the height down.
    let reduced = transform::crop_image(
        image_from(source.clone()),
        &square,
        &Gravity::default(),
        Some(CropAspectRatio {
            ratio: 2.0,
            enlarge: false,
        }),
    )
    .unwrap();
    assert_eq!((reduced.get_width(), reduced.get_height()), (100, 50));

    // The same ratio by enlargement takes the width up instead.
    let enlarged = transform::crop_image(
        image_from(source.clone()),
        &square,
        &Gravity::default(),
        Some(CropAspectRatio {
            ratio: 2.0,
            enlarge: true,
        }),
    )
    .unwrap();
    assert_eq!((enlarged.get_width(), enlarged.get_height()), (200, 100));

    // A ratio of zero means "leave it alone".
    let untouched = transform::crop_image(
        image_from(source),
        &square,
        &Gravity::default(),
        Some(CropAspectRatio {
            ratio: 0.0,
            enlarge: false,
        }),
    )
    .unwrap();
    assert_eq!((untouched.get_width(), untouched.get_height()), (100, 100));
}

/// A crop larger than the source is clamped to the source *before* the ratio is
/// corrected. Correcting first threw the correction away entirely whenever the
/// request exceeded the image.
#[test]
fn test_crop_aspect_ratio_survives_an_oversized_request() {
    init_vips();
    let img = image_from(create_test_image(100, 100));

    let cropped = transform::crop_image(
        img,
        &Crop {
            width: 1000.0,
            height: 1000.0,
            gravity: None,
        },
        &Gravity::default(),
        Some(CropAspectRatio {
            ratio: 2.0,
            enlarge: false,
        }),
    )
    .unwrap();

    assert_eq!(
        (cropped.get_width(), cropped.get_height()),
        (100, 50),
        "the 2:1 correction should apply to the clamped 100x100 window"
    );
}

/// The tone constants are written in 8-bit terms but have to be mixed into the
/// image's own range. On a 16-bit image an unscaled `ff0000` lands at 255 out
/// of 65535 — very nearly black rather than red.
#[test]
fn test_colorize_scales_its_colour_to_a_high_bit_depth_image() {
    init_vips();
    let eight_bit = image_from(create_test_image(8, 8));
    let sixteen_bit = ops::cast(&eight_bit, ops::BandFormat::Ushort).unwrap();
    // Bring the 0-255 values up to the 16-bit range so the source is genuinely
    // high bit depth rather than a 16-bit container holding 8-bit values.
    let sixteen_bit = ops::linear(&sixteen_bit, &mut [257.0; 4], &mut [0.0; 4]).unwrap();
    let sixteen_bit = ops::cast(&sixteen_bit, ops::BandFormat::Ushort).unwrap();

    let washed = transform::apply_colorize(
        sixteen_bit,
        Colorize {
            opacity: 1.0,
            color: [0, 0, 255, 255],
            keep_alpha: true,
        },
    )
    .unwrap();

    // Fully opaque blue, expressed in the image's own range.
    let blue = ops::extract_band(&washed, 2).unwrap();
    let mean = ops::avg(&blue).unwrap();
    assert!(
        mean > 60000.0,
        "expected the blue channel near the 16-bit ceiling, got {mean}"
    );
}

/// A signed 16-bit image spans 0-32767, not 0-65535. Grouping `Short` with
/// `Ushort` scaled mid-grey to roughly 32896, which the cast back clipped to
/// the format's own ceiling — so `colorize:1:808080` painted white instead of
/// grey, and a duotone's shadow colour clipped the same way.
#[test]
fn test_tone_constants_use_the_signed_range_for_short_images() {
    init_vips();
    let eight_bit = image_from(create_test_image(8, 8));
    let signed = ops::cast(&eight_bit, ops::BandFormat::Short).unwrap();
    // Scale the 0-255 values across the signed range, so this is genuinely a
    // signed high-bit-depth source rather than 8-bit values in a wider box.
    let signed = ops::linear(&signed, &mut [128.5; 4], &mut [0.0; 4]).unwrap();
    let signed = ops::cast(&signed, ops::BandFormat::Short).unwrap();

    let washed = transform::apply_colorize(
        signed,
        Colorize {
            opacity: 1.0,
            color: [128, 128, 128, 255],
            keep_alpha: true,
        },
    )
    .unwrap();

    // Half of 32767, give or take the rounding in `component`.
    let green = ops::extract_band(&washed, 1).unwrap();
    let mean = ops::avg(&green).unwrap();
    assert!(
        (15000.0..18000.0).contains(&mean),
        "mid-grey should land near half of the signed ceiling, got {mean}"
    );
    assert!(
        mean < 32000.0,
        "scaling against the unsigned ceiling clips mid-grey to white, got {mean}"
    );

    // The unsigned range is unaffected, so the earlier fix still holds.
    let unsigned = ops::cast(&image_from(create_test_image(8, 8)), ops::BandFormat::Ushort).unwrap();
    let unsigned = ops::linear(&unsigned, &mut [257.0; 4], &mut [0.0; 4]).unwrap();
    let unsigned = ops::cast(&unsigned, ops::BandFormat::Ushort).unwrap();
    let washed = transform::apply_colorize(
        unsigned,
        Colorize {
            opacity: 1.0,
            color: [128, 128, 128, 255],
            keep_alpha: true,
        },
    )
    .unwrap();
    let green = ops::extract_band(&washed, 1).unwrap();
    let mean = ops::avg(&green).unwrap();
    assert!(
        (31000.0..35000.0).contains(&mean),
        "mid-grey on a 16-bit unsigned image should land near half of 65535, got {mean}"
    );
}

/// `keep_alpha:false` asks the wash to reach the transparent parts too, which is
/// a blend toward the colour's own alpha — not a discard. Dropping the band made
/// opacity irrelevant to transparency: `colorize:0.01` turned an invisible pixel
/// fully solid instead of moving it one percent of the way.
#[test]
fn test_colorize_blends_alpha_rather_than_discarding_it() {
    init_vips();

    let washed_alpha = |opacity: f64, keep_alpha: bool| {
        // Fully transparent on the right half, so the alpha band has something
        // to move.
        let img = image_from(create_transparent_edge_image(8, 8));
        let out = transform::apply_colorize(
            img,
            Colorize {
                opacity,
                color: [255, 0, 0, 255],
                keep_alpha,
            },
        )
        .unwrap();
        assert!(out.image_hasalpha(), "the result must still carry an alpha channel");
        let alpha = ops::extract_band(&out, out.get_bands() - 1).unwrap();
        ops::avg(&alpha).unwrap()
    };

    // Half the image is transparent, so the untouched mean sits near half of 255.
    let untouched = washed_alpha(0.5, true);
    assert!(
        (120.0..=136.0).contains(&untouched),
        "keep_alpha:true must leave transparency exactly as it was, got {untouched}"
    );

    // A barely-there wash barely moves it.
    let faint = washed_alpha(0.01, false);
    assert!(
        faint < untouched + 5.0,
        "a 1% wash must not make a transparent pixel opaque, got {faint}"
    );
    assert!(
        faint > untouched,
        "but it must move alpha toward the colour's, got {faint} against {untouched}"
    );

    // A full wash reaches the colour's own alpha, which is opaque.
    let full = washed_alpha(1.0, false);
    assert!(
        full > 250.0,
        "a full wash should land on the colour's alpha, got {full}"
    );

    // And the progression is monotonic in between.
    let half = washed_alpha(0.5, false);
    assert!(
        faint < half && half < full,
        "alpha should track opacity: {faint} < {half} < {full}"
    );
}
