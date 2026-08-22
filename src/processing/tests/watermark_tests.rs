use crate::constants::ENV_WATERMARK_PATH;
use crate::processing::options::{GravityType, Watermark, WatermarkPosition, WatermarkSize};
use crate::processing::watermark::{self, WatermarkPlacement};
use bytes::Bytes;
use libvips::VipsImage;
use std::io::Write;
use tempfile::NamedTempFile;

use super::tests_support::*;

#[test]
fn test_apply_watermark() {
    init_vips();
    let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
    let mut watermark_file = NamedTempFile::new().unwrap();
    watermark_file.write_all(&watermark.bytes).unwrap();
    std::env::set_var(ENV_WATERMARK_PATH, watermark_file.path());

    let img = image_from(create_test_image(200, 200));
    let watermark_opts = Watermark {
        opacity: 0.5,
        position: WatermarkPosition::parse("ce").unwrap(),
        ..Watermark::default()
    };
    let watermarked_img =
        watermark::apply_watermark(img, &watermark, &watermark_opts, WatermarkPlacement::default()).unwrap();

    assert_eq!(watermarked_img.get_width(), 200);
    assert_eq!(watermarked_img.get_height(), 200);
    std::env::remove_var(ENV_WATERMARK_PATH);
}

#[test]
fn test_apply_watermark_prepared() {
    init_vips();
    // File-based watermarks (IMGFORGE_WATERMARK_PATH) go through
    // prepare_cached_watermark, which caches decoded pixels and must
    // survive the raw-memory round trip with its colourspace intact —
    // otherwise composite_2 rejects the multiband overlay (issue #47).
    let watermark = watermark::prepare_cached_watermark(Bytes::from(create_test_image(50, 50))).unwrap();
    assert!(watermark.prepared_rgba.is_some());

    let base = create_test_image_jpeg(200, 200);
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let watermark_opts = Watermark {
        opacity: 0.5,
        position: WatermarkPosition::parse("soea").unwrap(),
        ..Watermark::default()
    };
    let watermarked =
        watermark::apply_watermark(img, &watermark, &watermark_opts, WatermarkPlacement::default()).unwrap();

    assert_eq!(watermarked.get_width(), 200);
    assert_eq!(watermarked.get_height(), 200);
    assert!(!watermarked.image_write_to_memory().is_empty());
}

#[test]
fn test_apply_watermark_prepared_matches_bytes_path() {
    init_vips();
    // The prepared cache must be a faithful reproduction of the decoded
    // watermark: applying it should yield pixel-identical output to the
    // per-request decode path (a wrong restored header would surface as
    // shifted colors, not just a composite failure).
    let watermark_bytes = create_test_image(50, 50);
    let watermark_opts = Watermark {
        opacity: 0.5,
        position: WatermarkPosition::parse("ce").unwrap(),
        ..Watermark::default()
    };

    // The base buffers must outlive the (lazily evaluated) pipelines:
    // new_from_buffer borrows the encoded bytes without copying.
    let base_a = create_quadrant_test_image(200, 200);
    let base_b = create_quadrant_test_image(200, 200);

    let from_bytes = cached_watermark_from_bytes(watermark_bytes.clone());
    let img = VipsImage::new_from_buffer(&base_a, "").unwrap();
    let via_bytes =
        watermark::apply_watermark(img, &from_bytes, &watermark_opts, WatermarkPlacement::default()).unwrap();

    let prepared = watermark::prepare_cached_watermark(Bytes::from(watermark_bytes)).unwrap();
    let img = VipsImage::new_from_buffer(&base_b, "").unwrap();
    let via_prepared =
        watermark::apply_watermark(img, &prepared, &watermark_opts, WatermarkPlacement::default()).unwrap();

    assert_eq!(via_bytes.get_width(), via_prepared.get_width());
    assert_eq!(via_bytes.get_height(), via_prepared.get_height());
    assert_eq!(via_bytes.get_bands(), via_prepared.get_bands());
    let a = via_bytes.image_write_to_memory();
    let b = via_prepared.image_write_to_memory();
    assert!(!a.is_empty(), "reference pipeline rendered no pixels");
    assert!(
        a == b,
        "prepared-cache watermark output differs from the per-request decode output"
    );
}

#[test]
fn test_apply_watermark_prepared_rgb_watermark() {
    init_vips();
    // A watermark without an alpha channel (e.g. a JPEG logo) gains one
    // in load_watermark_image before being cached; the added band must
    // survive the prepared round trip too.
    let watermark = watermark::prepare_cached_watermark(Bytes::from(create_test_image_jpeg(50, 50))).unwrap();
    assert!(watermark.prepared_rgba.is_some());

    let base = create_test_image(200, 200);
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let watermark_opts = Watermark {
        opacity: 0.5,
        position: WatermarkPosition::parse("ce").unwrap(),
        ..Watermark::default()
    };
    let watermarked =
        watermark::apply_watermark(img, &watermark, &watermark_opts, WatermarkPlacement::default()).unwrap();

    assert_eq!(watermarked.get_width(), 200);
    assert_eq!(watermarked.get_height(), 200);
    assert!(!watermarked.image_write_to_memory().is_empty());
}

#[test]
fn test_watermark_all_positions() {
    init_vips();
    let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
    let positions = vec!["no", "so", "ea", "we", "ce", "nowe", "noea", "sowe", "soea"];

    for position in positions {
        let img = image_from(create_test_image(200, 200));
        let watermark_opts = Watermark {
            opacity: 0.5,
            position: WatermarkPosition::parse(position).unwrap(),
            ..Watermark::default()
        };
        let watermarked =
            watermark::apply_watermark(img, &watermark, &watermark_opts, WatermarkPlacement::default()).unwrap();
        assert_eq!(watermarked.get_width(), 200);
        assert_eq!(watermarked.get_height(), 200);
    }
}

