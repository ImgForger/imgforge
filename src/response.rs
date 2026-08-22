//! The caching and provenance headers a processed response carries.

use crate::config::Config;
use crate::fetch::FetchedImage;
use axum::http::HeaderMap;
use sha2::{Digest, Sha256};

/// Headers derived from the configuration and the source response.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeliveryHeaders {
    pub cache_control: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    /// `Link: <url>; rel="canonical"`, pointing at the original image.
    pub canonical: Option<String>,
    /// Request headers the response could differ by.
    pub vary: Vec<&'static str>,
}

impl DeliveryHeaders {
    /// Builds the headers for a response produced from `source`.
    pub fn build(config: &Config, source: &SourceMetadata, body: &[u8], vary: &[&'static str]) -> Self {
        Self::assemble(config, source, config.use_etag.then(|| entity_tag(body)), vary)
    }

    /// Builds the headers for a cache hit, whose entity tag was computed when
    /// the entry was stored.
    ///
    /// Hashing belongs on the miss, where the body was just produced anyway.
    /// Doing it again per hit put a whole-body SHA-256 on the async worker for
    /// exactly the requests that were supposed to be cheap.
    pub fn for_cache_hit(config: &Config, source: &SourceMetadata, stored_etag: &str, vary: &[&'static str]) -> Self {
        let etag = (config.use_etag && !stored_etag.is_empty()).then(|| stored_etag.to_string());
        Self::assemble(config, source, etag, vary)
    }

    fn assemble(config: &Config, source: &SourceMetadata, etag: Option<String>, vary: &[&'static str]) -> Self {
        Self {
            cache_control: cache_control(config, source.cache_control.as_deref()),
            etag,
            last_modified: config
                .last_modified_enabled
                .then(|| source.last_modified.clone())
                .flatten(),
            canonical: config
                .set_canonical_header
                .then(|| source.url.clone())
                .flatten()
                .map(|url| format!("<{}>; rel=\"canonical\"", link_uri_reference(&url))),
            vary: vary.to_vec(),
        }
    }
}

/// The part of a source response that shapes the delivery headers.
#[derive(Debug, Clone, Default)]
pub struct SourceMetadata {
    pub cache_control: Option<String>,
    pub last_modified: Option<String>,
    pub url: Option<String>,
}

impl SourceMetadata {
    pub fn from_fetch(fetched: &FetchedImage, url: &str) -> Self {
        Self {
            cache_control: fetched.cache_control.clone(),
            last_modified: fetched.last_modified.clone(),
            url: Some(url.to_string()),
        }
    }
}

/// The `Cache-Control` value to send.
///
/// Passthrough hands the origin's own policy to the client, which is what a
/// deployment wants when the origin already expresses one; otherwise a
/// configured TTL becomes a `max-age`. With neither, no header is sent and the
/// client falls back to its own heuristics, which is the behaviour imgforge
/// has always had.
fn cache_control(config: &Config, source_cache_control: Option<&str>) -> Option<String> {
    // A bearer-protected response must never be reusable from a shared cache:
    // `public` expressly invites a CDN to store the authorised answer and
    // replay it to a request carrying no token. The origin's own policy does
    // not get a say either — it cannot know imgforge put a token in front of
    // it — so this outranks passthrough, and it applies even with no TTL
    // configured, because heuristic caching needs refusing too.
    if config.secret.as_deref().is_some_and(|secret| !secret.is_empty()) {
        return Some(match config.ttl {
            Some(ttl) => format!("max-age={ttl}, private"),
            None => "private".to_string(),
        });
    }

    if config.cache_control_passthrough {
        if let Some(value) = source_cache_control.filter(|value| !value.trim().is_empty()) {
            return Some(sanitise_header_value(value));
        }
    }

    config.ttl.map(|ttl| format!("max-age={ttl}, public"))
}

/// A strong entity tag for a response body.
///
/// Derived from the bytes rather than from the source's own `ETag` and the
/// processing options: the same URL against the same source can still produce
/// different bytes once content negotiation is in play, and hashing the result
/// is the only derivation that cannot disagree with what was actually sent.
pub fn entity_tag(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    let mut tag = String::with_capacity(2 + 32);
    tag.push('"');
    for byte in digest.iter().take(16) {
        tag.push_str(&format!("{byte:02x}"));
    }
    tag.push('"');
    tag
}

/// Header values may not contain control characters; a source URL or an origin
/// header is attacker-influenced input, so it is stripped rather than trusted.
/// Escapes a URL for the `<...>` URI-reference slot of a `Link` header.
///
/// RFC 8288 ends the reference at the first `>`, so a source URL carrying one
/// closes the brackets early and everything after it is read as further link
/// parameters — a source could append its own `rel=` and have imgforge state it
/// as fact. Stripping control characters does not help: `<` and `>` are neither.
/// Percent-encoding them keeps the URL both valid and inert, and matches what
/// the URL parser does with the same characters before the fetch, so the header
/// still names the address that was actually requested.
fn link_uri_reference(url: &str) -> String {
    sanitise_header_value(url).replace('<', "%3C").replace('>', "%3E")
}

fn sanitise_header_value(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(1024)
        .collect()
}

/// Whether a request's `If-Modified-Since` covers the timestamp we would send.
///
/// Whether the representation is unmodified since the date the request names.
///
/// RFC 9110 asks the question chronologically — "not modified *since*" — so any
/// date at or after the one we would send answers 304. Comparing the strings
/// instead only recognised a client echoing our value back byte for byte, which
/// silently sent the whole body to a client whose cached copy was provably
/// current: a caching proxy that normalises the date's spelling, or one holding
/// a copy it fetched later than the origin's own timestamp, got a 200 every
/// time. The parse falls back to the string comparison rather than to `false`,
/// so a date in a form `httpdate` will not read still behaves as it used to.
pub fn matches_if_modified_since(headers: &HeaderMap, last_modified: &str) -> bool {
    let Some(requested) = headers
        .get("if-modified-since")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
    else {
        return false;
    };
    let last_modified = last_modified.trim();

    match (
        httpdate::parse_http_date(requested),
        httpdate::parse_http_date(last_modified),
    ) {
        (Ok(requested), Ok(modified)) => modified <= requested,
        _ => requested == last_modified,
    }
}

/// Whether a request's `If-None-Match` matches the tag we are about to send.
///
/// Handles the comma-separated list form and the weak-comparison prefix, both
/// of which real clients send.
pub fn matches_if_none_match(headers: &HeaderMap, etag: &str) -> bool {
    let Some(value) = headers.get("if-none-match").and_then(|value| value.to_str().ok()) else {
        return false;
    };

    value
        .split(',')
        .map(str::trim)
        .any(|candidate| candidate == "*" || candidate.trim_start_matches("W/") == etag.trim_start_matches("W/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::new(vec![0u8; 32], vec![0u8; 32])
    }

    /// "Not modified *since*" is a chronological question. Comparing the strings
    /// only recognised a client echoing our exact value back, so a proxy holding
    /// a provably current copy under a later or differently spelled date was
    /// sent the whole body.
    #[test]
    fn if_modified_since_is_compared_chronologically() {
        let last_modified = "Wed, 21 Oct 2015 07:28:00 GMT";
        let asking = |value: &str| {
            let mut headers = HeaderMap::new();
            headers.insert("if-modified-since", value.parse().unwrap());
            matches_if_modified_since(&headers, last_modified)
        };

        // The exact echo, which is what a well-behaved client sends.
        assert!(asking(last_modified));
        // Later than ours: the copy is still current, so 304.
        assert!(asking("Thu, 22 Oct 2015 07:28:00 GMT"));
        assert!(asking("Wed, 21 Oct 2015 07:28:01 GMT"));
        // Earlier: the client's copy predates the representation, so send it.
        assert!(!asking("Tue, 20 Oct 2015 07:28:00 GMT"));
        assert!(!asking("Wed, 21 Oct 2015 07:27:59 GMT"));

        // The other two formats RFC 9110 requires a recipient to accept.
        assert!(asking("Thursday, 22-Oct-15 07:28:00 GMT"));
        assert!(asking("Thu Oct 22 07:28:00 2015"));

        // A date neither side can parse falls back to the old exact match
        // rather than to a blanket 304.
        assert!(!asking("not a date"));

        // No header at all is not a conditional request.
        assert!(!matches_if_modified_since(&HeaderMap::new(), last_modified));
    }

    /// RFC 8288 ends the `<...>` URI reference at the first `>`, so a source URL
    /// carrying one closes the brackets early and everything after it is read as
    /// further link parameters — letting a source append its own `rel=` and have
    /// imgforge state it as fact. Control-character stripping does not catch it:
    /// `<` and `>` are neither.
    #[test]
    fn the_canonical_link_cannot_be_broken_out_of() {
        let mut config = Config::new(vec![0u8; 32], vec![0u8; 32]);
        config.set_canonical_header = true;

        let canonical_for = |url: &str| {
            DeliveryHeaders::build(
                &config,
                &SourceMetadata {
                    url: Some(url.to_string()),
                    ..SourceMetadata::default()
                },
                b"body",
                &[],
            )
            .canonical
            .expect("the canonical header is enabled")
        };

        let injected = canonical_for("https://cdn.example.com/a>;rel=\"stylesheet\";x=<b");
        assert!(
            !injected[1..injected.len() - "; rel=\"canonical\"".len() - 1].contains('>'),
            "the URI reference must contain no bare '>': {injected}"
        );
        assert!(injected.contains("%3E"), "the bracket should be encoded: {injected}");
        assert!(injected.contains("%3C"), "and so should its opening partner");
        assert!(
            injected.ends_with("; rel=\"canonical\""),
            "exactly one link-value should be emitted: {injected}"
        );

        // An ordinary URL is untouched.
        let plain = canonical_for("https://cdn.example.com/cat.jpg");
        assert_eq!(plain, "<https://cdn.example.com/cat.jpg>; rel=\"canonical\"");
    }

    #[test]
    fn no_ttl_and_no_passthrough_sends_no_cache_control() {
        let config = config();
        assert_eq!(cache_control(&config, Some("max-age=60")), None);
    }

    #[test]
    fn passthrough_prefers_the_origin_and_falls_back_to_the_ttl() {
        let mut config = config();
        config.cache_control_passthrough = true;
        config.ttl = Some(3600);

        assert_eq!(
            cache_control(&config, Some("public, max-age=60")).as_deref(),
            Some("public, max-age=60")
        );
        // An origin that says nothing leaves the configured policy in charge.
        assert_eq!(cache_control(&config, None).as_deref(), Some("max-age=3600, public"));
        assert_eq!(
            cache_control(&config, Some("   ")).as_deref(),
            Some("max-age=3600, public")
        );
    }

    #[test]
    fn a_ttl_alone_becomes_a_max_age() {
        let mut config = config();
        config.ttl = Some(86_400);
        assert_eq!(
            cache_control(&config, Some("no-store")).as_deref(),
            Some("max-age=86400, public")
        );
    }

    #[test]
    fn entity_tags_track_the_bytes_that_were_sent() {
        let one = entity_tag(b"first");
        let two = entity_tag(b"second");

        assert_ne!(one, two);
        assert_eq!(one, entity_tag(b"first"));
        assert!(one.starts_with('"') && one.ends_with('"'));
    }

    #[test]
    fn conditional_requests_match_a_list_and_the_weak_prefix() {
        let etag = entity_tag(b"body");

        let mut headers = HeaderMap::new();
        headers.insert("if-none-match", etag.parse().unwrap());
        assert!(matches_if_none_match(&headers, &etag));

        let mut headers = HeaderMap::new();
        headers.insert("if-none-match", format!("\"other\", W/{etag}").parse().unwrap());
        assert!(matches_if_none_match(&headers, &etag));

        let mut headers = HeaderMap::new();
        headers.insert("if-none-match", "*".parse().unwrap());
        assert!(matches_if_none_match(&headers, &etag));

        let mut headers = HeaderMap::new();
        headers.insert("if-none-match", "\"nope\"".parse().unwrap());
        assert!(!matches_if_none_match(&headers, &etag));

        // No header at all is not a match, or every first request would 304.
        assert!(!matches_if_none_match(&HeaderMap::new(), &etag));
    }

    #[test]
    fn control_characters_never_reach_a_header_value() {
        let mut config = config();
        config.cache_control_passthrough = true;
        let smuggled = cache_control(&config, Some("max-age=60\r\nX-Injected: yes"));
        assert_eq!(smuggled.as_deref(), Some("max-age=60X-Injected: yes"));
    }
}
