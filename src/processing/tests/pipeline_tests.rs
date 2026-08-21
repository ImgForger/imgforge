use crate::processing::options::{
    Crop, Extend, Gravity, GravityType, ParsedOptions, Resize, ResizingType, Watermark, WatermarkPosition, Zoom,
};
use crate::processing::process_image;
use crate::processing::save;
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
        width: 200.0,
        height: 200.0,
        gravity: None,
    };
    let cropped = transform::crop_image(img, &crop, &crop.gravity.unwrap_or_default()).unwrap();
    let resize = Resize {
        resizing_type: ResizingType::Fit,
        width: 100,
        height: 100,
    };
    let final_img = transform::apply_resize(cropped, &resize, &Gravity::default(), None, true, 1.0).unwrap();
    assert_eq!(final_img.get_width(), 100);
    assert_eq!(final_img.get_height(), 100);
}

#[test]
fn test_resize_then_blur() {
    init_vips();
    let img = image_from(create_test_image(200, 200));
    let resize = Resize {
        resizing_type: ResizingType::Fit,
        width: 100,
        height: 100,
    };
    let resized = transform::apply_resize(img, &resize, &Gravity::default(), None, true, 1.0).unwrap();
    let blurred = transform::apply_blur(resized, 3.0).unwrap();
    assert_eq!(blurred.get_width(), 100);
    assert_eq!(blurred.get_height(), 100);
}

#[test]
fn test_resize_then_sharpen() {
    init_vips();
    let img = image_from(create_test_image(200, 200));
    let resize = Resize {
        resizing_type: ResizingType::Fit,
        width: 300,
        height: 300,
    };
    let resized = transform::apply_resize(img, &resize, &Gravity::default(), None, true, 1.0).unwrap();
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
        resizing_type: ResizingType::Fit,
        width: 100,
        height: 100,
    };
    let resized = transform::apply_resize(rotated, &resize, &Gravity::default(), None, true, 1.0).unwrap();
    assert_eq!(resized.get_width(), 100);
    assert_eq!(resized.get_height(), 50);
}

#[test]
fn test_complex_pipeline_crop_resize_blur_rotate() {
    init_vips();
    let img = image_from(create_test_image(400, 400));

    let crop = Crop {
        width: 300.0,
        height: 300.0,
        gravity: None,
    };
    let img = transform::crop_image(img, &crop, &crop.gravity.unwrap_or_default()).unwrap();
    assert_eq!(img.get_width(), 300);

    let resize = Resize {
        resizing_type: ResizingType::Fit,
        width: 200,
        height: 200,
    };
    let img = transform::apply_resize(img, &resize, &Gravity::default(), None, true, 1.0).unwrap();
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
        resizing_type: ResizingType::Fit,
        width: 150,
        height: 150,
    };
    let img = transform::apply_resize(img, &resize, &Gravity::default(), None, true, 1.0).unwrap();

    let img = transform::apply_padding(img, 10, 10, 10, 10, &Some([255, 255, 255, 255])).unwrap();
    assert_eq!(img.get_width(), 170);
    assert_eq!(img.get_height(), 170);

    let watermark = cached_watermark_from_bytes(create_test_image(30, 30));
    let watermark_opts = Watermark {
        opacity: 0.7,
        position: WatermarkPosition::parse("soea").unwrap(),
        ..Watermark::default()
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
            resizing_type: ResizingType::Fit,
            width: 100,
            height: 200,
        }),
        format: Some("png".to_string()),
        enlarge: true,
        extend: Extend {
            enabled: true,
            gravity: None,
        },
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
            resizing_type: ResizingType::Fit,
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

/// The ceiling is policy and the encoder limit is fitting, and policy runs
/// first. Fitting first shrank a 20,000px result under WebP's 16,383px encoder
/// cap and the 18,000px ceiling then approved what it was configured to
/// refuse.
#[test]
fn test_max_result_dimension_is_checked_before_format_fitting() {
    init_vips();
    let source_bytes = Bytes::from(create_test_image(2000, 1));
    let img = VipsImage::new_from_buffer(&source_bytes, "").unwrap();
    let parsed_options = ParsedOptions {
        resize: Some(Resize {
            resizing_type: ResizingType::Force,
            width: 20000,
            height: 1,
        }),
        format: Some("webp".to_string()),
        enlarge: true,
        max_result_dimension: Some("18000".parse().unwrap()),
        ..ParsedOptions::default()
    };

    let err = process_image(img, parsed_options, &source_bytes, None).expect_err("the ceiling should still apply");
    let message = err.to_string();
    assert!(
        message.contains("20000") && message.contains("18000"),
        "error should name the unfitted result and the limit, got: {message}"
    );
}

