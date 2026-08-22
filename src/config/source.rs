//! Which source URLs a deployment will fetch, and how a bare path becomes one.

use reqwest::Url;
use tracing::debug;

/// One entry of `IMGFORGE_ALLOWED_SOURCES`.
///
/// imgproxy matches a prefix, with `*` allowed as a wildcard for one host
/// label — `https://*.example.com/` permits `images.example.com` but not
/// `example.com` or `a.b.example.com`.
///
/// The candidate is compared as a *parsed URL* rather than as a string. Three
/// separate bypasses came out of comparing text, each closed only where it was
/// found: `https://cdn.example.com` matched `cdn.example.com.evil.test` because
/// the string starts the same way; the `user@` form hid the real host behind
/// userinfo; and `/public/../private/x` satisfied a `/public/` prefix while the
/// URL parser resolved the dots and fetched `/private/x`. Every one of them is
/// a case where the bytes imgforge inspected and the address reqwest dialled
/// were not the same thing. Parsing first makes them the same thing by
/// construction, which is why this is a parse rather than a fourth check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePattern {
    scheme: String,
    host: HostMatch,
    /// Port the entry named, when it named one.
    port: Option<u16>,
    /// The path the entry restricts to, already normalised.
    path_prefix: String,
    /// The query the entry restricts to, when it named one.
    ///
    /// As a prefix of the raw URL, an entry with a query pins everything up to
    /// it: `?tenant=public` permits `?tenant=public&x=1` but not
    /// `?tenant=private`, and not a URL with no query at all. Dropping it
    /// widened the boundary the operator wrote down.
    query_prefix: Option<String>,
}

/// How an entry's host half is compared against a URL's host.
#[derive(Debug, Clone, PartialEq, Eq)]
enum HostMatch {
    /// The host must equal this exactly.
    Exact(String),
    /// The host must end with this after exactly one further label.
    Suffix(String),
    /// The entry could not be parsed as a URL prefix at all, so it is compared
    /// as literal text. imgproxy documents entries with a scheme; this only
    /// keeps a malformed one behaving as it did rather than silently matching
    /// nothing, and it can never be more permissive than the string it names.
    Unstructured,
}

impl SourcePattern {
    pub fn parse(pattern: &str) -> Self {
        let unstructured = || Self {
            scheme: pattern.to_string(),
            host: HostMatch::Unstructured,
            port: None,
            path_prefix: String::new(),
            query_prefix: None,
        };

        // The wildcard is not a legal host, so the entry cannot be parsed as a
        // URL directly. Substituting a placeholder label lets the same parser
        // handle both forms, and keeps the port and path handling identical.
        let (probe, wildcard) = match pattern.split_once("://*.") {
            Some((scheme, rest)) => (format!("{scheme}://imgforge-wildcard.{rest}"), true),
            None => (pattern.to_string(), false),
        };

        let Ok(parsed) = Url::parse(&probe) else {
            return unstructured();
        };
        let Some(host) = parsed.host_str() else {
            return unstructured();
        };

        let host = if wildcard {
            match host.split_once('.') {
                Some((_, suffix)) => HostMatch::Suffix(format!(".{suffix}")),
                None => return unstructured(),
            }
        } else {
            HostMatch::Exact(host.to_string())
        };

        // A query pins the path with it: as a prefix of the raw URL, anything
        // more path would have to come before the `?`, and nothing can. So the
        // path is kept exactly as parsed for equality rather than run through
        // the prefix normalisation.
        let (path_prefix, query_prefix) = match parsed.query() {
            Some(query) => (parsed.path().to_string(), Some(query.to_string())),
            // `Url` has already resolved any dot segments here, so an entry and
            // a candidate are compared in the same normalised terms.
            None => (normalise_prefix(parsed.path(), pattern), None),
        };

        Self {
            scheme: parsed.scheme().to_string(),
            host,
            port: parsed.port(),
            path_prefix,
            query_prefix,
        }
    }

