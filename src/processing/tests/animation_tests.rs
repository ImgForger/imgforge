use crate::processing::animation;
use crate::processing::options::{ParsedOptions, Resize, ResizingType};
use crate::processing::process_image;
use bytes::Bytes;
use libvips::VipsImage;

use super::tests_support::*;

/// Opens an animated source the way the service does, reading every frame.
fn open_animated(bytes: &Bytes) -> VipsImage {
    VipsImage::new_from_buffer(bytes, "page=0,n=-1").expect("animated source should decode")
}

#[test]
fn an_animation_is_split_into_frames_and_rejoined() {
    init_vips();
    let source = Bytes::from(create_animated_gif(60, 40, 4));
    let img = open_animated(&source);

    assert_eq!(animation::frame_geometry(&img), Some((4, 40)));

    let frames = animation::split(&img).expect("split succeeds");
    assert_eq!(frames.images.len(), 4);
    for frame in &frames.images {
        assert_eq!((frame.get_width(), frame.get_height()), (60, 40));
    }

    let (joined, frame_height) = animation::join(frames.images).expect("join succeeds");
    assert_eq!(frame_height, Some(40));
    assert_eq!((joined.get_width(), joined.get_height()), (60, 160));
}

#[test]
fn a_still_image_produces_exactly_one_frame() {
    init_vips();
    let img = image_from(create_test_image(20, 10));

    assert_eq!(animation::frame_geometry(&img), None);
    let frames = animation::split(&img).expect("split succeeds");
    assert_eq!(frames.images.len(), 1);

    let (joined, frame_height) = animation::join(frames.images).expect("join succeeds");
    // A single frame needs no page height: telling the encoder about frames
    // that do not exist is how a still ends up written as a one-frame
    // animation.
    assert_eq!(frame_height, None);
    assert_eq!((joined.get_width(), joined.get_height()), (20, 10));
}

/// The whole point of splitting frames: a resized animation has to come back
/// with the same number of frames, each at the new size. Resizing the stacked
/// strip in one go leaves the frame height stale, which silently reinterprets
/// four 40px frames as two 80px ones.
#[test]
fn resizing_an_animation_keeps_every_frame() {
    init_vips();
    let source = Bytes::from(create_animated_gif(60, 40, 4));
    let options = ParsedOptions {
        resize: Some(Resize {
            resizing_type: ResizingType::Fit,
            width: 30,
            height: 20,
        }),
        format: Some("gif".to_string()),
        ..ParsedOptions::default()
    };

    let output = process_image(open_animated(&source), options, &source, None).expect("processing succeeds");

    assert_eq!(frame_count(&output), 4, "every frame should survive the resize");
    let decoded = VipsImage::new_from_buffer(&output, "").expect("result decodes");
    assert_eq!((decoded.get_width(), decoded.get_page_height()), (30, 20));
}

#[test]
fn an_animation_survives_a_rotation_that_changes_the_frame_shape() {
    init_vips();
    // Rotating the stacked strip by 90 degrees would turn the frames into
    // vertical slices of one wide image. Rotating each frame separately is the
    // only way this can work at all.
    let source = Bytes::from(create_animated_gif(60, 40, 3));
    let options = ParsedOptions {
        rotation: Some(90),
        format: Some("gif".to_string()),
        ..ParsedOptions::default()
    };

    let output = process_image(open_animated(&source), options, &source, None).expect("processing succeeds");

    assert_eq!(frame_count(&output), 3);
    let decoded = VipsImage::new_from_buffer(&output, "").expect("result decodes");
    assert_eq!((decoded.get_width(), decoded.get_page_height()), (40, 60));
}

/// A still format cannot carry frames, so the encoder must be told nothing
/// about them; the result is the first frame at its own size, not a tall strip.
#[test]
fn a_still_output_format_gets_a_single_frame() {
    init_vips();
    let source = Bytes::from(create_animated_gif(60, 40, 4));
    let options = ParsedOptions {
        format: Some("png".to_string()),
        ..ParsedOptions::default()
    };

    // The service only reads every frame when the output can hold them; this
    // mirrors what a PNG request actually opens.
    let img = VipsImage::new_from_buffer(&source, "").expect("first frame decodes");
    let output = process_image(img, options, &source, None).expect("processing succeeds");

    let decoded = VipsImage::new_from_buffer(&output, "").expect("result decodes");
    assert_eq!((decoded.get_width(), decoded.get_height()), (60, 40));
}

#[test]
fn an_oversized_animation_frame_is_refused() {
    use crate::processing::ProcessingError;

    init_vips();
    let source = Bytes::from(create_animated_gif(60, 40, 4));
    let options = ParsedOptions {
        format: Some("gif".to_string()),
        // 60x40 is 2400 pixels; a limit of one thousandth of a megapixel is
        // below that.
        max_animation_frame_resolution: Some("0.001".parse().unwrap()),
        ..ParsedOptions::default()
    };

    let result = process_image(open_animated(&source), options, &source, None);
    assert!(matches!(result, Err(ProcessingError::FrameTooLarge { .. })));
}

/// Both output ceilings describe a frame the caller sees, not the stack libvips
/// hands over. Measuring the joined stack made a ten-frame 100x100 animation
/// fail a 500px result limit, and needlessly downscaled a tall stack while
/// leaving the encoder a page height that described the frames from before.
#[test]
fn the_result_ceiling_measures_a_frame_rather_than_the_stack() {
    use crate::processing::options::{Resize, ResizingType};

    init_vips();
    let source = Bytes::from(create_animated_gif(60, 40, 6));
    // Six 40px frames stack to 240px, well over the ceiling; each frame is not.
    let options = ParsedOptions {
        format: Some("gif".to_string()),
        max_result_dimension: Some("100".parse().unwrap()),
        resize: Some(Resize {
            resizing_type: ResizingType::Fit,
            width: 60,
            height: 40,
        }),
        ..ParsedOptions::default()
    };

    let output = process_image(open_animated(&source), options, &source, None)
        .expect("every frame is inside the ceiling, so the request stands");
    assert_eq!(frame_count(&output), 6);
}

/// The decode scale has to come from one frame too. Planning it against the
/// stacked height over-shrinks every frame, and `enlarge:false` cannot recover.
#[test]
fn the_load_scale_is_derived_from_one_frame() {
    use crate::processing::load_scale_factor;
    use crate::processing::options::{Resize, ResizingType};

    let options = ParsedOptions {
        resize: Some(Resize {
            resizing_type: ResizingType::Fit,
            width: 100,
            height: 100,
        }),
        ..ParsedOptions::default()
    };

    // One 2000x1000 frame needs a factor of 10; the same frame seen as part of
    // a ten-frame 2000x10000 stack would suggest 20.
    let per_frame = load_scale_factor(&options, 2000, 1000).expect("a reduced decode applies");
    assert!((per_frame - 0.1).abs() < 1e-9, "expected a tenth, got {per_frame}");
}