#[test]
fn test_max_result_dimension_allows_output_within_limit() {
    init_vips();
    let source_bytes = Bytes::from(create_test_image(64, 64));
    let img = VipsImage::new_from_buffer(&source_bytes, "").unwrap();
    let parsed_options = ParsedOptions {
        resize: Some(Resize {
            resizing_type: ResizingType::Fit,
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
            resizing_type: ResizingType::Fit,
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

/// The shrink must never take the source below what the pipeline still needs.
/// Overshooting hands the resize a source smaller than the request, which
/// `enlarge:false` then refuses to scale back up — the output would silently
/// come out too small.
#[test]
fn test_load_shrink_never_undershoots_the_target() {
    use crate::processing::load_shrink_factor;

    let plan = |w: u32, h: u32| ParsedOptions {
        resize: Some(Resize {
            resizing_type: ResizingType::Fit,
            width: w,
            height: h,
        }),
        ..ParsedOptions::default()
    };

    for (sw, sh, tw, th) in [
        (4000u32, 3000u32, 200u32, 200u32),
        (9000, 7000, 450, 450),
        (1000, 1000, 999, 999),
        (4000, 100, 200, 50),
        (100, 4000, 50, 200),
        (5000, 5000, 1, 1),
    ] {
        let factor = load_shrink_factor(&plan(tw, th), sw, sh);
        assert!(factor.is_power_of_two() && factor <= 8, "odd factor {factor}");
        let (after_w, after_h) = (sw / factor, sh / factor);
        assert!(
            after_w >= tw && after_h >= th,
            "shrink 1/{factor} took {sw}x{sh} to {after_w}x{after_h}, below the {tw}x{th} target"
        );
    }
}

#[test]
fn test_load_shrink_declines_when_it_cannot_reason_about_the_target() {
    use crate::processing::load_shrink_factor;

    // A 4x target, deliberately clear of the 1/8 ceiling: at the ceiling every
    // variation below would read as "no change" and the test would prove
    // nothing.
    let with = |f: fn(&mut ParsedOptions)| {
        let mut o = ParsedOptions {
            resize: Some(Resize {
                resizing_type: ResizingType::Fit,
                width: 1000,
                height: 1000,
            }),
            ..ParsedOptions::default()
        };
        f(&mut o);
        load_shrink_factor(&o, 4000, 4000)
    };
    assert_eq!(with(|_| ()), 4, "baseline");

    // A crop addresses source pixels by coordinate; shrinking underneath it
    // would move the region being cut.
    assert_eq!(
        with(|o| o.crop = Some(Crop {
            width: 50.0,
            height: 50.0,
            gravity: None,
        })),
        1
    );
    // raw returns the source untouched.
    assert_eq!(with(|o| o.raw = true), 1);
    // No resize target to reason from.
    assert_eq!(with(|o| o.resize = None), 1);
    // Growth after the resize has to be respected, not shrunk away.
    assert_eq!(with(|o| o.dpr = Some(2.0)), 2, "dpr doubles the pixels needed");
    assert_eq!(
        with(|o| o.zoom = Some(Zoom { x: 2.0, y: 2.0 })),
        2,
        "zoom doubles the pixels needed"
    );
    assert_eq!(
        with(|o| o.min_width = Some(2000)),
        2,
        "a minimum above the resize target raises the floor"
    );
}

/// An embedded thumbnail may only stand in for the source when the result
/// cannot tell the difference — a 160x120 thumbnail answering a 1000px request
/// is the difference.
#[test]
fn test_thumbnail_stands_in_only_when_it_covers_the_request() {
    use crate::processing::thumbnail_covers;

    let with = |f: fn(&mut ParsedOptions)| {
        let mut o = ParsedOptions {
            resize: Some(Resize {
                resizing_type: ResizingType::Fit,
                width: 100,
                height: 100,
            }),
            ..ParsedOptions::default()
        };
        f(&mut o);
        thumbnail_covers(&o, 160, 120)
    };

    assert!(with(|_| ()), "a 160x120 thumbnail covers a 100x100 fit");
    // The two failure modes from the report: a target beyond the thumbnail,
    // and no target at all, which means the source at its own size.
    assert!(!with(|o| o.resize.as_mut().unwrap().width = 1000));
    assert!(!with(|o| o.resize = None));
    // Growth after the resize counts against the thumbnail too.
    assert!(!with(|o| o.dpr = Some(2.0)), "dpr 2 needs 200px from 160");
    assert!(!with(|o| o.zoom = Some(Zoom { x: 2.0, y: 2.0 })));
    assert!(!with(|o| o.min_width = Some(500)));
    // A zero axis is derived from the aspect ratio, which the thumbnail keeps.
    assert!(with(|o| o.resize.as_mut().unwrap().height = 0));
    // `force` fills a zero axis from the source dimension, which only the
    // source has.
    assert!(!with(|o| {
        let resize = o.resize.as_mut().unwrap();
        resize.resizing_type = ResizingType::Force;
        resize.height = 0;
    }));
    // A crop addresses source pixels by coordinate; the thumbnail is a
    // different image in those coordinates. Trim and raw need the source.
    assert!(!with(|o| o.crop = Some(Crop {
        width: 50.0,
        height: 50.0,
        gravity: None,
    })));
    assert!(!with(|o| o.raw = true));
}

/// End to end: the output must be identical whether or not the source was
/// decoded at a reduced scale.
#[test]
fn test_shrink_on_load_does_not_change_output_dimensions() {
    init_vips();
    let source_bytes = Bytes::from(create_test_image_jpeg(2000, 1600));
    let options = || ParsedOptions {
        resize: Some(Resize {
            resizing_type: ResizingType::Fit,
            width: 200,
            height: 200,
        }),
        format: Some("png".to_string()),
        ..ParsedOptions::default()
    };

    // Straight through, no shrink.
    let full = process_image(image_from(source_bytes.to_vec()), options(), &source_bytes, None).unwrap();

    // Decoded at the scale the plan allows, as the service does.
    let factor = crate::processing::load_shrink_factor(&options(), 2000, 1600);
    assert!(factor > 1, "this plan should qualify for shrink-on-load");
    let shrunk = VipsImage::new_from_buffer(&source_bytes, &format!("shrink={factor}")).unwrap();
    let reduced = process_image(shrunk, options(), &source_bytes, None).unwrap();

    assert_eq!(
        image::load_from_memory(&full).unwrap().dimensions(),
        image::load_from_memory(&reduced).unwrap().dimensions()
    );
}

/// `force` fills a zero axis from the source dimension, so that axis needs the
/// source at full size: `resize:force:0:500` on 4000x3000 targets 4000x500, but
/// after a 1/4 decode `resolve_resize_dimensions` would call it 1000x500 and
/// silently return a quarter-width image.
///
/// Every other resizing type derives a zero axis from the aspect ratio, which
/// survives a shrink unchanged — asserted here so the distinction is not lost.
#[test]
fn test_load_shrink_declines_for_force_with_an_unset_axis() {
    use crate::processing::load_shrink_factor;
    use crate::processing::transform::resolve_resize_dimensions;

    let plan = |kind: &str, w: u32, h: u32| ParsedOptions {
        resize: Some(Resize {
            resizing_type: kind.parse().unwrap(),
            width: w,
            height: h,
        }),
        ..ParsedOptions::default()
    };

    for (w, h) in [(0, 500), (500, 0)] {
        let options = plan("force", w, h);
        assert_eq!(
            load_shrink_factor(&options, 4000, 3000),
            1,
            "force with an unset axis must decline shrink-on-load"
        );
    }

    // fit is unaffected: the derived axis is the same before and after.
    let fit = plan("fit", 0, 500);
    let resize = fit.resize.as_ref().unwrap();
    let full = resolve_resize_dimensions(resize, 4000, 3000).unwrap();
    let shrunk = resolve_resize_dimensions(resize, 1000, 750).unwrap();
    assert_eq!(full, shrunk, "an aspect-derived axis should survive a shrink");
    assert!(load_shrink_factor(&fit, 4000, 3000) > 1, "fit should still qualify");
}

/// EXIF orientations 5-8 transpose the image, and that rotation happens after
/// the load. The plan is written against what the viewer sees, so the factor
/// must be chosen against the rotated dimensions: a stored 8000x4000
/// orientation-6 image is displayed 4000x8000, and `fill:2000:1000` against the
/// stored shape picks a factor that leaves too few pixels once rotated.
#[test]
fn test_load_shrink_uses_displayed_dimensions_for_rotated_sources() {
    use crate::processing::load_shrink_factor;

    let options = ParsedOptions {
        resize: Some(Resize {
            resizing_type: ResizingType::Fill,
            width: 2000,
            height: 1000,
        }),
        ..ParsedOptions::default()
    };

    // Stored orientation, which is what the loader reports.
    let stored = load_shrink_factor(&options, 8000, 4000);
    // What the pipeline actually sees once orientation 6 is applied.
    let displayed = load_shrink_factor(&options, 4000, 8000);

    assert_eq!(stored, 4);
    assert_eq!(displayed, 2, "the rotated shape needs a gentler shrink");
    assert!(
        displayed < stored,
        "choosing against stored dimensions over-shrinks a transposed source"
    );

    // The decoded image must still cover the request after rotation.
    let (after_w, after_h) = (8000 / displayed, 4000 / displayed);
    let (rotated_w, rotated_h) = (after_h, after_w);
    assert!(
        rotated_w >= 2000 && rotated_h >= 1000,
        "rotated {rotated_w}x{rotated_h} cannot fill a 2000x1000 box"
    );
}

/// The WebP loader rounds decoded dimensions to nearest, which can round *down*
/// — 4000 x 0.3333 is 1333.2 and decodes to 1333. This decodes real WebP data at
/// the computed scale and checks the result against the target, rather than
/// trusting a model of that rounding.
#[test]
fn test_webp_load_scale_never_undershoots_the_target() {
    use crate::processing::load_scale_factor;

    init_vips();
    // Probe the capability under test rather than a proxy for it: whether this
    // build can *encode* WebP says nothing about whether its loader takes a
    // `scale`. They happen to travel together in every libvips back to 8.12,
    // the documented minimum, but the test should not depend on that holding.
    if !webp_loader_takes_a_scale() {
        eprintln!("skipping: this libvips build cannot decode WebP at a scale");
        return;
    }

    for (sw, sh, tw, th) in [
        (4000u32, 3000u32, 1333u32, 1000u32), // ratio just over 3
        (4000, 3000, 999, 749),
        (1000, 1000, 333, 333),
        (1000, 1000, 667, 667),
        (2000, 1000, 700, 300),
        (999, 1001, 333, 334),
    ] {
        let options = ParsedOptions {
            resize: Some(Resize {
                resizing_type: ResizingType::Fit,
                width: tw,
                height: th,
            }),
            ..ParsedOptions::default()
        };
        let Some(scale) = load_scale_factor(&options, sw, sh) else {
            continue;
        };

        let webp = save::save_image(image_from(create_test_image(sw, sh)), "webp", 80).unwrap();
        let decoded = VipsImage::new_from_buffer(&webp, &format!("scale={scale}")).unwrap();
        let (dw, dh) = (decoded.get_width() as u32, decoded.get_height() as u32);

        assert!(
            dw >= tw && dh >= th,
            "scale {scale} took {sw}x{sh} to {dw}x{dh}, below the {tw}x{th} target"
        );
    }
}

/// The point of the WebP branch: a continuous scale lands much closer to what is
/// needed than JPEG's power-of-two shrink can.
#[test]
fn test_webp_scale_is_finer_than_the_jpeg_shrink() {
    use crate::processing::{load_scale_factor, load_shrink_factor};

    let options = ParsedOptions {
        resize: Some(Resize {
            resizing_type: ResizingType::Fit,
            width: 1000,
            height: 1000,
        }),
        ..ParsedOptions::default()
    };

    // A 3x reduction: JPEG has to settle for 2x, WebP can take all of it.
    assert_eq!(load_shrink_factor(&options, 3000, 3000), 2);
    let scale = load_scale_factor(&options, 3000, 3000).unwrap();
    assert!(
        (scale - 1.0 / 3.0).abs() < 0.01,
        "expected roughly a third, got {scale}"
    );

    // And a reduction JPEG declines entirely still pays off for WebP.
    assert_eq!(load_shrink_factor(&options, 1800, 1800), 1);
    assert!(load_scale_factor(&options, 1800, 1800).is_some());
}

/// Whether this build can both produce WebP and decode it at a reduced scale,
/// which is what the scale-on-load path actually needs.
fn webp_loader_takes_a_scale() -> bool {
    if !crate::processing::save::is_format_supported("webp") {
        return false;
    }
    let Ok(encoded) = save::save_image(image_from(create_test_image(32, 32)), "webp", 80) else {
        return false;
    };
    VipsImage::new_from_buffer(&encoded, "scale=0.5").is_ok()
}

/// With a crop, the pixels that have to survive the decode are the crop region,
/// not the whole source. Measuring against the source would shrink past what
/// the crop still needs.
#[test]
fn test_load_shrink_measures_the_crop_region_not_the_source() {
    use crate::processing::load_shrink_factor;

    // ParsedOptions is not Clone, so each case is built fresh.
    let plan = |crop: Option<(u32, u32)>| ParsedOptions {
        crop: crop.map(|(w, h)| Crop {
            width: f64::from(w),
            height: f64::from(h),
            gravity: None,
        }),
        resize: Some(Resize {
            resizing_type: ResizingType::Fit,
            width: 500,
            height: 375,
        }),
        ..ParsedOptions::default()
    };

    // The crop leaves 2000x1500 for a 500-wide target: a factor of 4.
    assert_eq!(load_shrink_factor(&plan(Some((2000, 1500))), 8000, 6000), 4);

    // Without the crop the whole 8000-wide source feeds the same target, so
    // there is far more to lose.
    assert_eq!(load_shrink_factor(&plan(None), 8000, 6000), 8);

    // A crop that already sits near the target leaves nothing to gain.
    assert_eq!(load_shrink_factor(&plan(Some((600, 450))), 8000, 6000), 1);
}

/// End to end: a cropped request must produce the same result whether or not
/// the source was decoded at a reduced scale.
#[test]
fn test_cropped_request_survives_a_reduced_decode() {
    init_vips();
    let source_bytes = Bytes::from(create_test_image_jpeg(2000, 1600));
    let options = || ParsedOptions {
        crop: Some(Crop {
            width: 1000.0,
            height: 800.0,
            gravity: Some(Gravity::new(GravityType::SouthEast)),
        }),
        resize: Some(Resize {
            resizing_type: ResizingType::Fit,
            width: 250,
            height: 200,
        }),
        format: Some("png".to_string()),
        ..ParsedOptions::default()
    };

    let full = process_image(image_from(source_bytes.to_vec()), options(), &source_bytes, None).unwrap();

    let factor = crate::processing::load_shrink_factor(&options(), 2000, 1600);
    assert!(factor > 1, "a cropped request should now qualify for shrink-on-load");

    // Reproduce what the service does: decode smaller, then rescale the crop.
    let shrunk = VipsImage::new_from_buffer(&source_bytes, &format!("shrink={factor}")).unwrap();
    let mut scaled_options = options();
    if let Some(crop) = scaled_options.crop.as_mut() {
        crop.width /= f64::from(factor);
        crop.height /= f64::from(factor);
    }
    let reduced = process_image(shrunk, scaled_options, &source_bytes, None).unwrap();

    assert_eq!(
        image::load_from_memory(&full).unwrap().dimensions(),
        image::load_from_memory(&reduced).unwrap().dimensions(),
        "the cropped result changed size when the source was decoded smaller"
    );
}

/// Trim removes an unknown number of pixels, so there is no way to choose a
/// decode scale against what will be left. Guessing low leaves the resize
/// short, so scale-on-load stands aside — as it does in imgproxy.
#[test]
fn test_trim_disables_scale_on_load() {
    use crate::processing::load_shrink_factor;
    use crate::processing::options::Trim;

    let plan = |trim: Option<Trim>| ParsedOptions {
        trim,
        resize: Some(Resize {
            resizing_type: ResizingType::Fit,
            width: 200,
            height: 200,
        }),
        ..ParsedOptions::default()
    };

    assert!(
        load_shrink_factor(&plan(None), 4000, 4000) > 1,
        "baseline should shrink"
    );
    assert_eq!(
        load_shrink_factor(
            &plan(Some(Trim {
                threshold: 10.0,
                color: None,
                equal_hor: false,
                equal_ver: false
            })),
            4000,
            4000
        ),
        1
    );
}
