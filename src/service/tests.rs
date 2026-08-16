//! Service-layer tests: cache identity, limit resolution, and the mapping from
//! internal failures to what a client is told.

use super::*;
use crate::config::Config;
use crate::fetch::FetchError;
use crate::limits::MaxResultDimension;
use crate::processing::animation::LoadPlan;
use crate::processing::options::{Crop, OptionDefaults};
use crate::processing::save::SaveError;
use crate::processing::transform::TransformError;
use crate::service::cache_key::processed_cache_key;
use crate::service::security::{checked_source_pixel_count, resolve_max_result_dimension};
use crate::service::source::{rescale_crop, swaps_axes};
use std::error::Error as _;

fn key_parts(path: &str) -> CacheKeyParts<'_> {
    CacheKeyParts {
        path,
        source_url: path,
        default_format: DefaultOutputFormat::Source,
        has_explicit_format: false,
        is_raw: false,
        max_result_dimension: None,
        max_animation_frames: None,
        max_animation_frame_resolution: None,
        max_src_resolution: None,
        max_src_file_size: None,
        allowed_mime_types: None,
        watermark_path: None,
        option_defaults: None,
        negotiated_format: None,
        client_hints: None,
    }
}

#[test]
fn cache_keys_are_namespaced_by_the_effective_result_limit() {
    let path = "/unsafe/resize:fit:4000:4000/example";
    let unlimited = processed_cache_key(key_parts(path));
    let limited = processed_cache_key(CacheKeyParts {
        max_result_dimension: Some("1000".parse().unwrap()),
        ..key_parts(path)
    });
    let raised = processed_cache_key(CacheKeyParts {
        max_result_dimension: Some("8192".parse().unwrap()),
        ..key_parts(path)
    });

    // A disk cache outlives the config that filled it. Entries stored under
    // one ceiling must not be served under another, or a request that the
    // limit should refuse comes straight back out of the cache.
    assert_ne!(unlimited, limited);
    assert_ne!(limited, raised);

    // Turning the feature on must not invalidate caches that never use it.
    assert_eq!(unlimited, processed_cache_key(key_parts(path)));
}

/// Every ceiling that can reject a response has to reach the key, not just the
/// result dimension. Tightening an animation limit while a persistent cache
/// still holds the looser result would otherwise be answered from the cache
/// before the limit ever ran.
#[test]
fn cache_keys_are_namespaced_by_the_effective_animation_limits() {
    let path = "/unsafe/resize:fit:800:600/example.gif";

    let unlimited = processed_cache_key(key_parts(path));
    let few_frames = processed_cache_key(CacheKeyParts {
        max_animation_frames: Some("10".parse().unwrap()),
        ..key_parts(path)
    });
    let more_frames = processed_cache_key(CacheKeyParts {
        max_animation_frames: Some("50".parse().unwrap()),
        ..key_parts(path)
    });
    assert_ne!(unlimited, few_frames);
    assert_ne!(few_frames, more_frames);

    let small_frames = processed_cache_key(CacheKeyParts {
        max_animation_frame_resolution: Some("1.0".parse().unwrap()),
        ..key_parts(path)
    });
    let large_frames = processed_cache_key(CacheKeyParts {
        max_animation_frame_resolution: Some("9.0".parse().unwrap()),
        ..key_parts(path)
    });
    assert_ne!(unlimited, small_frames);
    assert_ne!(small_frames, large_frames);

    // The two limits are independent, so setting one must not collide with the
    // other's namespace.
    assert_ne!(few_frames, small_frames);

    // Deployments that configure neither keep the keys they already have.
    assert_eq!(unlimited, processed_cache_key(key_parts(path)));
}

/// A `Width: 320` request and a `Width: 1280` request are the same URL and
/// different images, so they cannot share an entry.
#[test]
fn client_hint_dimensions_get_their_own_cache_entries() {
    let path = "/unsafe/example";

    let narrow = processed_cache_key(CacheKeyParts {
        client_hints: Some((320, 1000)),
        ..key_parts(path)
    });
    let wide = processed_cache_key(CacheKeyParts {
        client_hints: Some((1280, 1000)),
        ..key_parts(path)
    });
    let retina = processed_cache_key(CacheKeyParts {
        client_hints: Some((320, 2000)),
        ..key_parts(path)
    });

    assert_ne!(narrow, wide);
    assert_ne!(narrow, retina, "the device pixel ratio changes the bytes too");
    // With hints off, the key is untouched, so enabling the feature does not
    // invalidate a cache that never used it.
    assert_eq!(
        processed_cache_key(key_parts(path)),
        processed_cache_key(key_parts(path))
    );
}

