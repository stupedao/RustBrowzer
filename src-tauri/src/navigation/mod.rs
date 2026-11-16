use url::Url;

pub fn resolve_input(input: &str, search_engine_prefix: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() { return "about:blank".into(); }

    // Check for protocol
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("about:") || trimmed.starts_with("file://") {
        return sanitize_url(trimmed);
    }

    // Check for domain with dot
    if trimmed.contains('.') && !trimmed.contains(' ') {
        if Url::parse(&format!("https://{}", trimmed)).is_ok() {
            return sanitize_url(&format!("https://{}", trimmed));
        }
    }

    // Try to parse as URL
    if let Ok(parsed) = Url::parse(trimmed) {
        return sanitize_url(&parsed.to_string());
    }

    // Otherwise, treat as search query
    format!("{}{}", search_engine_prefix, encode_search_query(trimmed))
}

/// Sanitize URL to prevent XSS and ensure safety
fn sanitize_url(input: &str) -> String {
    // Basic sanitization - only allow safe protocols and characters
    if let Ok(url) = Url::parse(input) {
        // Only allow safe schemes
        match url.scheme() {
            "http" | "https" | "about" | "file" => {
                // Check hostname is valid
                if let Some(host) = url.host_str() {
                    // Allow localhost and valid domain names
                    if host == "localhost" || host.contains('.') {
                        return url.to_string();
                    }
                }
            }
            _ => {}
        }
    }
    "about:blank".to_string()
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