    pub fn matches(&self, url: &str) -> bool {
        if matches!(self.host, HostMatch::Unstructured) {
            return url.starts_with(&self.scheme);
        }

        let Ok(parsed) = Url::parse(url) else {
            return false;
        };
        if parsed.scheme() != self.scheme || parsed.port() != self.port {
            return false;
        }
        let Some(host) = parsed.host_str() else {
            return false;
        };

        let host_ok = match &self.host {
            HostMatch::Exact(expected) => host.eq_ignore_ascii_case(expected),
            HostMatch::Suffix(suffix) => {
                let host = host.to_ascii_lowercase();
                match host.strip_suffix(suffix.as_str()) {
                    // The wildcard stands for exactly one label, so what
                    // precedes the suffix must be a single non-empty label.
                    Some(label) => !label.is_empty() && !label.contains('.'),
                    None => false,
                }
            }
            HostMatch::Unstructured => unreachable!("handled above"),
        };

        // `Url::path()` is normalised, so `/public/../private/x` arrives here as
        // `/private/x` — the path reqwest will actually request.
        let path_ok = match &self.query_prefix {
            Some(query) => {
                parsed.path() == self.path_prefix && parsed.query().unwrap_or("").starts_with(query.as_str())
            }
            None => parsed.path().starts_with(&self.path_prefix),
        };
        host_ok && path_ok
    }
}

/// The path an entry restricts to.
///
/// `Url::parse` gives a bare host the path `/`, which as a prefix would permit
/// the whole host — correct when the entry named no path, wrong if it did. The
/// original text decides which of those the operator meant.
fn normalise_prefix(path: &str, pattern: &str) -> String {
    let named_a_path = pattern
        .split_once("://")
        .is_some_and(|(_, rest)| rest.contains('/') && !rest.ends_with("://"));
    if !named_a_path && path == "/" {
        return String::new();
    }
    path.to_string()
}

/// How source URLs are resolved and restricted.
#[derive(Debug, Clone, Default)]
pub struct SourceRules {
    /// Prepended to every source URL, so URLs can carry only a path.
    pub base_url: Option<String>,
    /// When non-empty, a source URL must match one of these to be fetched.
    pub allowed: Vec<SourcePattern>,
}

impl SourceRules {
    /// Applies the base URL to a decoded source reference.
    pub fn resolve(&self, url: &str) -> String {
        let Some(base) = self.base_url.as_deref() else {
            return url.to_string();
        };
        // A URL that already names a scheme is complete; the base is for the
        // shorthand form where the URL carries only a path.
        if url.contains("://") {
            return url.to_string();
        }
        format!("{}{}", base.trim_end_matches('/'), ensure_leading_slash(url))
    }

    /// Whether a resolved source URL may be fetched.
    pub fn permits(&self, url: &str) -> bool {
        if self.allowed.is_empty() {
            return true;
        }
        let permitted = self.allowed.iter().any(|pattern| pattern.matches(url));
        if !permitted {
            debug!("Source URL rejected by IMGFORGE_ALLOWED_SOURCES");
        }
        permitted
    }
}