#[test]
fn negotiated_formats_get_their_own_cache_entries() {
    let path = "/unsafe/resize:fit:100:100/example";

    let plain = processed_cache_key(key_parts(path));
    let webp = processed_cache_key(CacheKeyParts {
        negotiated_format: Some("webp"),
        ..key_parts(path)
    });
    let avif = processed_cache_key(CacheKeyParts {
        negotiated_format: Some("avif"),
        ..key_parts(path)
    });

    // One URL now produces different bytes for different clients. Sharing an
    // entry between them would hand a client a format its Accept header said
    // it could not read.
    assert_ne!(plain, webp);
    assert_ne!(webp, avif);

    // An explicit format in the URL is not negotiable, so it keeps the bare
    // path and stays shared across clients.
    let explicit = processed_cache_key(CacheKeyParts {
        has_explicit_format: true,
        negotiated_format: Some("webp"),
        ..key_parts("/unsafe/format:png/example")
    });
    assert_eq!(explicit, "/unsafe/format:png/example");
}

/// A real JPEG with an APP1/Exif segment carrying just the Orientation tag,
/// spliced in after SOI. Built by hand so the fixture needs no tooling and
/// no checked-in binary.
fn exif_orientation_jpeg(orientation: u16) -> Vec<u8> {
    use image::{ImageBuffer, ImageFormat, Rgb};

    let mut base = Vec::new();
    ImageBuffer::<Rgb<u8>, Vec<u8>>::from_pixel(8, 4, Rgb([10, 20, 30]))
        .write_to(&mut std::io::Cursor::new(&mut base), ImageFormat::Jpeg)
        .unwrap();

    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II"); // little-endian
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 starts here
    tiff.extend_from_slice(&1u16.to_le_bytes()); // one entry
    tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // Orientation
    tiff.extend_from_slice(&3u16.to_le_bytes()); // SHORT
    tiff.extend_from_slice(&1u32.to_le_bytes()); // count
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&0u16.to_le_bytes()); // value field is 4 bytes wide
    tiff.extend_from_slice(&0u32.to_le_bytes()); // no further IFD

    let mut app1 = Vec::from(&b"Exif\0\0"[..]);
    app1.extend_from_slice(&tiff);

    let mut out = Vec::new();
    out.extend_from_slice(&base[..2]); // SOI
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&base[2..]);
    out
}

#[test]
fn crop_regions_are_rewritten_for_a_reduced_decode() {
    // A crop names source pixels, so a source decoded at a quarter size
    // needs the region quartered with it. Exercised through the function
    // the request path actually calls, not a hand-rolled equivalent.
    let original = (2000, 1600);
    let shrunk = (500, 400);

    let mut options = ParsedOptions {
        crop: Some(Crop {
            width: 1000.0,
            height: 800.0,
            gravity: None,
        }),
        ..ParsedOptions::default()
    };
    rescale_crop(&mut options, original, shrunk);
    let crop = options.crop.unwrap();
    assert_eq!((crop.width, crop.height), (250.0, 200.0));

    // A zero extent already means "all of it" and must stay that way,
    // otherwise it would be pinned to one pixel.
    let mut options = ParsedOptions {
        crop: Some(Crop {
            width: 0.0,
            height: 800.0,
            gravity: None,
        }),
        ..ParsedOptions::default()
    };
    rescale_crop(&mut options, original, shrunk);
    let crop = options.crop.unwrap();
    assert_eq!((crop.width, crop.height), (0.0, 200.0));

    // Rounding up: a region that does not divide evenly must not come back
    // smaller than the resize target needs.
    let mut options = ParsedOptions {
        crop: Some(Crop {
            width: 999.0,
            height: 3.0,
            gravity: None,
        }),
        ..ParsedOptions::default()
    };
    rescale_crop(&mut options, original, shrunk);
    let crop = options.crop.unwrap();
    assert_eq!((crop.width, crop.height), (250.0, 1.0));
}

/// A crop that mixes a fraction with an absolute extent has to be decided per
/// axis. Skipping both because one was fractional left the absolute extent
/// addressing coordinates in a source that had already been decoded smaller.
#[test]
fn a_mixed_crop_rescales_only_its_absolute_axis() {
    let mut options = ParsedOptions {
        crop: Some(Crop {
            width: 0.5,
            height: 1000.0,
            gravity: None,
        }),
        ..ParsedOptions::default()
    };
    rescale_crop(&mut options, (2000, 1600), (500, 400));

    let crop = options.crop.unwrap();
    assert_eq!(crop.width, 0.5, "a fraction is already relative to what was decoded");
    assert_eq!(crop.height, 250.0, "an absolute extent has to follow the decode");
}

#[test]
fn a_fractional_crop_survives_a_reduced_decode_untouched() {
    // A crop expressed as a fraction is measured against whatever was decoded,
    // so rescaling it would shrink it twice.
    let mut options = ParsedOptions {
        crop: Some(Crop {
            width: 0.5,
            height: 0.5,
            gravity: None,
        }),
        ..ParsedOptions::default()
    };
    rescale_crop(&mut options, (2000, 1600), (500, 400));
    let crop = options.crop.unwrap();
    assert_eq!((crop.width, crop.height), (0.5, 0.5));
}

