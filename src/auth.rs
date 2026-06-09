//! Shared auth-header parsing.

/// Extract the credential from an `Authorization` header value.
///
/// Trims surrounding whitespace, accepts the scheme case-insensitively
/// (`Bearer ` or `bearer `), trims the token, and rejects an empty token.
/// This is the canonical, lenient-but-safe form — both repos previously had
/// drifting copies (one strict single-space, one trimmed/case-insensitive).
///
/// Callers fetch the header themselves, e.g.
/// `headers.get(AUTHORIZATION)?.to_str().ok().and_then(strip_bearer)`.
pub fn strip_bearer(header_value: &str) -> Option<&str> {
    let v = header_value.trim();
    let rest = v
        .strip_prefix("Bearer ")
        .or_else(|| v.strip_prefix("bearer "))?
        .trim();
    if rest.is_empty() { None } else { Some(rest) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_scheme_case_insensitively_and_trims() {
        assert_eq!(strip_bearer("Bearer abc"), Some("abc"));
        assert_eq!(strip_bearer("bearer abc"), Some("abc"));
        assert_eq!(strip_bearer("  Bearer   abc  "), Some("abc"));
    }

    #[test]
    fn rejects_other_schemes_and_empty() {
        assert_eq!(strip_bearer("Basic abc"), None);
        assert_eq!(strip_bearer("Bearer "), None);
        assert_eq!(strip_bearer("Bearer    "), None);
        assert_eq!(strip_bearer(""), None);
        assert_eq!(strip_bearer("abc"), None);
    }
}
