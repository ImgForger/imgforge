//! Choosing what to send based on what the client said it can take.
//!
//! Two independent mechanisms, both from imgproxy. Format negotiation reads
//! `Accept` and upgrades the output to WebP or AVIF when the client advertises
//! it, which is how one URL can serve a modern format to browsers that support
//! it and JPEG to everything else. Client hints read `Width` and `DPR`, letting
//! the browser rather than the URL decide how large the image needs to be.

use crate::config::Config;
use crate::processing::options::ParsedOptions;
use crate::processing::save::is_format_supported;
use axum::http::HeaderMap;
use tracing::debug;

/// What the request told us about the client.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RequestHints {
    /// The raw `Accept` header, used for format negotiation.
    pub accept: Option<String>,
    /// Device pixel ratio the client reported.
    pub dpr: Option<f32>,
    /// Layout width in CSS pixels the client reported.
    pub width: Option<u32>,
}

impl RequestHints {
    /// Reads the hints a request carries.
    ///
    /// Client hints are only read when the server opted in: they let the client
    /// change the response for a URL that does not mention them, which is fine
    /// when a deployment expects it and surprising when it does not.
    pub fn from_headers(headers: &HeaderMap, enable_client_hints: bool) -> Self {
        let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());

        let accept = header("accept").map(str::to_string);
        if !enable_client_hints {
            return Self {
                accept,
                ..Self::default()
            };
        }

        // The `Sec-CH-` spellings are the current ones; the bare names are what
        // older clients send, and imgproxy accepts both.
        let dpr = header("sec-ch-dpr")
            .or_else(|| header("dpr"))
            .and_then(|value| value.trim().parse::<f32>().ok())
            .filter(|dpr| dpr.is_finite() && *dpr > 0.0);
        let width = header("sec-ch-width")
            .or_else(|| header("width"))
            .and_then(|value| value.trim().parse::<u32>().ok())
            .filter(|width| *width > 0);

        Self { accept, dpr, width }
    }

    /// How strongly the `Accept` header advertises a media type.
    ///
    /// `None` means the type is not offered at all, which includes an explicit
    /// `q=0` refusal. Otherwise the weight the client attached to it, defaulting
    /// to 1 when it named none.
    fn quality_for(&self, media_type: &str) -> Option<f32> {
        let accept = self.accept.as_deref()?;

        accept
            .split(',')
            .filter_map(|range| {
                let mut parts = range.split(';').map(str::trim);
                let name = parts.next()?;
                if !name.eq_ignore_ascii_case(media_type) {
                    return None;
                }

                // Anything unparseable, or absent, is the default of 1.
                // Parameter names are case-insensitive, so `Q=0` refuses a
                // type exactly as `q=0` does — reading only the lowercase
                // spelling turned that refusal into the default full weight.
                Some(
                    parts
                        .filter_map(|parameter| {
                            parameter
                                .get(..2)
                                .filter(|prefix| prefix.eq_ignore_ascii_case("q="))
                                .map(|_| &parameter[2..])
                        })
                        .next()
                        .and_then(|q| q.parse::<f32>().ok())
                        .unwrap_or(1.0),
                )
            })
            // A repeated type is the client's own contradiction; taking the
            // strongest offer is the reading that serves it something.
            .fold(None::<f32>, |best, q| Some(best.map_or(q, |best| best.max(q))))
            .filter(|q| *q > 0.0)
    }
}

/// Formats that can be negotiated, best compression first.
///
/// The order breaks ties: a client that offers both at the same weight gets
/// AVIF, which is smaller than WebP at equal quality. It does not override the
/// client, which gets to rank them itself with `q`.
const NEGOTIABLE: &[(&str, &str)] = &[("avif", "image/avif"), ("webp", "image/webp")];

