use url::Url;

pub fn resolve_input(input: &str, search_engine_prefix: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() { return "about:blank".into(); }

    // Check for protocol
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("about:") || trimmed.starts_with("file://") {
        return sanitize_url(trimmed).unwrap_or_else(|_| "about:blank".into());
    }

    // Check for domain with dot
    if trimmed.contains('.') && !trimmed.contains(' ') {
        if let Ok(sanitized) = sanitize_url(&format!("https://{}", trimmed)) {
            return sanitized;
        }
    }

    // Try to parse as URL
    if let Ok(parsed) = Url::parse(trimmed) {
        return sanitize_url(&parsed.to_string()).unwrap_or_else(|_| "about:blank".into());
    }

    // Otherwise, treat as search query
    format!("{}{}", search_engine_prefix, encode_search_query(trimmed))
}

/// Sanitize URL to prevent XSS and ensure safety
pub fn sanitize_url(input: &str) -> Result<String, String> {
    // Basic sanitization - only allow safe protocols and characters
    if let Ok(url) = Url::parse(input) {
        // Only allow safe schemes
        match url.scheme() {
            "http" | "https" | "about" => return Ok(url.to_string()),
            "file" => {
                // Allow file URLs as-is (parsed)
                return Ok(url.to_string());
            }
            _ => {}
        }
    }
    Err(format!("Invalid or unsupported URL: {}", input))
}

/// Encode search query to prevent XSS
fn encode_search_query(input: &str) -> String {
    let mut out = String::new();
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// lightweight local encoder to avoid extra dep; url crate has form encoding differently
mod urlencoding {
    pub fn encode(input: &str) -> String {
        let mut out = String::new();
        for b in input.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
                b' ' => out.push_str("%20"),
                _ => out.push_str(&format!("%{:02X}", b)),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_input;

    #[test]
    fn resolves_https_urls() {
        assert_eq!(resolve_input("https://example.com", "https://www.google.com/search?q="), "https://example.com");
    }

    #[test]
    fn resolves_bare_domains() {
        assert_eq!(resolve_input("example.com", "https://www.google.com/search?q="), "https://example.com");
    }

    #[test]
    fn resolves_search_queries() {
        let out = resolve_input("rust browser", "https://www.google.com/search?q=");
        assert!(out.starts_with("https://www.google.com/search?q="));
        assert!(out.contains("rust%20browser"));
    }
}