#[test]
fn test_watermark_full_opacity() {
    init_vips();
    let img = image_from(create_test_image(200, 200));
    let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
    let watermark_opts = Watermark {
        opacity: 1.0,
        position: WatermarkPosition::parse("ce").unwrap(),
        ..Watermark::default()
    };
    let watermarked =
        watermark::apply_watermark(img, &watermark, &watermark_opts, WatermarkPlacement::default()).unwrap();
    assert_eq!(watermarked.get_width(), 200);
    assert_eq!(watermarked.get_height(), 200);
}

#[test]
fn test_watermark_zero_opacity() {
    init_vips();
    let img = image_from(create_test_image(200, 200));
    let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
    let watermark_opts = Watermark {
        opacity: 0.0,
        position: WatermarkPosition::parse("ce").unwrap(),
        ..Watermark::default()
    };
    let watermarked =
        watermark::apply_watermark(img, &watermark, &watermark_opts, WatermarkPlacement::default()).unwrap();
    assert_eq!(watermarked.get_width(), 200);
    assert_eq!(watermarked.get_height(), 200);
}

/// The bounding box of everything that is not the base colour, as
/// `(left, top, width, height)`. Lets a test say where the watermark landed and
/// how big it came out without comparing whole pixel buffers.
fn drawn_box(img: &VipsImage, base: [u8; 4]) -> (u32, u32, u32, u32) {
    let decoded = decode_rgba(img);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for (x, y, pixel) in decoded.enumerate_pixels() {
        if [pixel[0], pixel[1], pixel[2], pixel[3]] != base {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    assert!(min_x != u32::MAX, "nothing was drawn over the base");
    (min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
}

fn place(
    watermark: &watermark::CachedWatermark,
    position: WatermarkPosition,
    offsets: (f64, f64),
    placement: WatermarkPlacement<'_>,
) -> VipsImage {
    watermark::apply_watermark(
        image_from(create_test_image(400, 400)),
        watermark,
        &Watermark {
            opacity: 1.0,
            position,
            x_offset: offsets.0,
            y_offset: offsets.1,
            scale: 0.0,
        },
        placement,
    )
    .unwrap()
}

/// imgproxy resizes a watermark to *fit* the size it is given — its docs are
/// explicit that it "always uses `fit` resizing type when resizing watermarks".
/// Scaling the axes independently turned `wms:100:100` on a 100x50 logo into a
/// 100x100 one, distorting a logo that was only asked to fit inside the box.
#[test]
fn watermark_size_fits_rather_than_stretches() {
    init_vips();

    // Twice as wide as it is tall, and patterned so a stretch is visible.
    let watermark = cached_watermark_from_bytes(create_quadrant_test_image(100, 50));
    const RED: [u8; 4] = [255, 0, 0, 255];

    let sized = |width: u32, height: u32| {
        let img = place(
            &watermark,
            WatermarkPosition::Anchor(GravityType::NorthWest),
            (0.0, 0.0),
            WatermarkPlacement {
                size: Some(WatermarkSize { width, height }),
                rotate: None,
                resizing_algorithm: None,
                offset_scale: 1.0,
            },
        );
        drawn_box(&img, RED)
    };

    // Asked to fit a 100x100 box, a 2:1 watermark stays 2:1 — the width binds.
    assert_eq!(sized(100, 100), (0, 0, 100, 50));
    // Asking for its natural size is the same request.
    assert_eq!(sized(100, 50), (0, 0, 100, 50));
    // A tighter height binds instead, and the width follows it down.
    assert_eq!(sized(100, 25), (0, 0, 50, 25));
    // A zero axis leaves that side unbounded.
    assert_eq!(sized(200, 0), (0, 0, 200, 100));
    assert_eq!(sized(0, 100), (0, 0, 200, 100));
}

/// imgproxy scales watermark *offsets* by DPR — "affects gravities offsets,
/// watermark offsets, and paddings to make the resulting image structures with
/// and without the dpr option applied match". Leaving them at 1x let the
/// watermark creep toward the corner on a high-density request.
#[test]
fn watermark_offsets_scale_with_dpr() {
    init_vips();

    let watermark = cached_watermark_from_bytes(create_quadrant_test_image(20, 20));
    const RED: [u8; 4] = [255, 0, 0, 255];

    let inset = |offset_scale: f64| {
        let img = place(
            &watermark,
            WatermarkPosition::Anchor(GravityType::NorthWest),
            (10.0, 10.0),
            WatermarkPlacement {
                size: None,
                rotate: None,
                resizing_algorithm: None,
                offset_scale,
            },
        );
        let (left, top, _, _) = drawn_box(&img, RED);
        (left, top)
    };

    assert_eq!(inset(1.0), (10, 10), "a 10px inset is 10px at 1x");
    assert_eq!(
        inset(2.0),
        (20, 20),
        "and has to stay visually 10px once the image is twice the size"
    );

    // The scale acts on the offsets and nothing else, so an anchor with none is
    // unaffected — the watermark's own size must not move with DPR.
    let centred = |offset_scale: f64| {
        let img = place(
            &watermark,
            WatermarkPosition::Anchor(GravityType::Center),
            (0.0, 0.0),
            WatermarkPlacement {
                size: None,
                rotate: None,
                resizing_algorithm: None,
                offset_scale,
            },
        );
        drawn_box(&img, RED)
    };
    assert_eq!(centred(1.0), centred(2.0));
}