fn ensure_leading_slash(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wildcard_stands_for_exactly_one_label() {
        let pattern = SourcePattern::parse("https://*.example.com/");

        assert!(pattern.matches("https://images.example.com/cat.jpg"));
        // Bare domain and multi-label subdomains are both outside the pattern,
        // which is what keeps `*.example.com` from being read as "anything
        // ending in example.com".
        assert!(!pattern.matches("https://example.com/cat.jpg"));
        assert!(!pattern.matches("https://a.b.example.com/cat.jpg"));
        assert!(!pattern.matches("http://images.example.com/cat.jpg"));
        // An attacker-controlled host must not be able to smuggle the suffix
        // into the path.
        assert!(!pattern.matches("https://evil.com/.example.com/cat.jpg"));
    }

    /// The suffix has to end the authority, not merely appear in it. Without
    /// that anchor an attacker registers `example.com.evil.test`, prefixes a
    /// label, and is fetched from — which is the whole of the SSRF the allow
    /// list exists to prevent.
    #[test]
    fn a_wildcard_cannot_be_extended_into_another_domain() {
        // Both spellings of the pattern have to hold: only the trailing slash
        // was ever incidentally anchored.
        for spelling in ["https://*.example.com", "https://*.example.com/"] {
            let pattern = SourcePattern::parse(spelling);

            assert!(pattern.matches("https://img.example.com/cat.jpg"), "{spelling}");
            assert!(
                !pattern.matches("https://img.example.com.evil.test/cat.jpg"),
                "{spelling} must not admit a longer domain that merely starts the same way"
            );
            assert!(
                !pattern.matches("https://img.example.com.evil.test/.example.com/x"),
                "{spelling} must not be satisfied by a suffix hidden in the path"
            );
            // `user@host` puts the real host after the authority's `@`, which
            // is the same trick spelled with credentials.
            assert!(
                !pattern.matches("https://img.example.com@evil.test/cat.jpg"),
                "{spelling} must not admit a host smuggled past userinfo"
            );
            // A query or fragment ends the authority just as a slash does.
            assert!(
                !pattern.matches("https://img.example.com.evil.test?a=.example.com"),
                "{spelling} must not be satisfied from the query string"
            );
        }
    }

    /// An entry with a query is a prefix of the whole URL, query included.
    /// Discarding it read `?tenant=public` as "any query or none", which
    /// widened the boundary the operator wrote down.
    #[test]
    fn a_query_in_the_entry_restricts_the_match() {
        let pattern = SourcePattern::parse("https://api.example.test/render?tenant=public");

        assert!(pattern.matches("https://api.example.test/render?tenant=public"));
        // More query after the prefix is more URL after the prefix, which a
        // prefix permits.
        assert!(pattern.matches("https://api.example.test/render?tenant=public&size=2"));
        assert!(!pattern.matches("https://api.example.test/render?tenant=private"));
        assert!(!pattern.matches("https://api.example.test/render"));
        // A query pins the path with it: as a prefix of the raw URL, anything
        // more path would have to come before the `?`, and nothing can.
        assert!(!pattern.matches("https://api.example.test/render/extra?tenant=public"));
        assert!(!pattern.matches("https://api.example.test/other?tenant=public"));

        // An entry without a query keeps its prefix-of-the-path reading and
        // says nothing about the candidate's query.
        let open = SourcePattern::parse("https://api.example.test/render");
        assert!(open.matches("https://api.example.test/render?tenant=private"));
    }

    /// The literal form needs the same anchoring as the wildcard. Fixing only
    /// the wildcard branch left `https://cdn.example.com` — the spelling most
    /// operators reach for — matching `cdn.example.com.evil.test`, which is the
    /// whole of the SSRF the allow list exists to prevent.
    #[test]
    fn a_literal_entry_cannot_be_extended_into_another_domain() {
        for spelling in ["https://cdn.example.com", "https://cdn.example.com/"] {
            let pattern = SourcePattern::parse(spelling);

            assert!(pattern.matches("https://cdn.example.com/cat.jpg"), "{spelling}");
            assert!(
                !pattern.matches("https://cdn.example.com.evil.test/cat.jpg"),
                "{spelling} must not admit a longer domain that starts the same way"
            );
            assert!(
                !pattern.matches("https://cdn.example.com@evil.test/cat.jpg"),
                "{spelling} must not admit a host smuggled past userinfo"
            );
            assert!(
                !pattern.matches("https://cdn.example.com.evil.test?a=cdn.example.com"),
                "{spelling} must not be satisfied from the query string"
            );
            // A different scheme is a different origin.
            assert!(!pattern.matches("http://cdn.example.com/cat.jpg"), "{spelling}");
            // A port is part of the authority, so an entry naming none does not
            // match a URL that does.
            assert!(!pattern.matches("https://cdn.example.com:8443/cat.jpg"), "{spelling}");
        }

        // An entry that names a port matches that port.
        let ported = SourcePattern::parse("https://cdn.example.com:8443/");
        assert!(ported.matches("https://cdn.example.com:8443/cat.jpg"));
        assert!(!ported.matches("https://cdn.example.com/cat.jpg"));
    }

    /// A path prefix has to survive dot segments. `/public/../private/x`
    /// satisfies a naive `/public/` prefix while the URL parser resolves the
    /// dots and fetches `/private/x` — the check and the request disagreeing
    /// about what the URL says, which is the whole shape of this bug class.
    #[test]
    fn a_path_prefix_cannot_be_escaped_by_traversal() {
        for spelling in ["https://cdn.example.com/public/", "https://*.example.com/public/"] {
            let pattern = SourcePattern::parse(spelling);
            let host = if spelling.contains('*') {
                "img.example.com"
            } else {
                "cdn.example.com"
            };

            assert!(pattern.matches(&format!("https://{host}/public/cat.jpg")), "{spelling}");
            assert!(
                !pattern.matches(&format!("https://{host}/public/../private/secret.png")),
                "{spelling} must not admit a dot-segment escape"
            );
            assert!(
                !pattern.matches(&format!("https://{host}/public/../../etc/passwd")),
                "{spelling} must not admit a deeper escape"
            );
            // Percent-encoded dots resolve the same way once parsed.
            assert!(
                !pattern.matches(&format!("https://{host}/public/%2e%2e/private/secret.png")),
                "{spelling} must not admit an encoded escape"
            );
            // A path that merely contains "..'" in a segment name is not a
            // traversal and stays permitted.
            assert!(
                pattern.matches(&format!("https://{host}/public/a..b/cat.jpg")),
                "{spelling}"
            );
        }
    }

    /// Hosts are compared case-insensitively, as DNS does.
    #[test]
    fn host_matching_ignores_case() {
        assert!(SourcePattern::parse("https://cdn.example.com/").matches("https://CDN.Example.COM/cat.jpg"));
        assert!(SourcePattern::parse("https://*.example.com/").matches("https://IMG.Example.com/cat.jpg"));
    }

    /// A wildcard may also constrain the path, and that half stays a prefix.
    #[test]
    fn a_wildcard_can_carry_a_path_prefix() {
        let pattern = SourcePattern::parse("https://*.example.com/assets/");

        assert!(pattern.matches("https://img.example.com/assets/cat.jpg"));
        assert!(!pattern.matches("https://img.example.com/private/cat.jpg"));
        assert!(!pattern.matches("https://img.example.com.evil.test/assets/cat.jpg"));
    }

    #[test]
    fn a_plain_pattern_matches_by_prefix() {
        let pattern = SourcePattern::parse("https://cdn.example.com/assets/");
        assert!(pattern.matches("https://cdn.example.com/assets/cat.jpg"));
        assert!(!pattern.matches("https://cdn.example.com/private/cat.jpg"));
    }

    #[test]
    fn an_empty_allow_list_permits_everything() {
        let rules = SourceRules::default();
        assert!(rules.permits("https://anywhere.example/cat.jpg"));
    }

    #[test]
    fn the_base_url_only_applies_to_relative_references() {
        let rules = SourceRules {
            base_url: Some("https://cdn.example.com/".to_string()),
            allowed: Vec::new(),
        };

        assert_eq!(rules.resolve("cat.jpg"), "https://cdn.example.com/cat.jpg");
        assert_eq!(rules.resolve("/cat.jpg"), "https://cdn.example.com/cat.jpg");
        assert_eq!(
            rules.resolve("https://other.example/cat.jpg"),
            "https://other.example/cat.jpg"
        );
    }
}