#[test]
fn axis_swapping_orientations_are_recognised() {
    // The wiring that feeds load_shrink_factor its dimensions. Without this,
    // a transposed source is measured on its stored shape and over-shrunk:
    // the factor test proves the arithmetic, this proves it is reached.
    let rotating = ParsedOptions {
        auto_rotate: true,
        ..ParsedOptions::default()
    };
    let fixed = ParsedOptions {
        auto_rotate: false,
        ..ParsedOptions::default()
    };

    // A JPEG carrying orientation 6, which transposes the image.
    let rotated_jpeg = Bytes::from(exif_orientation_jpeg(6));
    assert!(
        swaps_axes(&rotating, &rotated_jpeg),
        "orientation 6 transposes and must swap the axes"
    );
    assert!(
        !swaps_axes(&fixed, &rotated_jpeg),
        "auto_rotate:false leaves the stored shape alone"
    );

    // Orientation 3 is a 180 rotation: same shape, no swap.
    let upright_jpeg = Bytes::from(exif_orientation_jpeg(3));
    assert!(!swaps_axes(&rotating, &upright_jpeg));

    // No EXIF at all.
    assert!(!swaps_axes(&rotating, &Bytes::from_static(b"not an image")));
}

/// A JPEG whose EXIF block carries an orientation in IFD0 and a real embedded
/// thumbnail in IFD1 — the shape `enforce_thumbnail` looks for. Built by hand
/// like `exif_orientation_jpeg`, so the fixture needs no checked-in binary.
fn thumbnail_jpeg(orientation: u16, thumb: &[u8]) -> Vec<u8> {
    use image::{ImageBuffer, ImageFormat, Rgb};

    let mut base = Vec::new();
    ImageBuffer::<Rgb<u8>, Vec<u8>>::from_pixel(80, 40, Rgb([10, 20, 30]))
        .write_to(&mut std::io::Cursor::new(&mut base), ImageFormat::Jpeg)
        .unwrap();

    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II"); // little-endian
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 starts here

    // IFD0: the orientation, then a link to IFD1.
    let ifd1_offset = 8u32 + 2 + 12 + 4;
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // Orientation
    tiff.extend_from_slice(&3u16.to_le_bytes()); // SHORT
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&0u16.to_le_bytes()); // value field is 4 bytes wide
    tiff.extend_from_slice(&ifd1_offset.to_le_bytes());

    // IFD1: where the thumbnail lives and how long it is.
    let thumb_offset = ifd1_offset + 2 + 2 * 12 + 4;
    tiff.extend_from_slice(&2u16.to_le_bytes());
    tiff.extend_from_slice(&0x0201u16.to_le_bytes()); // JPEGInterchangeFormat
    tiff.extend_from_slice(&4u16.to_le_bytes()); // LONG
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&thumb_offset.to_le_bytes());
    tiff.extend_from_slice(&0x0202u16.to_le_bytes()); // JPEGInterchangeFormatLength
    tiff.extend_from_slice(&4u16.to_le_bytes()); // LONG
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&(thumb.len() as u32).to_le_bytes());
    tiff.extend_from_slice(&0u32.to_le_bytes()); // no further IFD
    tiff.extend_from_slice(thumb);

    let mut app1 = Vec::from(&b"Exif\0\0"[..]);
    app1.extend_from_slice(&tiff);

    let mut out = Vec::new();
    out.extend_from_slice(&base[..2]); // SOI
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&base[2..]);
    out
}

#[test]
fn thumbnail_substitution_is_gated_and_keeps_the_source_in_view() {
    use crate::processing::options::{Resize, ResizingType};
    crate::processing::tests::tests_support::init_vips();

    // An 80x40 source carrying a 16x8 thumbnail.
    let mut thumb = Vec::new();
    image::ImageBuffer::<image::Rgb<u8>, Vec<u8>>::from_pixel(16, 8, image::Rgb([40, 50, 60]))
        .write_to(&mut std::io::Cursor::new(&mut thumb), image::ImageFormat::Jpeg)
        .unwrap();

    let request = |orientation: u16, w: u32, h: u32| -> (Bytes, ParsedOptions) {
        let source = Bytes::from(thumbnail_jpeg(orientation, &thumb));
        let options = ParsedOptions {
            enforce_thumbnail: true,
            auto_rotate: true,
            resize: Some(Resize {
                resizing_type: ResizingType::Fit,
                width: w,
                height: h,
            }),
            ..ParsedOptions::default()
        };
        (source, options)
    };

    // Covered: the thumbnail stands in, while metadata keeps reading the
    // source and the ceilings keep measuring it.
    let (source, options) = request(1, 10, 5);
    let opened = open_source(&source, &options, "jpeg").unwrap();
    assert_eq!((opened.image.get_width(), opened.image.get_height()), (16, 8));
    assert_ne!(opened.decode_bytes, source, "the pixels come from the thumbnail");
    assert_eq!(opened.metadata_bytes, source, "metadata still reads the source");
    let ceiling = opened.constraint_image();
    assert_eq!(
        (ceiling.get_width(), ceiling.get_height()),
        (80, 40),
        "the ceilings measure the source, not the stand-in"
    );

    // Beyond the thumbnail: the full image is opened instead.
    let (source, options) = request(1, 50, 20);
    let opened = open_source(&source, &options, "jpeg").unwrap();
    assert_eq!((opened.image.get_width(), opened.image.get_height()), (80, 40));
    assert!(opened.original.is_none());

    // Orientation 6 shows the 16x8 thumbnail as 8x16, so a 6x10 request fits
    // only the transposed shape and a 10x6 request only the stored one. The
    // gate has to judge the shape the viewer gets.
    let (source, options) = request(6, 6, 10);
    let opened = open_source(&source, &options, "jpeg").unwrap();
    assert!(opened.original.is_some(), "covered once the axes are swapped");

    let (source, options) = request(6, 10, 6);
    let opened = open_source(&source, &options, "jpeg").unwrap();
    assert!(opened.original.is_none(), "the stored axes no longer pass the gate");
}

