//! Deriving the cache key for a processed response.
//!
//! The path alone is not enough. Anything that changes the bytes without
//! changing the URL — the configured default format, a negotiated format from
//! the client's `Accept`, the effective security ceilings — has to be part of
//! the key, or a persistent cache will hand one client an entry that was
//! produced for another.
//!
//! The security ceilings are here for a second reason, and it is the one that
//! matters most. Every one of them is checked *after* the cache lookup, so an
//! entry stored while a limit was loose keeps being served once the limit is
//! tightened: the request is answered before the check it should have failed.
//! Namespacing by the effective limit is what retires those entries, and it has
//! to cover the limits that describe the *source* as well as the result,
//! because `raw` and `skip_processing` return source bytes straight from the
//! cache without ever reaching `enforce_source_constraints`.

use crate::config::DefaultOutputFormat;
use crate::limits::{
    MaxAnimationFrameResolution, MaxAnimationFrames, MaxResultDimension, MaxSourceFileSize, MaxSourceResolution,
};
use crate::processing::options::OptionDefaults;
use sha2::{Digest, Sha256};
use std::borrow::Cow;

/// Everything outside the URL path that changes the response bytes, or that
/// decides whether the response may be produced at all.
#[derive(Debug, Clone, Copy)]
pub struct CacheKeyParts<'a> {
    pub path: &'a str,
    /// The URL the request actually resolves to, after `IMGFORGE_BASE_URL`.
    ///
    /// The path alone does not identify the bytes: with a base URL configured,
    /// the same relative reference points at a different origin the moment that
    /// setting changes, and the first request after a migration would otherwise
    /// be answered from the old origin's entry without ever fetching the new
    /// one. Including it also means an entry is tied to the address it was
    /// fetched from rather than to the shorthand that named it.
    pub source_url: &'a str,
    pub default_format: DefaultOutputFormat,
    pub has_explicit_format: bool,
    pub is_raw: bool,
    pub max_result_dimension: Option<MaxResultDimension>,
    /// Frame ceiling in force, when one is.
    pub max_animation_frames: Option<MaxAnimationFrames>,
    /// Per-frame pixel ceiling in force, when one is.
    pub max_animation_frame_resolution: Option<MaxAnimationFrameResolution>,
    /// Source-resolution ceiling in force, when one is.
    pub max_src_resolution: Option<MaxSourceResolution>,
    /// Source-size ceiling in force, when one is.
    pub max_src_file_size: Option<MaxSourceFileSize>,
    /// The configured `allowed_mime_types`, when the deployment restricts them.
    pub allowed_mime_types: Option<&'a [String]>,
    /// The server-side watermark file, when the request composites one.
    ///
    /// `watermark:1` names no image of its own: the overlay comes from
    /// `IMGFORGE_WATERMARK_PATH`. Repointing that setting changes the bytes of
    /// every watermarked response while leaving every URL identical, so without
    /// this the old logo is served until the entries age out.
    pub watermark_path: Option<&'a str>,
    /// The resolved URL of a `watermark_url` watermark, when the request
    /// carries one.
    ///
    /// A relative watermark reference resolves through `IMGFORGE_BASE_URL`
    /// exactly as the main source does, so the same URL composites a different
    /// overlay the moment that setting changes — while the path and the main
    /// source stay identical. Keying by the resolved form retires those
    /// entries the same way `source_url` does for the image itself.
    pub watermark_url: Option<&'a str>,
    /// Configured option defaults, when the deployment changes any of them.
    ///
    /// These seed the parse, so `IMGFORGE_QUALITY` and its neighbours change the
    /// bytes exactly as a URL option would — and unlike a release, a config
    /// change carries no version bump to retire the entries it invalidates.
    pub option_defaults: Option<OptionDefaults>,
    /// Format chosen from the request's `Accept` header, when one was.
    pub negotiated_format: Option<&'static str>,
    /// Dimensions the client's own hints contributed, when they were honoured.
    ///
    /// A `Width: 320` request and a `Width: 1280` request are the same URL and
    /// different images, so without this the first one's output is handed to
    /// the second.
    pub client_hints: Option<(u32, u32)>,
}