/// Picks the output format for a request, or `None` to leave it alone.
///
/// A format named in the URL normally wins — that is the point of naming it —
/// but `enforce` overrides even that, which is what lets a deployment move its
/// whole catalogue to a modern format without rewriting the URLs.
pub fn negotiate_format(config: &Config, hints: &RequestHints, has_explicit_format: bool) -> Option<&'static str> {
    let mut best: Option<(&'static str, f32)> = None;

    for (format, media_type) in NEGOTIABLE {
        let (detect, enforce) = match *format {
            "avif" => (config.enable_avif_detection, config.enforce_avif),
            "webp" => (config.enable_webp_detection, config.enforce_webp),
            _ => continue,
        };

        if !detect && !enforce {
            continue;
        }
        if has_explicit_format && !enforce {
            continue;
        }
        let Some(quality) = hints.quality_for(media_type) else {
            continue;
        };
        // A build without the encoder would turn negotiation into a 400 for
        // every modern browser.
        if !is_format_supported(format) {
            debug!("{} was negotiated but this libvips build cannot encode it", format);
            continue;
        }

        // The client ranked these, so serving the first one imgforge happens to
        // prefer would ignore what it said: `image/avif;q=0.1, image/webp` asks
        // for WebP. The strict comparison keeps NEGOTIABLE's order as the
        // tie-break, so an equal-weight offer still resolves to AVIF.
        if best.is_none_or(|(_, best_quality)| quality > best_quality) {
            best = Some((format, quality));
        }
    }

    let (format, quality) = best?;
    debug!("Negotiated {} from the client's Accept header (q={})", format, quality);
    Some(format)
}

/// The request headers a response can differ by, for `Vary`.
///
/// A shared cache has to be told, or it will hand an AVIF to a client that
/// cannot read one — or, with client hints on, hand one client's dimensions to
/// another.
pub fn vary_headers(config: &Config) -> Vec<&'static str> {
    let mut headers = Vec::new();

    if config.enable_webp_detection || config.enforce_webp || config.enable_avif_detection || config.enforce_avif {
        headers.push("Accept");
    }

    if config.enable_client_hints {
        headers.extend_from_slice(&["Sec-CH-Width", "Width", "Sec-CH-DPR", "DPR"]);
    }

    headers
}