#[test]
fn raw_cache_keys_ignore_the_result_limit() {
    // The raw path inserts under the bare path, so the lookup key has to match
    // it exactly or raw requests miss the cache forever and refetch the source
    // every time.
    let path = "/unsafe/raw/example";
    let limit = Some("1000".parse::<MaxResultDimension>().unwrap());

    assert_eq!(
        processed_cache_key(CacheKeyParts {
            is_raw: true,
            max_result_dimension: limit,
            ..key_parts(path)
        }),
        path
    );
    assert_eq!(
        processed_cache_key(CacheKeyParts {
            is_raw: true,
            ..key_parts(path)
        }),
        path
    );
}

#[test]
fn max_result_dimension_override_requires_security_options() {
    let request_limit = "1000".parse::<MaxResultDimension>().unwrap();
    let server_limit = "4000".parse::<MaxResultDimension>().unwrap();

    let parsed_options = ParsedOptions {
        max_result_dimension: Some(request_limit),
        ..ParsedOptions::default()
    };

    let mut config = Config::new(vec![0u8; 32], vec![0u8; 32]);
    config.max_result_dimension = Some(server_limit);

    // Locked down: the URL cannot set its own ceiling, so the server's stands.
    config.allow_security_options = false;
    assert_eq!(
        resolve_max_result_dimension(&config, &parsed_options),
        Some(server_limit)
    );

    // Opted in: the request wins, matching how max_src_* already behave.
    config.allow_security_options = true;
    assert_eq!(
        resolve_max_result_dimension(&config, &parsed_options),
        Some(request_limit)
    );

    // No server limit and no opt-in means no ceiling at all.
    config.allow_security_options = false;
    config.max_result_dimension = None;
    assert_eq!(resolve_max_result_dimension(&config, &parsed_options), None);
}

#[test]
fn skip_processing_only_applies_when_the_output_matches_the_source() {
    let listed = |formats: &[&str], requested: Option<&str>| ParsedOptions {
        skip_processing: formats.iter().map(|f| f.to_string()).collect(),
        format: requested.map(str::to_string),
        ..ParsedOptions::default()
    };

    assert!(can_skip_processing(
        &listed(&["png"], None),
        Some("png"),
        DefaultOutputFormat::Source
    ));
    assert!(can_skip_processing(
        &listed(&["png"], Some("png")),
        Some("png"),
        DefaultOutputFormat::Source
    ));
    // jpg and jpeg name the same format.
    assert!(can_skip_processing(
        &listed(&["jpg"], None),
        Some("jpeg"),
        DefaultOutputFormat::Source
    ));

    // A conversion is processing, however the source is listed.
    assert!(!can_skip_processing(
        &listed(&["png"], Some("webp")),
        Some("png"),
        DefaultOutputFormat::Source
    ));
    // A format that was not listed is processed as usual.
    assert!(!can_skip_processing(
        &listed(&["png"], None),
        Some("jpeg"),
        DefaultOutputFormat::Source
    ));
    // Nothing listed means nothing skipped.
    assert!(!can_skip_processing(
        &ParsedOptions::default(),
        Some("png"),
        DefaultOutputFormat::Source
    ));
}

#[test]
fn multi_page_load_plans_are_only_built_when_they_change_something() {
    // The default load already reads one page, so a still-image request must
    // produce no loader options at all — passing `n` to a loader that has no
    // such property makes libvips reject the whole call.
    let defaults = ParsedOptions::default();
    assert_eq!(LoadPlan::resolve(&defaults, Some("jpeg"), "jpeg"), None);
    assert_eq!(LoadPlan::resolve(&defaults, Some("gif"), "jpeg"), None);

    // An animated source into an animated output reads every frame.
    let plan = LoadPlan::resolve(&defaults, Some("gif"), "gif").expect("animation is read whole");
    assert_eq!(plan.as_load_options(), "page=0,n=-1");

    // Explicitly disabling animation collapses it to one frame again.
    let still = ParsedOptions {
        disable_animation: true,
        ..ParsedOptions::default()
    };
    assert_eq!(LoadPlan::resolve(&still, Some("gif"), "gif"), None);

    // page/pages address a multi-page document.
    let paged = ParsedOptions {
        page: Some(2),
        pages: Some(3),
        ..ParsedOptions::default()
    };
    let plan = LoadPlan::resolve(&paged, Some("pdf"), "png").expect("pages are requested");
    assert_eq!(plan.as_load_options(), "page=2,n=3");
}

