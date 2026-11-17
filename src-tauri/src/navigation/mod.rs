//! URL resolution and sanitization.
//!
//! Everything that enters the navigation pipeline goes through
//! [`sanitize_url`], which only ever allows `http`/`https`. `file://` is
//! deliberately NOT allowed (it would let a crafted page/link read local
//! files); `about:blank` is only used internally as a fallback.

use url::Url;

/// The built-in start page, rendered by the chrome UI itself (never a content
/// webview). `browser://home` keeps the scheme clearly namespaced as ours.
pub const HOME_URL: &str = "browser://home";
/// Scheme of the built-in pages.
pub const INTERNAL_SCHEME: &str = "browser";

/// Built-in pages (rendered by the UI webview).
pub fn is_internal_url(url: &str) -> bool {
    url == HOME_URL
        || url == "about:home"
        || url == "browser://newtab"
        || url == "about:blank"
        || url.is_empty()
}

/// Only these schemes may ever be rendered in a content webview.
pub fn is_web_scheme(scheme: &str) -> bool {
    scheme == "http" || scheme == "https"
}

/// A search engine usable from the omnibox.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchEngine {
    /// Short name shown in the UI ("DuckDuckGo").
    pub name: String,
    /// Omnibox keyword ("ddg"); an empty keyword never matches.
    pub keyword: String,
    /// Query template with a literal `{}` where the encoded query goes.
    pub template: String,
    /// Shown as the default engine's hint.
    pub suggest_url: Option<String>,
}

impl SearchEngine {
    pub fn build_query(&self, query: &str) -> String {
        self.template.replace("{}", &encode_query(query))
    }
}

/// Built-in engines. Defaults are conservative and privacy-respecting.
pub fn builtin_engines() -> Vec<SearchEngine> {
    vec![
        SearchEngine {
            name: "DuckDuckGo".into(),
            keyword: "ddg".into(),
            template: "https://duckduckgo.com/?q={}".into(),
            suggest_url: None,
        },
        SearchEngine {
            name: "Google".into(),
            keyword: "g".into(),
            template: "https://www.google.com/search?q={}".into(),
            suggest_url: None,
        },
        SearchEngine {
            name: "Bing".into(),
            keyword: "b".into(),
            template: "https://www.bing.com/search?q={}".into(),
            suggest_url: None,
        },
        SearchEngine {
            name: "Brave Search".into(),
            keyword: "br".into(),
            template: "https://search.brave.com/search?q={}".into(),
            suggest_url: None,
        },
        SearchEngine {
            name: "Wikipedia".into(),
            keyword: "w".into(),
            template: "https://en.wikipedia.org/wiki/{}".into(),
            suggest_url: None,
        },
    ]
}

/// Resolve raw omnibox input to a URL.
///
/// Order:
/// 1. empty / built-in aliases -> the start page
/// 2. `keyword query` or `!bang query` -> that engine
/// 3. already-absolute http(s) URL -> sanitized as-is
/// 4. `scheme://...` for a non-web scheme -> treated as a search query
///    (prevents `file:`/`javascript:` from ever being navigated to)
/// 5. bare domain heuristics (`example.com`, `localhost:8080`,
///    `user@host`) -> `https://…`
/// 6. otherwise -> default engine search
pub fn resolve_input(input: &str, engines: &[SearchEngine], default_engine: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return HOME_URL.into();
    }

    if is_internal_url(trimmed) {
        return HOME_URL.into();
    }

    // Engine keyword ("ddg cats") or bang ("!g cats" / "g cats").
    let mut words = trimmed.split_whitespace();
    let first = words.next().unwrap_or("");
    let rest: Vec<&str> = words.collect();
    if !rest.is_empty() {
        let token = first.trim_start_matches('!');
        if let Some(engine) = engines
            .iter()
            .find(|e| !e.keyword.is_empty() && e.keyword == token)
        {
            return engine.build_query(&rest.join(" "));
        }
    }

    if let Ok(url) = Url::parse(trimmed) {
        if is_web_scheme(url.scheme()) {
            return sanitize_url(&url.to_string()).unwrap_or_else(|_| HOME_URL.into());
        }
        // Non-web scheme in an absolute URL: fall through to search so a
        // typed "file:///etc/passwd" searches for that text instead of
        // opening it.
    }

    if looks_like_host(trimmed) {
        if let Ok(url) = Url::parse(&format!("https://{trimmed}")) {
            if is_web_scheme(url.scheme()) {
                return sanitize_url(&url.to_string()).unwrap_or_else(|_| HOME_URL.into());
            }
        }
    }

    let engine = engines
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(default_engine))
        .unwrap_or(&engines[0]);
    engine.build_query(trimmed)
}

