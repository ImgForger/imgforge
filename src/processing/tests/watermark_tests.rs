use crate::constants::ENV_WATERMARK_PATH;
use crate::processing::options::Watermark;
use crate::processing::watermark;
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

    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
    let watermark_opts = Watermark {
        opacity: 0.5,
        position: "ce".to_string(),
    };
    let watermarked_img = watermark::apply_watermark(img, &watermark, &watermark_opts, None).unwrap();

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
        position: "soea".to_string(),
    };
    let watermarked = watermark::apply_watermark(img, &watermark, &watermark_opts, None).unwrap();

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
        position: "ce".to_string(),
    };

    // The base buffers must outlive the (lazily evaluated) pipelines:
    // new_from_buffer borrows the encoded bytes without copying.
    let base_a = create_quadrant_test_image(200, 200);
    let base_b = create_quadrant_test_image(200, 200);

    let from_bytes = cached_watermark_from_bytes(watermark_bytes.clone());
    let img = VipsImage::new_from_buffer(&base_a, "").unwrap();
    let via_bytes = watermark::apply_watermark(img, &from_bytes, &watermark_opts, None).unwrap();

    let prepared = watermark::prepare_cached_watermark(Bytes::from(watermark_bytes)).unwrap();
    let img = VipsImage::new_from_buffer(&base_b, "").unwrap();
    let via_prepared = watermark::apply_watermark(img, &prepared, &watermark_opts, None).unwrap();

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
        position: "ce".to_string(),
    };
    let watermarked = watermark::apply_watermark(img, &watermark, &watermark_opts, None).unwrap();

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
        let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
        let watermark_opts = Watermark {
            opacity: 0.5,
            position: position.to_string(),
        };
        let watermarked = watermark::apply_watermark(img, &watermark, &watermark_opts, None).unwrap();
        assert_eq!(watermarked.get_width(), 200);
        assert_eq!(watermarked.get_height(), 200);
    }
}

#[test]
fn test_watermark_full_opacity() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
    let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
    let watermark_opts = Watermark {
        opacity: 1.0,
        position: "ce".to_string(),
    };
    let watermarked = watermark::apply_watermark(img, &watermark, &watermark_opts, None).unwrap();
    assert_eq!(watermarked.get_width(), 200);
    assert_eq!(watermarked.get_height(), 200);
}

#[test]
fn test_watermark_zero_opacity() {
    init_vips();
    let img = VipsImage::new_from_buffer(&create_test_image(200, 200), "").unwrap();
    let watermark = cached_watermark_from_bytes(create_test_image(50, 50));
    let watermark_opts = Watermark {
        opacity: 0.0,
        position: "ce".to_string(),
    };
    let watermarked = watermark::apply_watermark(img, &watermark, &watermark_opts, None).unwrap();
    assert_eq!(watermarked.get_width(), 200);
    assert_eq!(watermarked.get_height(), 200);
}