#[test]
fn the_animation_frame_limit_caps_what_is_decoded() {
    let options = ParsedOptions {
        max_animation_frames: Some("4".parse().unwrap()),
        ..ParsedOptions::default()
    };

    let plan = LoadPlan::resolve(&options, Some("gif"), "gif").expect("a limit always produces a plan");
    assert_eq!(plan.as_load_options(), "page=0,n=4");

    // A request for more frames than the limit allows is cut down to it.
    let options = ParsedOptions {
        pages: Some(100),
        max_animation_frames: Some("4".parse().unwrap()),
        ..ParsedOptions::default()
    };
    let plan = LoadPlan::resolve(&options, Some("gif"), "gif").expect("a limit always produces a plan");
    assert_eq!(plan.as_load_options(), "page=0,n=4");
}

#[test]
fn configured_option_defaults_seed_the_parse() {
    use crate::processing::options::{parse_all_options_with_defaults, ProcessingOption};

    let defaults = OptionDefaults {
        auto_rotate: false,
        strip_metadata: true,
        enforce_thumbnail: true,
        quality: Some(60),
        ..OptionDefaults::default()
    };

    let parsed = parse_all_options_with_defaults(Vec::new(), defaults).expect("defaults parse");
    assert!(!parsed.auto_rotate);
    assert_eq!(parsed.save.strip_metadata, Some(true));
    assert!(parsed.enforce_thumbnail);
    assert_eq!(parsed.quality, Some(60));

    // The URL always wins, because it is applied on top of the defaults.
    let overridden = parse_all_options_with_defaults(
        vec![
            ProcessingOption {
                name: "auto_rotate".to_string(),
                args: vec!["1".to_string()],
            },
            ProcessingOption {
                name: "strip_metadata".to_string(),
                args: vec!["0".to_string()],
            },
            ProcessingOption {
                name: "quality".to_string(),
                args: vec!["90".to_string()],
            },
        ],
        defaults,
    )
    .expect("overrides parse");
    assert!(overridden.auto_rotate);
    assert_eq!(overridden.save.strip_metadata, Some(false));
    assert_eq!(overridden.quality, Some(90));
}

#[test]
fn fetch_size_error_has_centralized_http_mapping() {
    let error = ServiceError::from(FetchError::SourceTooLarge {
        limit: 1024,
        actual: Some(2048),
    });

    assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        error.message(),
        "Source image exceeds the maximum allowed size of 1024 bytes"
    );
    assert!(matches!(
        error,
        ServiceError::Fetch(FetchError::SourceTooLarge {
            limit: 1024,
            actual: Some(2048)
        })
    ));
}

#[tokio::test]
async fn fetch_request_error_does_not_expose_network_details() {
    let source = reqwest::Client::new()
        .get("not_a_valid_url")
        .send()
        .await
        .expect_err("invalid URL should fail");
    let error = ServiceError::from(FetchError::Request(source));

    assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error.message(), "Error fetching image");
    assert!(error.source().is_some());
}

#[tokio::test]
async fn blocking_task_failure_maps_to_internal_server_error() {
    let source = tokio::task::spawn_blocking(|| panic!("test blocking-task panic"))
        .await
        .expect_err("panicking task should return a join error");
    let error = ServiceError::BlockingTask {
        operation: "test image operation",
        source,
    };

    assert_eq!(error.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(error.message(), "Image operation failed");
    assert!(error.source().is_some());
}

#[test]
fn option_parse_error_has_centralized_http_mapping() {
    use crate::processing::options::OptionParseError;

    let error = ServiceError::from(OptionParseError::InvalidValue(
        "quality option requires one argument".to_string(),
    ));

    assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error.message(), "quality option requires one argument");
}

#[test]
fn source_url_error_uses_safe_client_message() {
    use base64::Engine as _;

    let source = crate::url::SourceUrlInfo::Base64 {
        encoded_url: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0xff]),
    }
    .decode()
    .expect_err("invalid UTF-8 should fail");
    let error = ServiceError::from(source);

    assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error.message(), "Error decoding URL");
    assert!(error.source().is_some());
}

#[test]
fn processing_error_preserves_vips_source_and_uses_safe_client_message() {
    let transform_error = TransformError::Vips {
        operation: "test resize",
        source: libvips::error::Error::ResizeError,
    };
    let error = ServiceError::from(ProcessingError::from(transform_error));

    assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error.message(), "Error processing image");
    assert!(error.source().is_some());
}

#[test]
fn encoder_failure_maps_to_internal_server_error() {
    let save_error = SaveError::Vips {
        format: "jpeg",
        source: libvips::error::Error::JpegsaveBufferError,
    };
    let error = ServiceError::from(ProcessingError::from(save_error));

    assert_eq!(error.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(error.message(), "Failed to encode image");
    assert!(error.source().is_some());
}

#[test]
fn an_upstream_failure_is_reported_as_such() {
    // A 404 from the origin used to reach the caller as "failed to decode
    // source image", which pointed at the wrong thing entirely.
    let error = ServiceError::from(FetchError::UpstreamStatus { status: 404 });
    assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error.message(), "Source responded with status 404");
}