pub fn processed_cache_key<'a>(parts: CacheKeyParts<'a>) -> Cow<'a, str> {
    // A raw response is the untouched source, so the format decision and the
    // result ceiling cannot apply to it — but the limits describing the source
    // very much can, and they are the only thing standing between a tightened
    // policy and the bytes already in the cache.
    if parts.is_raw {
        return source_limits(parts, Cow::Owned(source_scoped(parts)));
    }

    let base = if parts.has_explicit_format {
        Cow::Owned(source_scoped(parts))
    } else {
        match parts.negotiated_format {
            // Content negotiation makes one URL produce different bytes for
            // different clients, so the chosen format is part of the identity
            // of the entry. Without this a Chrome request would poison the
            // cache for a client that cannot read AVIF.
            Some(format) => Cow::Owned(format!("accept-format={format}:{}", source_scoped(parts))),
            None => Cow::Owned(format!(
                "default-format={}:{}",
                parts.default_format.as_str(),
                source_scoped(parts)
            )),
        }
    };

    let base = match parts.client_hints {
        Some((width, dpr_thousandths)) => Cow::Owned(format!("hint={width}x{dpr_thousandths}:{base}")),
        None => base,
    };

    // The ceilings that describe the *result*. Changing any of them retires the
    // entries stored under the previous setting, including the change from
    // "unset" to a value: an unset limit contributes nothing to the key and a
    // set one contributes its prefix. That is the intended cost — a cold cache
    // for the affected URLs — and not a property to preserve. Only a deployment
    // that never sets them keeps the keys it already had.
    let base = match parts.max_result_dimension {
        Some(limit) => Cow::Owned(format!("mrd={}:{}", limit.get(), base)),
        None => base,
    };

    let base = match parts.max_animation_frames {
        Some(limit) => Cow::Owned(format!("maf={}:{}", limit.get(), base)),
        None => base,
    };

    let base = match parts.max_animation_frame_resolution {
        Some(limit) => Cow::Owned(format!("mafr={}:{}", limit.pixels(), base)),
        None => base,
    };

    source_limits(parts, base)
}

/// The request path, scoped to the source it actually resolves to.
///
/// Only prefixed when the two differ — a URL that carries its own absolute
/// source resolves to itself, so the vast majority of keys stay exactly as they
/// were and only a `IMGFORGE_BASE_URL` deployment pays for the distinction.
fn source_scoped(parts: CacheKeyParts<'_>) -> String {
    if parts.source_url == parts.path {
        return parts.path.to_string();
    }
    format!("src={}:{}", parts.source_url, parts.path)
}

/// Namespaces a key by the limits that describe the source rather than the
/// result.
///
/// Shared by both paths because both can return source bytes: `skip_processing`
/// takes the processed key, `raw` takes the bare path, and neither reaches the
/// source checks on a cache hit.
fn source_limits<'a>(parts: CacheKeyParts<'a>, base: Cow<'a, str>) -> Cow<'a, str> {
    let base = match parts.max_src_resolution {
        Some(limit) => Cow::Owned(format!("msr={}:{}", limit.pixels(), base)),
        None => base,
    };

    let base = match parts.max_src_file_size {
        Some(limit) => Cow::Owned(format!("msfs={}:{}", limit.get(), base)),
        None => base,
    };

    let base = match parts.allowed_mime_types {
        Some(types) => Cow::Owned(format!("amt={}:{}", mime_digest(types), base)),
        None => base,
    };

    let base = match parts.watermark_path {
        Some(path) => Cow::Owned(format!("wm={}:{}", digest(&[path.as_bytes()]), base)),
        None => base,
    };

    let base = match parts.watermark_url {
        Some(url) => Cow::Owned(format!("wmu={}:{}", digest(&[url.as_bytes()]), base)),
        None => base,
    };

    match parts.option_defaults {
        Some(defaults) => Cow::Owned(format!("od={}:{}", defaults_digest(&defaults), base)),
        None => base,
    }
}

/// A short, stable digest of the configured option defaults.
///
/// Destructured rather than read field by field, so adding an option to
/// `OptionDefaults` fails to compile here until it is accounted for — the point
/// being not to grow a second copy of the struct that quietly falls behind it.
fn defaults_digest(defaults: &OptionDefaults) -> String {
    let OptionDefaults {
        auto_rotate,
        strip_metadata,
        keep_copyright,
        strip_color_profile,
        preserve_hdr,
        enforce_thumbnail,
        return_attachment,
        quality,
    } = *defaults;

    let flags = [
        u8::from(auto_rotate),
        u8::from(strip_metadata),
        u8::from(keep_copyright),
        u8::from(strip_color_profile),
        u8::from(preserve_hdr),
        u8::from(enforce_thumbnail),
        u8::from(return_attachment),
        u8::from(quality.is_some()),
        quality.unwrap_or(0),
    ];
    digest(&[&flags])
}

/// A short, stable digest of the permitted MIME types.
///
/// The list itself would make the key unbounded, and the key only has to change
/// when the list does. Sorted first so that reordering the environment variable
/// is not treated as a policy change, and hashed rather than truncated so two
/// different lists cannot collide into one namespace. `sha2` is already a
/// dependency for URL signing, and the digest is stable across releases in a way
/// `DefaultHasher` explicitly is not.
fn mime_digest(types: &[String]) -> String {
    let mut sorted: Vec<&str> = types.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    sorted.dedup();

    let parts: Vec<&[u8]> = sorted.iter().map(|entry| entry.as_bytes()).collect();
    digest(&parts)
}

/// Six bytes of SHA-256 over the parts, separated so that concatenations of
/// different shapes cannot collide.
fn digest(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
        hasher.update([0u8]);
    }
    hasher
        .finalize()
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
