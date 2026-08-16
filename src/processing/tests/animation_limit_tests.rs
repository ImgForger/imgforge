//! `max_animation_frame_resolution` against sources that collapse to one frame.

use crate::processing::options::ParsedOptions;
use crate::processing::{process_image, ProcessingError};
use bytes::Bytes;
use image::codecs::gif::GifEncoder;
use image::{Delay, Frame, RgbaImage};
use libvips::VipsImage;

use super::tests_support::*;

/// A two-frame GIF of `size` x `size`, so each frame is `size * size` pixels.
fn animated_gif(size: u32, frames: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        for index in 0..frames {
            // Frames that are identical get collapsed by some encoders, so each
            // one is given its own colour.
            let shade = (index * 60 % 256) as u8;
            let image = RgbaImage::from_pixel(size, size, image::Rgba([shade, 20, 200, 255]));
            encoder
                .encode_frame(Frame::from_parts(image, 0, 0, Delay::from_numer_denom_ms(100, 1)))
                .expect("frame should encode");
        }
    }
    bytes
}

fn open(bytes: &Bytes, load_options: &str) -> VipsImage {
    VipsImage::new_from_buffer(bytes, load_options).expect("the GIF should decode")
}

/// The limit describes an animation frame, and a frame does not stop being one
/// because the request asked for only the first of them. Reading the frame
/// count off the frames in hand let `disable_animation` — and equally `pages:1`
/// or a still output format — walk an oversized animation straight past the
/// ceiling that exists to refuse it.
#[test]
fn a_collapsed_animation_still_meets_the_frame_limit() {
    init_vips();

    let bytes = Bytes::from(animated_gif(100, 3));
    let limit = "0.005".parse().expect("5000 pixels is a valid limit");

    // Whole: three 10000-pixel frames against a 5000-pixel ceiling.
    let options = ParsedOptions {
        max_animation_frame_resolution: Some(limit),
        format: Some("gif".to_string()),
        ..ParsedOptions::default()
    };
    let whole = process_image(open(&bytes, "n=-1"), options, &bytes, None);
    assert!(
        matches!(whole, Err(ProcessingError::FrameTooLarge { .. })),
        "an oversized animation must be refused, got {:?}",
        whole.map(|bytes| bytes.len())
    );

    // Collapsed: the same source, opened as its first page alone. The frame is
    // the same size, so the same answer is the only consistent one.
    let options = ParsedOptions {
        max_animation_frame_resolution: Some(limit),
        disable_animation: true,
        format: Some("png".to_string()),
        ..ParsedOptions::default()
    };
    let collapsed = process_image(open(&bytes, "page=0,n=1"), options, &bytes, None);
    assert!(
        matches!(collapsed, Err(ProcessingError::FrameTooLarge { .. })),
        "disable_animation must not be a way past the frame limit, got {:?}",
        collapsed.map(|bytes| bytes.len())
    );
}

/// The limit is about animation frames, so a genuine still image is measured by
/// `max_src_resolution` and must pass untouched.
#[test]
fn a_still_image_is_not_measured_against_the_frame_limit() {
    init_vips();

    let bytes = Bytes::from(create_test_image(100, 100));
    let options = ParsedOptions {
        max_animation_frame_resolution: Some("0.005".parse().unwrap()),
        format: Some("png".to_string()),
        ..ParsedOptions::default()
    };

    let result = process_image(open(&bytes, ""), options, &bytes, None);
    assert!(
        result.is_ok(),
        "a still image must not be refused by the animation limit: {:?}",
        result.err()
    );
}

/// A GIF whose subject is a different width in each frame, so a per-frame trim
/// would find a different region in each and produce frames that no animated
/// container can hold.
fn animated_gif_with_varying_content() -> Vec<u8> {
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame, RgbaImage};

    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        for width in [20u32, 60u32] {
            let mut frame = RgbaImage::from_pixel(100, 60, image::Rgba([255, 255, 255, 255]));
            for y in 20..40 {
                for x in 10..(10 + width) {
                    frame.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
                }
            }
            encoder
                .encode_frame(Frame::from_parts(frame, 0, 0, Delay::from_numer_denom_ms(100, 1)))
                .expect("frame should encode");
        }
    }
    bytes
}

/// Trim is a per-image measurement and an animation is not one image: each frame
/// finds its own borders, comes out a different size, and the join then refuses
/// the set — failing a request over an option that simply does not apply.
/// imgproxy drops the option with a warning instead, and so does imgforge.
#[test]
fn trim_is_ignored_for_an_animation_rather_than_failing_it() {
    init_vips();

    let bytes = Bytes::from(animated_gif_with_varying_content());
    let trim = crate::processing::options::Trim {
        threshold: 10.0,
        color: None,
        equal_hor: false,
        equal_ver: false,
    };

    // The premise: trimmed independently, these frames really do disagree.
    let split = crate::processing::animation::split(&open(&bytes, "n=-1")).unwrap();
    assert_eq!(split.images.len(), 2);
    let sizes: Vec<(i32, i32)> = split
        .images
        .into_iter()
        .map(|frame| {
            let trimmed = crate::processing::transform::apply_trim(frame, &trim).unwrap();
            (trimmed.get_width(), trimmed.get_height())
        })
        .collect();
    assert_ne!(sizes[0], sizes[1], "the frames must trim to different sizes");

    // Through the pipeline the option is dropped, and the animation survives
    // whole: both frames, at the source's own size.
    let options = ParsedOptions {
        trim: Some(trim),
        format: Some("gif".to_string()),
        ..ParsedOptions::default()
    };
    let out = process_image(open(&bytes, "n=-1"), options, &bytes, None)
        .expect("an animation with trim must not fail the request");

    let result = open(&Bytes::from(out.to_vec()), "n=-1");
    assert_eq!(result.get_n_pages(), 2, "both frames should survive");
    assert_eq!(result.get_width(), 100, "and keep the source's width, untrimmed");
    assert_eq!(result.get_page_height(), 60, "and its frame height");
}

/// A still image is one image, so trim applies to it exactly as asked.
#[test]
fn trim_still_applies_to_a_single_frame_source() {
    init_vips();

    let bytes = Bytes::from(create_bordered_image(
        (100, 60),
        [255, 255, 255, 255],
        (20, 10, 40, 30),
        [0, 0, 0, 255],
    ));
    let options = ParsedOptions {
        trim: Some(crate::processing::options::Trim {
            threshold: 10.0,
            color: None,
            equal_hor: false,
            equal_ver: false,
        }),
        format: Some("png".to_string()),
        ..ParsedOptions::default()
    };

    let out = process_image(open(&bytes, ""), options, &bytes, None).expect("a still image trims normally");
    let result = open(&Bytes::from(out.to_vec()), "");
    assert_eq!(
        (result.get_width(), result.get_height()),
        (40, 30),
        "the border should be gone, leaving just the subject"
    );
}