#[test]
fn pixel_count_does_not_overflow_i32_sized_dimensions() {
    assert_eq!(checked_source_pixel_count(50_000, 50_000).unwrap(), 2_500_000_000);
}

#[test]
fn pixel_count_rejects_negative_dimensions() {
    assert!(checked_source_pixel_count(-1, 100).is_err());
    assert!(checked_source_pixel_count(100, -1).is_err());
}

#[test]
fn fixed_default_format_is_resolved_without_sniffing() {
    assert_eq!(
        default_output_format(DefaultOutputFormat::Jpeg, b"not an image"),
        Some("jpeg")
    );
    assert_eq!(
        default_output_format(DefaultOutputFormat::Heif, b"not an image"),
        Some("heif")
    );
}

#[test]
fn implicit_format_cache_keys_include_the_configured_default() {
    let source_key = processed_cache_key(key_parts("/unsafe/example"));
    let jpeg_key = processed_cache_key(CacheKeyParts {
        default_format: DefaultOutputFormat::Jpeg,
        ..key_parts("/unsafe/example")
    });
    let explicit_key = processed_cache_key(CacheKeyParts {
        default_format: DefaultOutputFormat::Jpeg,
        has_explicit_format: true,
        ..key_parts("/unsafe/format:png/example")
    });

    assert_ne!(source_key, jpeg_key);
    assert_eq!(explicit_key, "/unsafe/format:png/example");
}

/// A crop's position is measured in the same pixels as its size, so a reduced
/// decode has to move the gravity offset with the extents. Rewriting only the
/// extents left an absolute offset pointing four times too far into a source
/// decoded at a quarter size.
#[test]
fn crop_gravity_offsets_are_rewritten_for_a_reduced_decode() {
    use crate::processing::options::{Gravity, GravityType};

    let original = (2000, 1600);
    let shrunk = (500, 400);

    let with_gravity = |gravity: Gravity| {
        let mut options = ParsedOptions {
            crop: Some(Crop {
                width: 1000.0,
                height: 800.0,
                gravity: Some(gravity),
            }),
            ..ParsedOptions::default()
        };
        rescale_crop(&mut options, original, shrunk);
        options.crop.unwrap().gravity.unwrap()
    };

    // Absolute offsets are pixel counts and scale per axis.
    let scaled = with_gravity(Gravity {
        kind: GravityType::NorthWest,
        x: 400.0,
        y: 200.0,
    });
    assert_eq!((scaled.x, scaled.y), (100.0, 50.0));

    // Anything below 1 is a fraction of the axis and already scales itself.
    let fractional = with_gravity(Gravity {
        kind: GravityType::NorthWest,
        x: 0.25,
        y: 0.5,
    });
    assert_eq!((fractional.x, fractional.y), (0.25, 0.5));

    // A focus point reads both arguments as 0..1 coordinates throughout.
    let focus = with_gravity(Gravity {
        kind: GravityType::FocusPoint,
        x: 0.5,
        y: 0.25,
    });
    assert_eq!((focus.x, focus.y), (0.5, 0.25));
}

/// The crop falls back to the request's `gravity` when it names none, but that
/// same field positions the *resized* image for a fill — which is not measured
/// in source pixels. Scaling it in place would fix the crop by breaking the fill,
/// so the scaled copy is written into the crop's own gravity.
#[test]
fn rescaling_a_crop_leaves_the_requests_own_gravity_alone() {
    use crate::processing::options::{Gravity, GravityType};

    let request_gravity = Gravity {
        kind: GravityType::NorthWest,
        x: 400.0,
        y: 200.0,
    };
    let mut options = ParsedOptions {
        crop: Some(Crop {
            width: 1000.0,
            height: 800.0,
            gravity: None,
        }),
        gravity: Some(request_gravity),
        ..ParsedOptions::default()
    };

    rescale_crop(&mut options, (2000, 1600), (500, 400));

    let crop_gravity = options.crop_gravity();
    assert_eq!((crop_gravity.x, crop_gravity.y), (100.0, 50.0));

    let fill_gravity = options.fill_gravity();
    assert_eq!(
        (fill_gravity.x, fill_gravity.y),
        (400.0, 200.0),
        "the fill window positions the resized image and must keep its own offsets"
    );
}

/// An alias picks the right encoder, so it must also pick the right MIME type.
/// `format:tif` selected TIFF and then fell through `format_to_content_type`'s
/// catch-all, labelling TIFF bytes as JPEG.
#[test]
fn format_aliases_resolve_to_one_canonical_name() {
    use crate::processing::save::canonical_format_name;
    use crate::utils::format_to_content_type;

    for (alias, canonical, mime) in [
        ("tif", "tiff", "image/tiff"),
        ("tiff", "tiff", "image/tiff"),
        ("jpg", "jpeg", "image/jpeg"),
        ("heic", "heif", "image/heif"),
    ] {
        assert_eq!(canonical_format_name(alias), Some(canonical), "{alias}");
        assert_eq!(
            format_to_content_type(canonical_format_name(alias).unwrap()),
            mime,
            "{alias} must be described by its own media type"
        );
    }

    assert_eq!(canonical_format_name("not-a-format"), None);
}