/// Host heuristics for input without a scheme.
fn looks_like_host(s: &str) -> bool {
    if s.contains(' ') || s.contains('\t') {
        return false;
    }
    // Must not contain a scheme colon before any slash.
    if let Some(slash) = s.find('/') {
        if let Some(colon) = s.find(':') {
            if colon < slash {
                return false;
            }
        }
    } else if s.contains(':') {
        // "localhost:3000" is a host; "hello:world" is not.
        let after = &s[s.find(':').unwrap() + 1..];
        if !after.chars().all(|c| c.is_ascii_digit()) || after.is_empty() {
            return false;
        }
    }
    // A dot in a plausible domain position, or localhost.
    let host = s.split('/').next().unwrap_or(s);
    let host = host.split('@').next_back().unwrap_or(host);
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if !(host.contains('.') && !host.starts_with('.') && !host.ends_with('.')) {
        return false;
    }
    // The last label must be all letters and at least 2 chars ("example.com"
    // yes, "1.2.3.4" yes via digits handled below, "a.b-" no).
    match host.split('.').next_back() {
        Some(t) if t.chars().all(|c| c.is_ascii_alphabetic()) && t.len() >= 2 => true,
        // IPv4 literal
        _ => host
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit())),
    }
}

/// Allow only `http`/`https` URLs with a host. Internal pages are handled by
/// the UI, never sanitized into content URLs.
pub fn sanitize_url(input: &str) -> Result<String, String> {
    let url = Url::parse(input).map_err(|_| format!("invalid URL: {input}"))?;
    if !is_web_scheme(url.scheme()) {
        return Err(format!("scheme not allowed: {}", url.scheme()));
    }
    if url.host_str().is_none() {
        return Err("URL has no host".into());
    }
    Ok(url.to_string())
}

/// Percent-encode a search query (path-segment-safe, spaces as `%20`).
pub fn encode_query(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xf) as usize] as char);
            }
        }
    }
    out
}

/// Render a host for display ("https://example.com/" -> "example.com").
pub fn pretty_host(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_else(|| url.to_string())
}

/// Strip common `www.` for display purposes.
pub fn display_host(host: &str) -> &str {
    host.strip_prefix("www.").unwrap_or(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engines() -> Vec<SearchEngine> {
        builtin_engines()
    }

    #[test]
    fn resolves_https_urls() {
        assert_eq!(
            resolve_input("https://example.com", &engines(), "DuckDuckGo"),
            "https://example.com/"
        );
    }

    #[test]
    fn resolves_bare_domains() {
        assert_eq!(
            resolve_input("example.com", &engines(), "DuckDuckGo"),
            "https://example.com/"
        );
    }

    #[test]
    fn resolves_host_with_port() {
        // "localhost:8080" is not a valid absolute URL (scheme "localhost"),
        // so the host heuristic applies and upgrades it to https.
        assert_eq!(
            resolve_input("localhost:8080", &engines(), "DuckDuckGo"),
            "https://localhost:8080/"
        );
    }

    #[test]
    fn resolves_search_queries() {
        let out = resolve_input("rust browser", &engines(), "DuckDuckGo");
        assert!(out.starts_with("https://duckduckgo.com/?q=rust%20browser"), "{out}");
    }

    #[test]
    fn resolves_keyword_queries() {
        let out = resolve_input("g rust browser", &engines(), "DuckDuckGo");
        assert!(out.starts_with("https://www.google.com/search?q=rust%20browser"), "{out}");
    }

    #[test]
    fn resolves_bang_queries() {
        let out = resolve_input("!w_SOCKET", &engines(), "DuckDuckGo");
        assert!(out.starts_with("https://en.wikipedia.org/wiki/"), "{out}");
    }

    #[test]
    fn resolves_builtin_home_aliases() {
        for input in [
            "browser://home",
            "about:home",
            "browser://newtab",
            "  ",
            "about:blank",
        ] {
            assert_eq!(resolve_input(input, &engines(), "DuckDuckGo"), HOME_URL);
        }
    }

    #[test]
    fn file_scheme_is_rejected_everywhere() {
        assert!(sanitize_url("file:///etc/passwd").is_err());
        // Typed directly, it becomes a search, not a file open.
        let out = resolve_input("file:///etc/passwd", &engines(), "DuckDuckGo");
        assert!(out.starts_with("https://duckduckgo.com/?q="), "{out}");
        // With an absolute-URL shape it still never resolves to file.
        assert!(sanitize_url(&out).is_ok());
    }

    #[test]
    fn javascript_scheme_is_rejected() {
        assert!(sanitize_url("javascript:alert(1)").is_err());
        let out = resolve_input("javascript:alert(1)", &engines(), "DuckDuckGo");
        assert!(out.starts_with("https://duckduckgo.com/?q="), "{out}");
    }

    #[test]
    fn sanitize_rejects_no_host() {
        assert!(sanitize_url("https://").is_err());
    }

    #[test]
    fn colon_text_is_search_not_host() {
        let out = resolve_input("hello:world", &engines(), "DuckDuckGo");
        assert!(out.starts_with("https://duckduckgo.com/?q=hello%3Aworld"), "{out}");
    }

    #[test]
    fn non_default_engine_selection() {
        let out = resolve_input("rust lang", &engines(), "Google");
        assert!(out.starts_with("https://www.google.com/search?q="), "{out}");
    }
}
