//! URL directive names and their imgproxy-compatible aliases.
//!
//! Kept in one place so the dispatch table in [`super::parse_all_options`] and
//! the documentation stay in step: a new option is a constant here plus an arm
//! there.

/// Option name for resizing.
pub(super) const RESIZE: &str = "resize";
/// Shorthand for resize.
pub(super) const RESIZE_SHORT: &str = "rs";
/// Option name for resizing type.
pub(super) const RESIZING_TYPE: &str = "resizing_type";
/// Shorthand for resizing type.
pub(super) const RESIZING_TYPE_SHORT: &str = "rt";
/// Option name for size.
pub(super) const SIZE: &str = "size";
/// Shorthand for size.
pub(super) const SIZE_SHORT: &str = "s";
/// Option name for width.
pub(super) const WIDTH: &str = "width";
/// Shorthand for width.
pub(super) const WIDTH_SHORT: &str = "w";
/// Option name for height.
pub(super) const HEIGHT: &str = "height";
/// Shorthand for height.
pub(super) const HEIGHT_SHORT: &str = "h";
/// Option name for gravity.
pub(super) const GRAVITY: &str = "gravity";
/// Shorthand for gravity.
pub(super) const GRAVITY_SHORT: &str = "g";
/// Option name for quality.
pub(super) const QUALITY: &str = "quality";
/// Shorthand for quality.
pub(super) const QUALITY_SHORT: &str = "q";
/// Option name for format-specific quality.
pub(super) const FORMAT_QUALITY: &str = "format_quality";
/// Shorthand for format_quality.
pub(super) const FORMAT_QUALITY_SHORT: &str = "fq";
/// Option name for auto_rotate.
pub(super) const AUTO_ROTATE: &str = "auto_rotate";
/// Shorthand for auto_rotate.
pub(super) const AUTO_ROTATE_SHORT: &str = "ar";
/// Option name for background.
pub(super) const BACKGROUND: &str = "background";
/// Shorthand for background.
pub(super) const BACKGROUND_SHORT: &str = "bg";
/// Option name for enlarge.
pub(super) const ENLARGE: &str = "enlarge";
/// Shorthand for enlarge.
pub(super) const ENLARGE_SHORT: &str = "el";
/// Option name for extend.
pub(super) const EXTEND: &str = "extend";
/// Shorthand for extend.
pub(super) const EXTEND_SHORT: &str = "ex";
/// Option name for extend_aspect_ratio.
pub(super) const EXTEND_ASPECT_RATIO: &str = "extend_aspect_ratio";
/// Alternate spelling for extend_aspect_ratio.
pub(super) const EXTEND_ASPECT_RATIO_ALT: &str = "extend_ar";
/// Shorthand for extend_aspect_ratio.
pub(super) const EXTEND_ASPECT_RATIO_SHORT: &str = "exar";
/// Option name for padding.
pub(super) const PADDING: &str = "padding";
/// Shorthand for padding.
pub(super) const PADDING_SHORT: &str = "pd";
/// Option name for rotation.
pub(super) const ROTATE: &str = "rotate";
/// Shorthand for rotation.
pub(super) const ROTATE_SHORT: &str = "rot";
/// Option name for flip.
pub(super) const FLIP: &str = "flip";
/// Shorthand for flip.
pub(super) const FLIP_SHORT: &str = "fl";
/// Option name for raw.
pub(super) const RAW: &str = "raw";
/// Option name for blur.
pub(super) const BLUR: &str = "blur";
/// Shorthand for blur.
pub(super) const BLUR_SHORT: &str = "bl";
/// Option name for crop.
pub(super) const CROP: &str = "crop";
/// Shorthand for crop.
pub(super) const CROP_SHORT: &str = "c";
/// Option name for format.
pub(super) const FORMAT: &str = "format";
/// Shorthand for format.
pub(super) const FORMAT_SHORT: &str = "f";
/// Alternate shorthand for format.
pub(super) const FORMAT_EXT: &str = "ext";
/// Option name for max_src_resolution.
pub(super) const MAX_SRC_RESOLUTION: &str = "max_src_resolution";
/// Shorthand for max_src_resolution.
pub(super) const MAX_SRC_RESOLUTION_SHORT: &str = "msr";
/// Option name for trim.
pub(super) const TRIM: &str = "trim";
/// Shorthand for trim.
pub(super) const TRIM_SHORT: &str = "t";
/// Option name for max_result_dimension.
pub(super) const MAX_RESULT_DIMENSION: &str = "max_result_dimension";
/// Shorthand for max_result_dimension.
pub(super) const MAX_RESULT_DIMENSION_SHORT: &str = "mrd";
/// Option name for max_src_file_size.
pub(super) const MAX_SRC_FILE_SIZE: &str = "max_src_file_size";
/// Shorthand for max_src_file_size.
pub(super) const MAX_SRC_FILE_SIZE_SHORT: &str = "msfs";
/// Option name for max_animation_frames.
pub(super) const MAX_ANIMATION_FRAMES: &str = "max_animation_frames";
/// Shorthand for max_animation_frames.
pub(super) const MAX_ANIMATION_FRAMES_SHORT: &str = "maf";
/// Option name for max_animation_frame_resolution.
pub(super) const MAX_ANIMATION_FRAME_RESOLUTION: &str = "max_animation_frame_resolution";
/// Shorthand for max_animation_frame_resolution.
pub(super) const MAX_ANIMATION_FRAME_RESOLUTION_SHORT: &str = "mafr";
/// Option name for cache buster.
pub(super) const CACHEBUSTER: &str = "cachebuster";
/// Shorthand for cachebuster.
pub(super) const CACHEBUSTER_SHORT: &str = "cb";
/// Option name for dpr.
pub(super) const DPR: &str = "dpr";
/// Option name for min-width.
pub(super) const MIN_WIDTH: &str = "min-width";
/// Alternate spelling for min-width.
pub(super) const MIN_WIDTH_ALT: &str = "min_width";
/// Shorthand for min_width.
pub(super) const MIN_WIDTH_SHORT: &str = "mw";
/// Option name for min-height.
pub(super) const MIN_HEIGHT: &str = "min-height";
/// Alternate spelling for min-height.
pub(super) const MIN_HEIGHT_ALT: &str = "min_height";
/// Shorthand for min_height.
pub(super) const MIN_HEIGHT_SHORT: &str = "mh";
/// Option name for zoom.
pub(super) const ZOOM: &str = "zoom";
/// Shorthand for zoom.
pub(super) const ZOOM_SHORT: &str = "z";
/// Option name for sharpen.
pub(super) const SHARPEN: &str = "sharpen";
/// Shorthand for sharpen.
pub(super) const SHARPEN_SHORT: &str = "sh";
/// Option name for pixelate.
pub(super) const PIXELATE: &str = "pixelate";
/// Shorthand for pixelate.
pub(super) const PIXELATE_SHORT: &str = "pix";
/// Option name for watermark.
pub(super) const WATERMARK: &str = "watermark";
/// Shorthand for watermark.
pub(super) const WATERMARK_SHORT: &str = "wm";
/// Option name for watermark_url.
pub(super) const WATERMARK_URL: &str = "watermark_url";
/// Shorthand for watermark_url.
pub(super) const WATERMARK_URL_SHORT: &str = "wmu";
/// Option name for resizing_algorithm.
pub(super) const RESIZING_ALGORITHM: &str = "resizing_algorithm";
/// Shorthand for resizing_algorithm.
pub(super) const RESIZING_ALGORITHM_SHORT: &str = "ra";
/// Option name for background_alpha.
pub(super) const BACKGROUND_ALPHA: &str = "background_alpha";
/// Shorthand for background_alpha.
pub(super) const BACKGROUND_ALPHA_SHORT: &str = "bga";
/// Option name for adjust.
pub(super) const ADJUST: &str = "adjust";
/// Shorthand for adjust.
pub(super) const ADJUST_SHORT: &str = "a";
/// Option name for brightness.
pub(super) const BRIGHTNESS: &str = "brightness";
/// Shorthand for brightness.
pub(super) const BRIGHTNESS_SHORT: &str = "br";
/// Option name for contrast.
pub(super) const CONTRAST: &str = "contrast";
/// Shorthand for contrast.
pub(super) const CONTRAST_SHORT: &str = "co";
/// Option name for monochrome.
pub(super) const MONOCHROME: &str = "monochrome";
/// Shorthand for monochrome.
pub(super) const MONOCHROME_SHORT: &str = "mc";
/// Option name for duotone.
pub(super) const DUOTONE: &str = "duotone";
/// Shorthand for duotone.
pub(super) const DUOTONE_SHORT: &str = "dt";
/// Option name for colorize.
pub(super) const COLORIZE: &str = "colorize";
/// Shorthand for colorize.
pub(super) const COLORIZE_SHORT: &str = "col";
/// Option name for crop_aspect_ratio.
pub(super) const CROP_ASPECT_RATIO: &str = "crop_aspect_ratio";
/// Shorthand for crop_aspect_ratio.
pub(super) const CROP_ASPECT_RATIO_SHORT: &str = "car";
/// Option name for watermark_size.
pub(super) const WATERMARK_SIZE: &str = "watermark_size";
/// Shorthand for watermark_size.
pub(super) const WATERMARK_SIZE_SHORT: &str = "wms";
/// Option name for watermark_rotate.
pub(super) const WATERMARK_ROTATE: &str = "watermark_rotate";
/// Shorthand for watermark_rotate.
pub(super) const WATERMARK_ROTATE_SHORT: &str = "wmr";
/// Option name for saturation.
pub(super) const SATURATION: &str = "saturation";
/// Shorthand for saturation.
pub(super) const SATURATION_SHORT: &str = "sa";
/// Option name for max_bytes.
pub(super) const MAX_BYTES: &str = "max_bytes";
/// Shorthand for max_bytes.
pub(super) const MAX_BYTES_SHORT: &str = "mb";
/// Option name for strip_metadata.
pub(super) const STRIP_METADATA: &str = "strip_metadata";
/// Shorthand for strip_metadata.
pub(super) const STRIP_METADATA_SHORT: &str = "sm";
/// Option name for keep_copyright.
pub(super) const KEEP_COPYRIGHT: &str = "keep_copyright";
/// Shorthand for keep_copyright.
pub(super) const KEEP_COPYRIGHT_SHORT: &str = "kcr";
/// Option name for strip_color_profile.
pub(super) const STRIP_COLOR_PROFILE: &str = "strip_color_profile";
/// Shorthand for strip_color_profile.
pub(super) const STRIP_COLOR_PROFILE_SHORT: &str = "scp";
/// Option name for enforce_thumbnail.
pub(super) const ENFORCE_THUMBNAIL: &str = "enforce_thumbnail";
/// Shorthand for enforce_thumbnail.
pub(super) const ENFORCE_THUMBNAIL_SHORT: &str = "eth";
/// Option name for preserve_hdr.
pub(super) const PRESERVE_HDR: &str = "preserve_hdr";
/// Shorthand for preserve_hdr.
pub(super) const PRESERVE_HDR_SHORT: &str = "ph";
/// Option name for JPEG options.
pub(super) const JPEG_OPTIONS: &str = "jpeg_options";
/// Shorthand for JPEG options.
pub(super) const JPEG_OPTIONS_SHORT: &str = "jpgo";
/// Option name for PNG options.
pub(super) const PNG_OPTIONS: &str = "png_options";
/// Shorthand for PNG options.
pub(super) const PNG_OPTIONS_SHORT: &str = "pngo";
/// Option name for WebP options.
pub(super) const WEBP_OPTIONS: &str = "webp_options";
/// Shorthand for WebP options.
pub(super) const WEBP_OPTIONS_SHORT: &str = "webpo";
/// Option name for AVIF options.
pub(super) const AVIF_OPTIONS: &str = "avif_options";
/// Shorthand for AVIF options.
pub(super) const AVIF_OPTIONS_SHORT: &str = "avifo";
/// Option name for page.
pub(super) const PAGE: &str = "page";
/// Shorthand for page.
pub(super) const PAGE_SHORT: &str = "pg";
/// Option name for pages.
pub(super) const PAGES: &str = "pages";
/// Shorthand for pages.
pub(super) const PAGES_SHORT: &str = "pgs";
/// Option name for disable_animation.
pub(super) const DISABLE_ANIMATION: &str = "disable_animation";
/// Shorthand for disable_animation.
pub(super) const DISABLE_ANIMATION_SHORT: &str = "da";
/// Option name for skip_processing.
pub(super) const SKIP_PROCESSING: &str = "skip_processing";
/// Shorthand for skip_processing.
pub(super) const SKIP_PROCESSING_SHORT: &str = "skp";
/// Option name for expires.
pub(super) const EXPIRES: &str = "expires";
/// Shorthand for expires.
pub(super) const EXPIRES_SHORT: &str = "exp";
/// Option name for filename.
pub(super) const FILENAME: &str = "filename";
/// Shorthand for filename.
pub(super) const FILENAME_SHORT: &str = "fn";
/// Option name for return_attachment.
pub(super) const RETURN_ATTACHMENT: &str = "return_attachment";
/// Shorthand for return_attachment.
pub(super) const RETURN_ATTACHMENT_SHORT: &str = "att";