/// The ceilings describing the *source* have to reach the key too. Every one is
/// checked after the cache lookup, and `raw` and `skip_processing` return source
/// bytes straight from the cache without reaching the checks at all — so a
/// tightened policy was simply outrun by the entry already stored.
#[test]
fn cache_keys_are_namespaced_by_the_effective_source_limits() {
    let path = "/unsafe/raw:1/example";
    let jpeg_only = vec!["image/jpeg".to_string()];
    let jpeg_and_png = vec!["image/jpeg".to_string(), "image/png".to_string()];

    let unlimited = processed_cache_key(key_parts(path));

    let by_resolution = processed_cache_key(CacheKeyParts {
        max_src_resolution: Some("10".parse().unwrap()),
        ..key_parts(path)
    });
    let tighter_resolution = processed_cache_key(CacheKeyParts {
        max_src_resolution: Some("5".parse().unwrap()),
        ..key_parts(path)
    });
    assert_ne!(unlimited, by_resolution);
    assert_ne!(by_resolution, tighter_resolution);

    let by_size = processed_cache_key(CacheKeyParts {
        max_src_file_size: Some("1048576".parse().unwrap()),
        ..key_parts(path)
    });
    assert_ne!(unlimited, by_size);
    assert_ne!(by_size, by_resolution);

    let by_mime = processed_cache_key(CacheKeyParts {
        allowed_mime_types: Some(&jpeg_only),
        ..key_parts(path)
    });
    let by_wider_mime = processed_cache_key(CacheKeyParts {
        allowed_mime_types: Some(&jpeg_and_png),
        ..key_parts(path)
    });
    assert_ne!(unlimited, by_mime);
    assert_ne!(by_mime, by_wider_mime, "widening the list is a policy change");

    // Reordering the environment variable is not a policy change, so it must
    // not cold-start the cache.
    let reordered = vec!["image/png".to_string(), "image/jpeg".to_string()];
    assert_eq!(
        by_wider_mime,
        processed_cache_key(CacheKeyParts {
            allowed_mime_types: Some(&reordered),
            ..key_parts(path)
        })
    );

    // A deployment that sets none of them keeps the keys it already had.
    assert_eq!(unlimited, processed_cache_key(key_parts(path)));
}

/// A `raw` response takes the bare path rather than the full processed key, so
/// the source limits have to be applied to that path too — it is the one shape
/// of response that returns origin bytes with no processing between them and the
/// client.
#[test]
fn a_raw_key_still_carries_the_source_limits() {
    let path = "/unsafe/raw:1/example";

    let plain = processed_cache_key(CacheKeyParts {
        is_raw: true,
        ..key_parts(path)
    });
    assert_eq!(plain, path, "an unrestricted deployment keeps the bare path");

    let restricted = processed_cache_key(CacheKeyParts {
        is_raw: true,
        max_src_resolution: Some("5".parse().unwrap()),
        ..key_parts(path)
    });
    assert_ne!(restricted, path);
    assert!(restricted.ends_with(path), "the path stays the tail of the key");

    // The result-side namespaces stay out of a raw key: nothing is processed,
    // so they cannot change these bytes.
    let with_result_limit = processed_cache_key(CacheKeyParts {
        is_raw: true,
        max_result_dimension: Some("100".parse().unwrap()),
        ..key_parts(path)
    });
    assert_eq!(with_result_limit, path);
}

/// A fixed `IMGFORGE_DEFAULT_FORMAT` is a standing instruction that every
/// response is that format, so a URL naming none is still asking for a
/// conversion. Reading an absent URL format as "same as the source" let
/// `skip_processing` hand back the original from a deployment configured to
/// serve only WebP.
#[test]
fn skip_processing_respects_a_fixed_default_format() {
    let listed = |formats: &[&str]| ParsedOptions {
        skip_processing: formats.iter().map(|f| f.to_string()).collect(),
        ..ParsedOptions::default()
    };

    // With the source's own format as the default, nothing is being converted.
    assert!(can_skip_processing(
        &listed(&["jpeg"]),
        Some("jpeg"),
        DefaultOutputFormat::Source
    ));

    // A fixed default that differs is a conversion, so the pipeline has to run.
    assert!(!can_skip_processing(
        &listed(&["jpeg"]),
        Some("jpeg"),
        "webp".parse().expect("webp is a valid default format")
    ));

    // A fixed default that matches the source is not.
    assert!(can_skip_processing(
        &listed(&["jpeg"]),
        Some("jpeg"),
        "jpeg".parse().expect("jpeg is a valid default format")
    ));

    // An explicit URL format still wins over the configured default.
    let mut explicit = listed(&["jpeg"]);
    explicit.format = Some("jpeg".to_string());
    assert!(can_skip_processing(
        &explicit,
        Some("jpeg"),
        "webp".parse().expect("webp is a valid default format")
    ));
}

