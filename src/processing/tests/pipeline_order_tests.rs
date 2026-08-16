//! Tests for the *order* of the pipeline rather than any single stage.
//!
//! Every case here passed before the stages were reordered, because each stage
//! was correct on its own and nothing asserted how they compose. These are the
//! interactions imgproxy's `mainPipeline` order exists to get right.

use crate::processing::options::{
    Crop, Flip, Gravity, GravityType, ParsedOptions, Resize, ResizingType, Watermark, WatermarkPosition,
};
use crate::processing::process_image;
use bytes::Bytes;

use super::tests_support::*;

fn options(configure: impl FnOnce(&mut ParsedOptions)) -> ParsedOptions {
    let mut options = ParsedOptions {
        format: Some("png".to_string()),
        ..ParsedOptions::default()
    };
    configure(&mut options);
    options
}

fn run(source: &Bytes, options: ParsedOptions) -> image::RgbaImage {
    let img = image_from(source.to_vec());
    let bytes = process_image(img, options, source, None).expect("processing succeeds");
    image::load_from_memory(&bytes).expect("result decodes").to_rgba8()
}

/// A rotation between the scale and the result crop is what keeps the
/// requested size meaning the size the caller gets. With the crop before the
/// rotation, `resize:fill:80:40` plus `rotate:90` came back 40x80 — the right
/// pixels, transposed.
#[test]
fn a_rotation_does_not_transpose_the_requested_fill_size() {
    init_vips();
    let source = Bytes::from(create_test_image(200, 100));

    let upright = run(
        &source,
        options(|o| {
            o.resize = Some(Resize {
                resizing_type: ResizingType::Fill,
                width: 80,
                height: 40,
            });
        }),
    );
    assert_eq!((upright.width(), upright.height()), (80, 40));

    let rotated = run(
        &source,
        options(|o| {
            o.resize = Some(Resize {
                resizing_type: ResizingType::Fill,
                width: 80,
                height: 40,
            });
            o.rotation = Some(90);
        }),
    );
    assert_eq!(
        (rotated.width(), rotated.height()),
        (80, 40),
        "the requested size describes the image the caller receives"
    );
}

/// The crop gravity names a corner of the image the caller receives, so a
/// rotation still to come has to be compensated for. Rotating the quadrant
/// image 90 degrees clockwise moves the bottom-left quadrant to the top left,
/// so that is the one `nowe` has to select.
#[test]
fn a_crop_gravity_names_a_corner_of_the_rotated_result() {
    init_vips();
    let source = Bytes::from(create_quadrant_test_image(100, 100));
    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    // Without a rotation, the top-left quadrant is red.
    let upright = run(
        &source,
        options(|o| {
            o.crop = Some(Crop {
                width: 50.0,
                height: 50.0,
                gravity: Some(Gravity::new(GravityType::NorthWest)),
            });
        }),
    );
    assert_eq!(rgba_pixel(&upright, 25, 25), RED);

    // With one, the quadrant that *becomes* the top left is the blue one.
    let rotated = run(
        &source,
        options(|o| {
            o.crop = Some(Crop {
                width: 50.0,
                height: 50.0,
                gravity: Some(Gravity::new(GravityType::NorthWest)),
            });
            o.rotation = Some(90);
        }),
    );
    assert_eq!(
        rgba_pixel(&rotated, 25, 25),
        BLUE,
        "the crop should select the quadrant that ends up at the top left"
    );
}

/// A flip needs the same compensation as a rotation.
#[test]
fn a_crop_gravity_names_a_corner_of_the_flipped_result() {
    init_vips();
    let source = Bytes::from(create_quadrant_test_image(100, 100));
    const GREEN: [u8; 4] = [0, 255, 0, 255];

    // Flipped horizontally, the top-right (green) quadrant lands top left.
    let flipped = run(
        &source,
        options(|o| {
            o.crop = Some(Crop {
                width: 50.0,
                height: 50.0,
                gravity: Some(Gravity::new(GravityType::NorthWest)),
            });
            o.flip = Some(Flip {
                horizontal: true,
                vertical: false,
            });
        }),
    );
    assert_eq!(rgba_pixel(&flipped, 25, 25), GREEN);
}

/// Filters run before the canvas grows, so a blur cannot bleed across the
/// padding boundary. Blurring afterwards mixed the pad colour into the image
/// and the image into the pad.
#[test]
fn a_blur_does_not_bleed_into_the_padding() {
    init_vips();
    let source = Bytes::from(create_test_image(20, 20));

    let padded = run(
        &source,
        options(|o| {
            o.blur = Some(3.0);
            o.padding = Some((5, 5, 5, 5));
            o.background = Some([255, 255, 255, 255]);
        }),
    );

    assert_eq!((padded.width(), padded.height()), (30, 30));
    // The pad pixel directly against the image edge must still be the pad
    // colour. With the blur applied after padding it picked up the red.
    for (x, y) in [(4, 15), (25, 15), (15, 4), (15, 25), (0, 0)] {
        assert_eq!(
            rgba_pixel(&padded, x, y),
            [255, 255, 255, 255],
            "padding at ({x}, {y}) should be untouched by the blur"
        );
    }
}

/// Flattening closes out the picture before anything is laid over it.
///
/// Unlike the cases above this is not a behaviour fix: "over" compositing is
/// associative, so flattening before or after the watermark yields the same
/// pixels. What it does buy is that the encoder is handed an image with no
/// alpha band at all rather than one that regained a fourth band from the
/// watermark. This test guards that, and passes on both orders — it is here to
/// stop the reorder regressing, not to prove it fixed something.
#[test]
fn a_watermark_over_a_flattened_image_stays_encodable() {
    init_vips();
    let source = Bytes::from(create_transparent_edge_image(80, 40));
    let watermark = cached_watermark_from_bytes(create_test_image(20, 20));

    let mut options = ParsedOptions {
        format: Some("jpeg".to_string()),
        background: Some([0, 0, 255, 255]),
        watermark: Some(Watermark {
            opacity: 0.5,
            position: WatermarkPosition::parse("ce").unwrap(),
            ..Watermark::default()
        }),
        ..ParsedOptions::default()
    };
    options.resize = Some(Resize {
        resizing_type: ResizingType::Fit,
        width: 40,
        height: 0,
    });

    let img = image_from(source.to_vec());
    let bytes = process_image(img, options, &source, Some(&watermark)).expect("jpeg with a watermark encodes");

    let decoded = image::load_from_memory(&bytes).expect("result decodes");
    // JPEG cannot carry alpha, so the transparent half must have become the
    // background rather than reaching the encoder as a fourth band.
    assert_eq!(decoded.color().channel_count(), 3, "the result should be opaque");
}