/// Folds the client's own size hints into the processing options.
///
/// The URL still wins: a request that already names a width is asking for that
/// width, and a hint is the client's suggestion for a URL that left the choice
/// open.
pub fn apply_client_hints(options: &mut ParsedOptions, hints: &RequestHints) {
    if let Some(dpr) = hints.dpr {
        // Only when the URL left the choice open. `dpr:1` names a ratio just
        // as `dpr:2` does, but the explicit form and the default both arrive
        // here as 1.0 — so presence is the signal, not the value.
        if options.dpr.is_none() {
            debug!("Applying client DPR hint: {}", dpr);
            options.dpr = Some(dpr.clamp(1.0, 5.0));
        }
    }

    let Some(width) = hints.width else {
        return;
    };

    // The hint is attacker-adjacent input: a signature covers the path, not
    // the headers, so a reusable signed URL that leaves its width to hints
    // must not let `Width: 1000000` size the pipeline. Bounded by the result
    // ceiling when one is in force, and by a hard cap no real screen exceeds
    // when none is.
    const MAX_HINTED_WIDTH: u32 = 16_384;
    let cap = options
        .max_result_dimension
        .map(|limit| limit.get().min(MAX_HINTED_WIDTH))
        .unwrap_or(MAX_HINTED_WIDTH);
    let width = width.min(cap);

    // `Width` is already in physical pixels — that is what the client hint
    // means — while `dpr` is multiplied back onto the resize target later in
    // the pipeline. Taking the hint at face value therefore applied the ratio
    // twice: `Width: 640` with `DPR: 2` produced a 1280px image for a client
    // that asked for 640. Dividing here is what imgproxy does for the same
    // reason, as `imath.Shrink(features.ClientHintsWidth, dpr)`.
    //
    // The divisor is the *client's* reported ratio, not the effective one. The
    // hint describes that client's own screen, so it is the only ratio the
    // number was expressed against; a `dpr:` in the URL is a separate
    // instruction that multiplies afterwards. imgproxy reads the hints before
    // URL options are applied, which produces exactly this ordering.
    let hinted_dpr = hints.dpr.map_or(1.0, |dpr| dpr.clamp(1.0, 5.0));
    let width = if hinted_dpr > 1.0 {
        let shrunk = (f64::from(width) / f64::from(hinted_dpr)).round() as u32;
        debug!("Client width hint {} is {} before DPR {}", width, shrunk, hinted_dpr);
        shrunk.max(1)
    } else {
        width
    };

    match options.resize.as_mut() {
        Some(resize) if resize.width == 0 => {
            debug!("Applying client width hint: {}", width);
            resize.width = width;
        }
        Some(_) => {}
        None => {
            // The URL may have named a resizing type without dimensions, which
            // the parser drops because it describes no target. Honouring that
            // type here is what keeps `resizing_type:fill` plus a `Width` hint
            // a fill rather than silently becoming a fit.
            let resizing_type = options.resizing_type.unwrap_or_default();
            debug!(
                "Applying client width hint as the {:?} resize target: {}",
                resizing_type, width
            );
            options.width = Some(width);
            options.resize = Some(crate::processing::options::Resize {
                resizing_type,
                width,
                height: 0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processing::options::ResizingType;
    use crate::test_support::init_vips;

    fn config_with(detection: (bool, bool), enforcement: (bool, bool)) -> Config {
        let mut config = Config::new(vec![0u8; 32], vec![0u8; 32]);
        config.enable_webp_detection = detection.0;
        config.enable_avif_detection = detection.1;
        config.enforce_webp = enforcement.0;
        config.enforce_avif = enforcement.1;
        config
    }

    fn accepting(accept: &str) -> RequestHints {
        RequestHints {
            accept: Some(accept.to_string()),
            ..RequestHints::default()
        }
    }

    #[test]
    fn detection_only_applies_when_the_url_left_the_format_open() {
        init_vips();
        let config = config_with((true, false), (false, false));
        let hints = accepting("image/webp,image/*,*/*");

        assert_eq!(negotiate_format(&config, &hints, false), Some("webp"));
        // A URL that names a format is asking for it.
        assert_eq!(negotiate_format(&config, &hints, true), None);
        // A client that does not advertise WebP never gets one.
        assert_eq!(negotiate_format(&config, &accepting("image/*"), false), None);
    }

    #[test]
    fn enforcement_overrides_an_explicit_format() {
        init_vips();
        let config = config_with((false, false), (true, false));
        let hints = accepting("image/webp");

        assert_eq!(negotiate_format(&config, &hints, true), Some("webp"));
    }

    #[test]
    fn avif_is_preferred_when_the_client_takes_both() {
        init_vips();
        // Negotiation only offers what this libvips build can encode, so the
        // AVIF expectations hold only where an AVIF encoder exists — the same
        // guard the save tests use.
        if !is_format_supported("avif") {
            return;
        }
        let config = config_with((true, true), (false, false));
        let hints = accepting("image/avif,image/webp,*/*");

        assert_eq!(negotiate_format(&config, &hints, false), Some("avif"));

        // With only WebP detection enabled, an AVIF-capable client still gets
        // WebP: negotiation offers what the deployment turned on.
        let config = config_with((true, false), (false, false));
        assert_eq!(negotiate_format(&config, &hints, false), Some("webp"));
    }

    #[test]
    fn a_zero_quality_is_a_refusal_rather_than_an_offer() {
        init_vips();
        let config = config_with((true, true), (false, false));

        // The name is present, and explicitly refused.
        assert_eq!(
            negotiate_format(&config, &accepting("image/avif;q=0, image/webp"), false),
            Some("webp")
        );
        assert_eq!(negotiate_format(&config, &accepting("image/webp;q=0.0"), false), None);
        // Parameter names are case-insensitive too: `Q=0` is the same refusal,
        // not an unrecognised parameter defaulting to full weight.
        assert_eq!(
            negotiate_format(&config, &accepting("image/avif;Q=0, image/webp"), false),
            Some("webp")
        );
        // A positive quality still counts, and matching is case-insensitive.
        assert_eq!(
            negotiate_format(&config, &accepting("IMAGE/WEBP;q=0.5"), false),
            Some("webp")
        );
        // A substring of an unrelated range must not match.
        assert_eq!(
            negotiate_format(&config, &accepting("application/image/webp+xml"), false),
            None
        );
    }

    /// `q` ranks the offers; it does not merely gate them. Reducing it to
    /// "greater than zero" made the fixed AVIF-first order override a client
    /// that had explicitly said it would rather have WebP.
    #[test]
    fn the_clients_quality_weights_decide_between_two_offers() {
        init_vips();
        // The ranking under test needs both offers on the table, and AVIF is
        // only offered where this libvips build can encode it.
        if !is_format_supported("avif") {
            return;
        }
        let config = config_with((true, true), (false, false));

        assert_eq!(
            negotiate_format(&config, &accepting("image/avif;q=0.1, image/webp;q=1"), false),
            Some("webp"),
            "a client that prefers WebP must be given WebP"
        );
        assert_eq!(
            negotiate_format(&config, &accepting("image/avif;q=1, image/webp;q=0.1"), false),
            Some("avif")
        );
        // An unweighted offer is q=1, so it outranks a weighted one below it.
        assert_eq!(
            negotiate_format(&config, &accepting("image/avif;q=0.5, image/webp"), false),
            Some("webp")
        );
        // Equal weights fall back to imgforge's own order, which prefers the
        // format that compresses better.
        assert_eq!(
            negotiate_format(&config, &accepting("image/avif;q=0.8, image/webp;q=0.8"), false),
            Some("avif")
        );
        assert_eq!(
            negotiate_format(&config, &accepting("image/avif,image/webp,*/*"), false),
            Some("avif")
        );
    }

    #[test]
    fn client_hints_are_ignored_unless_enabled() {
        let mut headers = HeaderMap::new();
        headers.insert("dpr", "2".parse().unwrap());
        headers.insert("width", "800".parse().unwrap());
        headers.insert("accept", "image/webp".parse().unwrap());

        let ignored = RequestHints::from_headers(&headers, false);
        assert_eq!(ignored.dpr, None);
        assert_eq!(ignored.width, None);
        // Accept is not a client hint and is always read.
        assert_eq!(ignored.accept.as_deref(), Some("image/webp"));

        let honoured = RequestHints::from_headers(&headers, true);
        assert_eq!(honoured.dpr, Some(2.0));
        assert_eq!(honoured.width, Some(800));
    }

    /// `Width` is defined in physical pixels, and `dpr` is multiplied back onto
    /// the resize target further down the pipeline. Taking the hint at face
    /// value applied the ratio twice, so a client asking for 640 was sent 1280.
    #[test]
    fn the_width_hint_is_not_multiplied_by_dpr_twice() {
        use crate::processing::options::Resize;

        let hints = RequestHints {
            width: Some(640),
            dpr: Some(2.0),
            ..RequestHints::default()
        };

        let mut options = ParsedOptions::default();
        apply_client_hints(&mut options, &hints);

        // Stored at 320 so that the later DPR pass brings it back to the 640
        // physical pixels the client actually asked for.
        assert_eq!(options.resize.expect("the hint supplies a width").width, 320);
        assert_eq!(options.dpr, Some(2.0));

        // Without a DPR hint the width is already what it should be.
        let mut options = ParsedOptions::default();
        apply_client_hints(
            &mut options,
            &RequestHints {
                width: Some(640),
                ..RequestHints::default()
            },
        );
        assert_eq!(options.resize.unwrap().width, 640);

        // A width that divides to nothing still has to describe an image.
        let mut options = ParsedOptions::default();
        apply_client_hints(
            &mut options,
            &RequestHints {
                width: Some(1),
                dpr: Some(5.0),
                ..RequestHints::default()
            },
        );
        assert_eq!(options.resize.unwrap().width, 1);

        // The divisor is the client's own ratio. A `dpr:` in the URL is a
        // separate instruction applied afterwards, so it must not change how the
        // hint is read — imgproxy reads the hints before URL options for exactly
        // this reason.
        let mut options = ParsedOptions {
            dpr: Some(3.0),
            ..ParsedOptions::default()
        };
        apply_client_hints(&mut options, &hints);
        assert_eq!(
            options.resize.unwrap().width,
            320,
            "the URL's own dpr must not become the divisor for the client's hint"
        );
        assert_eq!(options.dpr, Some(3.0), "and the URL's dpr still wins for scaling");

        // `dpr:1` in the URL is a choice, not an absence: it says "do not
        // scale", and a larger hint must not overrule it. It used to, because
        // the default was also stored as 1.0 and the two were told apart by
        // value rather than by presence.
        let mut options = ParsedOptions {
            dpr: Some(1.0),
            ..ParsedOptions::default()
        };
        apply_client_hints(
            &mut options,
            &RequestHints {
                dpr: Some(2.0),
                ..RequestHints::default()
            },
        );
        assert_eq!(options.dpr, Some(1.0), "an explicit dpr:1 refuses the hint");

        // The hint is attacker-adjacent input on a signed URL — the signature
        // covers the path, not the headers — so it is bounded before it can
        // size the pipeline.
        let mut options = ParsedOptions::default();
        apply_client_hints(
            &mut options,
            &RequestHints {
                width: Some(1_000_000),
                ..RequestHints::default()
            },
        );
        assert_eq!(options.resize.unwrap().width, 16_384, "the hard cap bounds the hint");

        let mut options = ParsedOptions {
            max_result_dimension: Some("2000".parse().unwrap()),
            ..ParsedOptions::default()
        };
        apply_client_hints(
            &mut options,
            &RequestHints {
                width: Some(1_000_000),
                ..RequestHints::default()
            },
        );
        assert_eq!(
            options.resize.unwrap().width,
            2000,
            "a configured result ceiling bounds it tighter still"
        );

        // The same division applies when the hint fills a zero-width resize the
        // URL already named.
        let mut options = ParsedOptions {
            resize: Some(Resize {
                resizing_type: ResizingType::Fill,
                width: 0,
                height: 480,
            }),
            ..ParsedOptions::default()
        };
        apply_client_hints(&mut options, &hints);
        assert_eq!(options.resize.unwrap().width, 320);
    }

    /// A `resizing_type` with no dimensions survives to meet the hint, so the
    /// request stays the fill the URL asked for.
    #[test]
    fn a_hint_honours_a_resizing_type_the_url_named_without_dimensions() {
        use crate::processing::options::ResizingType;

        let hints = RequestHints {
            width: Some(800),
            ..RequestHints::default()
        };

        let mut options = ParsedOptions {
            resizing_type: Some(ResizingType::Fill),
            ..ParsedOptions::default()
        };
        apply_client_hints(&mut options, &hints);

        let resize = options.resize.expect("the hint supplies the missing width");
        assert_eq!(resize.width, 800);
        assert_eq!(
            resize.resizing_type,
            ResizingType::Fill,
            "the requested type must not silently become a fit"
        );
    }

    #[test]
    fn a_width_in_the_url_wins_over_the_hint() {
        use crate::processing::options::{Resize, ResizingType};

        let hints = RequestHints {
            width: Some(800),
            ..RequestHints::default()
        };

        let mut options = ParsedOptions {
            resize: Some(Resize {
                resizing_type: ResizingType::Fit,
                width: 300,
                height: 0,
            }),
            ..ParsedOptions::default()
        };
        apply_client_hints(&mut options, &hints);
        assert_eq!(options.resize.unwrap().width, 300);

        // With no width of its own, the hint supplies one.
        let mut options = ParsedOptions::default();
        apply_client_hints(&mut options, &hints);
        assert_eq!(options.resize.unwrap().width, 800);
    }
}