/// `watermark:1` names no image of its own — the overlay comes from
/// `IMGFORGE_WATERMARK_PATH`. Repointing that setting changes the bytes of every
/// watermarked response while leaving every URL identical, so the old logo was
/// served until the entries aged out.
#[test]
fn cache_keys_follow_the_configured_watermark() {
    let path = "/unsafe/rs:fit:100:100/wm:0.5/example";

    let unwatermarked = processed_cache_key(key_parts(path));
    let logo = processed_cache_key(CacheKeyParts {
        watermark_path: Some("/etc/imgforge/logo.png"),
        ..key_parts(path)
    });
    let new_logo = processed_cache_key(CacheKeyParts {
        watermark_path: Some("/etc/imgforge/logo-2026.png"),
        ..key_parts(path)
    });

    assert_ne!(unwatermarked, logo);
    assert_ne!(logo, new_logo, "repointing the watermark must retire the old entries");

    // A deployment that configures none keeps the keys it had.
    assert_eq!(unwatermarked, processed_cache_key(key_parts(path)));
}

/// The configured defaults seed the parse, so `IMGFORGE_QUALITY` and its
/// neighbours change the bytes exactly as a URL option would. Unlike a release,
/// a config change carries no version bump to retire what it invalidates.
#[test]
fn cache_keys_follow_the_configured_option_defaults() {
    let path = "/unsafe/rs:fit:100:100/example";
    let unconfigured = processed_cache_key(key_parts(path));

    let with_quality = |quality: u8| {
        processed_cache_key(CacheKeyParts {
            option_defaults: Some(OptionDefaults {
                quality: Some(quality),
                ..OptionDefaults::default()
            }),
            ..key_parts(path)
        })
    };
    assert_ne!(unconfigured, with_quality(85));
    assert_ne!(
        with_quality(85),
        with_quality(20),
        "lowering the default must retire the old bytes"
    );

    // A flag is as byte-affecting as a number.
    let stripped = processed_cache_key(CacheKeyParts {
        option_defaults: Some(OptionDefaults {
            strip_metadata: true,
            ..OptionDefaults::default()
        }),
        ..key_parts(path)
    });
    assert_ne!(unconfigured, stripped);
    assert_ne!(
        stripped,
        with_quality(85),
        "different settings must not share a namespace"
    );

    // A deployment that changes none of them keeps the keys it had.
    assert_eq!(unconfigured, processed_cache_key(key_parts(path)));
}

/// The alias table lives in exactly one place. This used to be a second copy
/// that spelled out jpg/jpeg and heic/heif and simply omitted tif/tiff, so
/// `skip_processing:tif` never matched a TIFF source — the same drift that made
/// `format_quality:tif:20` miss its lookup, one file over.
#[test]
fn skip_processing_understands_every_format_alias() {
    let listed = |format: &str| ParsedOptions {
        skip_processing: vec![format.to_string()],
        ..ParsedOptions::default()
    };

    for (alias, source) in [
        ("tif", "tiff"),
        ("tiff", "tiff"),
        ("jpg", "jpeg"),
        ("jpeg", "jpeg"),
        ("heic", "heif"),
        ("heif", "heif"),
        ("png", "png"),
    ] {
        assert!(
            can_skip_processing(&listed(alias), Some(source), DefaultOutputFormat::Source),
            "skip_processing:{alias} should match a {source} source"
        );
    }

    // A different format still does not match.
    assert!(!can_skip_processing(
        &listed("tif"),
        Some("png"),
        DefaultOutputFormat::Source
    ));
    // And a name that is no format at all only ever matches itself.
    assert!(!can_skip_processing(
        &listed("notaformat"),
        Some("png"),
        DefaultOutputFormat::Source
    ));
}

/// A relative source reference names a different image the moment
/// `IMGFORGE_BASE_URL` changes, so the path alone cannot identify the entry:
/// the first request after an origin migration would be answered from the old
/// origin's bytes without ever fetching the new one.
#[test]
fn cache_keys_follow_the_resolved_source_url() {
    let path = "/unsafe/resize:fit:100:100/cat.jpg";

    let old_origin = processed_cache_key(CacheKeyParts {
        source_url: "https://old.example.com/cat.jpg",
        ..key_parts(path)
    });
    let new_origin = processed_cache_key(CacheKeyParts {
        source_url: "https://new.example.com/cat.jpg",
        ..key_parts(path)
    });
    assert_ne!(old_origin, new_origin);

    // A raw response resolves through the same base URL, so it needs the same
    // distinction — it is the one shape that hands back origin bytes directly.
    let raw_old = processed_cache_key(CacheKeyParts {
        is_raw: true,
        source_url: "https://old.example.com/cat.jpg",
        ..key_parts(path)
    });
    let raw_new = processed_cache_key(CacheKeyParts {
        is_raw: true,
        source_url: "https://new.example.com/cat.jpg",
        ..key_parts(path)
    });
    assert_ne!(raw_old, raw_new);

    // A URL that carries its own absolute source resolves to itself, so the
    // overwhelmingly common case pays nothing: no `src=` scope appears at all.
    let unscoped = processed_cache_key(key_parts(path));
    assert!(
        !unscoped.contains("src="),
        "a self-resolving URL should not be scoped: {unscoped}"
    );
    assert!(old_origin.contains("src=") && raw_old.contains("src="));
}
