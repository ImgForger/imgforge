use crate::processing::options::SaveOptions;
use crate::processing::save;
use image::{ImageBuffer, Rgb};
use libvips::VipsImage;

use super::tests_support::*;

/// Deterministic textured image — flat colors encode to the same few
/// bytes at any quality, so quality assertions need detail to discard.
fn create_textured_image(width: u32, height: u32) -> Vec<u8> {
    let mut state: u32 = 0x2545_f491;
    let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (_x, _y, pixel) in img.enumerate_pixels_mut() {
        // xorshift32 — deterministic, no rand dependency
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *pixel = Rgb([
            (state & 0xff) as u8,
            ((state >> 8) & 0xff) as u8,
            ((state >> 16) & 0xff) as u8,
        ]);
    }
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

#[test]
fn test_webp_save_honors_quality() {
    init_vips();
    // Quality must change the encoded output: a low-quality save has to
    // be smaller than a high-quality one (issue #46 — quality was
    // silently dropped for WebP).
    let base = create_textured_image(400, 400);
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let low = save::save_image(img, "webp", 20).unwrap();
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let high = save::save_image(img, "webp", 95).unwrap();

    assert!(!low.is_empty());
    assert!(
        low.len() < high.len(),
        "webp quality ignored: q20 is {} bytes, q95 is {} bytes",
        low.len(),
        high.len()
    );
}

#[test]
fn test_webp_save_lossless_applies() {
    init_vips();
    // Lossless output of a noisy source is much larger than a lossy
    // encode — if the option were dropped, the sizes would match.
    let base = create_textured_image(400, 400);
    let mut options = SaveOptions::default();
    options.webp.lossless = Some(true);

    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let lossless = save::save_image_with_options(img, "webp", 80, &options, None).unwrap();
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let lossy = save::save_image(img, "webp", 80).unwrap();

    assert!(!lossless.is_empty());
    assert!(
        lossless.len() > lossy.len(),
        "webp lossless option had no effect: lossless {} bytes, lossy {} bytes",
        lossless.len(),
        lossy.len()
    );
}

#[test]
fn test_webp_save_honors_max_bytes() {
    init_vips();
    // max_bytes walks quality down until the encode fits; that loop was a
    // no-op while every WebP encode came back the same size.
    let base = create_textured_image(400, 400);
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let unbounded = save::save_image(img, "webp", 95).unwrap();
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let budget = save::save_image(img, "webp", 20).unwrap().len();

    let options = SaveOptions {
        max_bytes: Some(budget),
        ..Default::default()
    };
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let bounded = save::save_image_with_options(img, "webp", 95, &options, None).unwrap();

    assert!(
        bounded.len() <= budget && bounded.len() < unbounded.len(),
        "webp max_bytes not enforced: {} bytes for a {} byte budget (unbounded is {} bytes)",
        bounded.len(),
        budget,
        unbounded.len()
    );
}

#[test]
fn test_webp_save_suffix_carries_encoder_options() {
    let mut options = SaveOptions::default();
    assert_eq!(
        save::save_suffix("webp", 80, &options, None).unwrap(),
        ".webp[Q=80,keep=all]"
    );

    options.webp.lossless = Some(true);
    options.webp.smart_subsample = Some(true);
    options.webp.preset = Some("photo".to_string());
    options.strip_metadata = Some(true);
    options.strip_color_profile = Some(true);
    assert_eq!(
        save::save_suffix("webp", 90, &options, None).unwrap(),
        ".webp[Q=90,lossless,smart-subsample,preset=photo,keep=none]"
    );
}

#[test]
fn test_webp_save_suffix_carries_the_animation_frame_height() {
    // libvips stores an animation as one tall image; without the frame height
    // the encoder writes a single very tall still instead of an animation.
    let options = SaveOptions::default();
    assert_eq!(
        save::save_suffix("webp", 80, &options, Some(40)).unwrap(),
        ".webp[Q=80,page-height=40,keep=all]"
    );

    // A format that cannot hold more than one frame never gets the option,
    // because naming it would be meaningless rather than merely redundant.
    assert_eq!(
        save::save_suffix("jpeg", 80, &options, Some(40)).unwrap(),
        ".jpg[Q=80,optimize-coding,quant-table=0,subsample-mode=auto,keep=all]"
    );
}

#[test]
fn test_webp_save_suffix_clamps_quality() {
    let options = SaveOptions::default();
    assert_eq!(
        save::save_suffix("webp", 0, &options, None).unwrap(),
        ".webp[Q=1,keep=all]"
    );
}

#[test]
fn test_webp_save_suffix_drops_unknown_preset() {
    // `preset` arrives as free text from the URL, so anything vips does not
    // define must never reach the option string.
    let mut options = SaveOptions::default();
    options.webp.preset = Some("photo],lossless".to_string());
    assert_eq!(
        save::save_suffix("webp", 75, &options, None).unwrap(),
        ".webp[Q=75,keep=all]"
    );
}

#[test]
fn test_heif_save_suffix_carries_encoder_options() {
    let mut options = SaveOptions::default();
    assert_eq!(
        save::save_suffix("avif", 80, &options, None).unwrap(),
        ".avif[Q=80,compression=av1,effort=7,subsample-mode=auto,keep=all]"
    );

    options.avif.no_subsample = Some(true);
    options.strip_metadata = Some(true);
    options.strip_color_profile = Some(true);
    assert_eq!(
        save::save_suffix("heif", 100, &options, None).unwrap(),
        // effort clamps to the 0-9 libvips accepts
        ".heif[Q=100,compression=hevc,effort=9,subsample-mode=off,keep=none]"
    );
}

#[test]
fn test_gif_save_suffix_carries_encoder_options() {
    let options = SaveOptions::default();
    assert_eq!(
        save::save_suffix("gif", 50, &options, None).unwrap(),
        ".gif[effort=5,keep=all]"
    );
    // gifsave takes effort 1-10, unlike the 0-9 of the HEIF family
    assert_eq!(
        save::save_suffix("gif", 1, &options, None).unwrap(),
        ".gif[effort=1,keep=all]"
    );
}

#[test]
fn stripping_one_kind_of_metadata_keeps_the_other() {
    // The two strip options address different flags. Collapsing them into a
    // single "keep nothing" — which a one-variant enum forces — meant asking
    // to drop the colour profile also discarded the EXIF, and the reverse.
    let mut options = SaveOptions {
        strip_metadata: Some(true),
        ..SaveOptions::default()
    };

    assert_eq!(
        save::save_suffix("webp", 80, &options, None).unwrap(),
        ".webp[Q=80,keep=icc]"
    );

    options.strip_metadata = Some(false);
    options.strip_color_profile = Some(true);
    assert_eq!(
        save::save_suffix("webp", 80, &options, None).unwrap(),
        ".webp[Q=80,keep=exif|xmp|iptc|other]"
    );

    // A gain map is what makes an image HDR, so preserving HDR has to keep it
    // even while everything else is being stripped — on a libvips that has the
    // flag. On an older one the flag is dropped rather than named, because
    // naming it would make the option-string parser reject the whole encode.
    init_vips();
    options.strip_metadata = Some(true);
    options.preserve_hdr = Some(true);
    let suffix = save::save_suffix("avif", 80, &options, None).unwrap();
    assert!(
        suffix == ".avif[Q=80,compression=av1,effort=7,subsample-mode=auto,keep=gainmap]"
            || suffix == ".avif[Q=80,compression=av1,effort=7,subsample-mode=auto,keep=none]",
        "unexpected suffix: {suffix}"
    );
}

/// These formats had no encode coverage at all, which is why the encoder
/// options naming properties absent from older libvips went unnoticed — see
/// the WebP case in issue #46.
#[test]
fn test_avif_save_produces_output() {
    assert_encodes_without_property_error("avif");
}

#[test]
fn test_gif_save_produces_output() {
    assert_encodes_without_property_error("gif");
}

/// Fails only on the regression these tests exist for: an encoder option
/// naming a property the runtime libvips does not have, which rejects the
/// whole call.
///
/// Whether a build ships a given codec is environmental and separate.
/// `is_format_supported` cannot tell the two apart — it asks libvips for a
/// saver for the extension, which exists whether or not the codec was
/// compiled in — so the distinction has to come from the failure itself. CI
/// runs a libvips with no AV1 encoder, where AVIF fails with "Unsupported
/// compression"; that is a skip, while "no property named ..." is a bug.
fn assert_encodes_without_property_error(format: &str) {
    init_vips();
    if !save::is_format_supported(format) {
        eprintln!("skipping {format}: this libvips build has no saver for it");
        return;
    }

    let base = create_textured_image(64, 64);
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    clear_vips_error();

    match save::save_image(img, format, 80) {
        Ok(bytes) => assert!(!bytes.is_empty(), "{format} encode produced no bytes"),
        Err(err) => {
            let detail = vips_error_buffer();
            assert!(
                !detail.contains("no property named"),
                "{format}: encoder options named a property this libvips does not have — {} ({err})",
                detail.trim()
            );
            eprintln!(
                "skipping {format}: not encodable by this libvips build — {}",
                detail.trim()
            );
        }
    }
}

#[test]
fn test_tiff_save_produces_output() {
    assert_encodes_without_property_error("tiff");
}

/// TIFF is the one format whose compression depends on the requested quality:
/// `save.rs` picks LZW at 100 to keep the output lossless and JPEG below that.
/// Nothing exercised either branch, so this asserts the actual property — the
/// pixels survive at 100 and do not below it — rather than just that bytes come
/// back.
#[test]
fn test_tiff_is_lossless_at_max_quality_and_lossy_below() {
    init_vips();
    if !save::is_format_supported("tiff") {
        eprintln!("skipping: this libvips build has no TIFF saver");
        return;
    }

    // Noise: a flat image survives lossy compression intact and would hide the
    // difference between the two branches.
    let source_bytes = create_textured_image(64, 64);
    let expected = decode_rgba(&image_from(source_bytes.clone()));

    let lossless = save::save_image(image_from(source_bytes.clone()), "tiff", 100).unwrap();
    let lossy = save::save_image(image_from(source_bytes.clone()), "tiff", 60).unwrap();

    let lossless_pixels = collect_rgba_pixels(&decode_rgba(&image_from(lossless)));
    let lossy_pixels = collect_rgba_pixels(&decode_rgba(&image_from(lossy)));
    let expected_pixels = collect_rgba_pixels(&expected);

    assert_eq!(
        lossless_pixels, expected_pixels,
        "quality 100 should select LZW and round-trip every pixel"
    );
    assert_ne!(
        lossy_pixels, expected_pixels,
        "quality 60 should select JPEG compression, which is lossy"
    );
}

/// Each format's ceiling is the encoder's, not the container's. libjpeg refuses
/// anything over `JPEG_MAX_DIMENSION` (65,500) even though a JPEG's 16-bit size
/// fields could describe 65,535, so taking the wider number let a result in that
/// 35-pixel band skip the fit and fail in the encoder anyway.
#[test]
fn format_ceilings_match_the_encoders_own_limits() {
    use crate::processing::save::format_max_dimension;

    assert_eq!(format_max_dimension("jpeg"), Some(65_500));
    assert_eq!(format_max_dimension("jpg"), Some(65_500), "the alias shares the limit");
    // GIF really is bounded by its 16-bit fields.
    assert_eq!(format_max_dimension("gif"), Some(65_535));
    // libwebp's own cap, and the HEIF family's.
    assert_eq!(format_max_dimension("webp"), Some(16_383));
    assert_eq!(format_max_dimension("avif"), Some(16_384));
    assert_eq!(format_max_dimension("heif"), Some(16_384));
    // PNG and TIFF address far more than any request will produce.
    assert_eq!(format_max_dimension("png"), None);
    assert_eq!(format_max_dimension("tiff"), None);
}
