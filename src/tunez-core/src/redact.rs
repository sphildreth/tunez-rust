//! Log redaction utilities for preventing secret leakage.
//!
//! This module provides functions to redact sensitive information from strings
//! before logging or displaying them.

use std::borrow::Cow;

/// Patterns that indicate sensitive data that should be redacted.
const SENSITIVE_PATTERNS: &[(&str, &str)] = &[
    // Authorization headers
    ("Authorization: Bearer ", "Authorization: Bearer [REDACTED]"),
    ("Authorization: Basic ", "Authorization: Basic [REDACTED]"),
    ("authorization: bearer ", "authorization: bearer [REDACTED]"),
    ("authorization: basic ", "authorization: basic [REDACTED]"),
    // Common token query parameters
    ("token=", "token=[REDACTED]"),
    ("access_token=", "access_token=[REDACTED]"),
    ("refresh_token=", "refresh_token=[REDACTED]"),
    ("api_key=", "api_key=[REDACTED]"),
    ("apikey=", "apikey=[REDACTED]"),
    ("secret=", "secret=[REDACTED]"),
    ("password=", "password=[REDACTED]"),
    ("passwd=", "passwd=[REDACTED]"),
];

/// Redact sensitive information from a string.
///
/// This function identifies and replaces known sensitive patterns such as:
/// - Authorization headers (Bearer, Basic)
/// - URL query parameters containing tokens, API keys, passwords
/// - User credentials in URLs (user:pass@host)
///
/// # Examples
/// ```
/// use tunez_core::redact::redact_secrets;
///
/// let input = "Authorization: Bearer my_secret_token";
/// let output = redact_secrets(input);
/// assert!(!output.contains("my_secret_token"));
/// assert!(output.contains("[REDACTED]"));
/// ```
pub fn redact_secrets(input: &str) -> Cow<'_, str> {
    let mut result = Cow::Borrowed(input);

    // Handle URL credentials (user:pass@host pattern)
    if let Some(redacted) = redact_url_credentials(&result) {
        result = Cow::Owned(redacted);
    }

    // Handle known sensitive patterns
    for (pattern, replacement) in SENSITIVE_PATTERNS {
        if result.contains(pattern) {
            let redacted = redact_pattern_value(&result, pattern, replacement);
            result = Cow::Owned(redacted);
        }
    }

    result
}

/// Redact URL credentials in the format `scheme://user:pass@host`.
fn redact_url_credentials(input: &str) -> Option<String> {
    // Match patterns like https://user:password@host.com
    let patterns = ["https://", "http://", "file://"];

    for scheme in patterns {
        if let Some(start) = input.find(scheme) {
            let after_scheme = &input[start + scheme.len()..];
            if let Some(at_pos) = after_scheme.find('@') {
                // Check if there's a colon before the @ (indicating user:pass)
                if let Some(colon_pos) = after_scheme[..at_pos].find(':') {
                    // Found user:pass@ pattern
                    let user = &after_scheme[..colon_pos];
                    let rest = &after_scheme[at_pos..];
                    return Some(format!(
                        "{}{}{}:[REDACTED]{}{}",
                        &input[..start],
                        scheme,
                        user,
                        rest,
                        ""
                    ));
                }
            }
        }
    }
    None
}

/// Redact the value following a pattern, up to the next delimiter.
fn redact_pattern_value(input: &str, pattern: &str, replacement: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(pos) = remaining.find(pattern) {
        result.push_str(&remaining[..pos]);
        result.push_str(replacement);

        let after_pattern = &remaining[pos + pattern.len()..];
        // Find the end of the value (space, &, newline, or end of string)
        let end = after_pattern
            .find(|c: char| c.is_whitespace() || c == '&' || c == '\n' || c == '"' || c == '\'')
            .unwrap_or(after_pattern.len());

        remaining = &after_pattern[end..];
    }

    result.push_str(remaining);
    result
}

/// Check if a string contains any sensitive patterns.
///
/// Useful for validation or deciding whether redaction is needed.
pub fn contains_sensitive(input: &str) -> bool {
    // Check for URL credentials
    for scheme in ["https://", "http://"] {
        if let Some(start) = input.find(scheme) {
            let after = &input[start + scheme.len()..];
            if let Some(at_pos) = after.find('@') {
                if after[..at_pos].contains(':') {
                    return true;
                }
            }
        }
    }

    // Check for known patterns
    SENSITIVE_PATTERNS
        .iter()
        .any(|(pattern, _)| input.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_bearer_token() {
        let input = "Authorization: Bearer sk_live_abc123xyz";
        let output = redact_secrets(input);
        assert!(!output.contains("sk_live_abc123xyz"));
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_basic_auth() {
        let input = "Authorization: Basic dXNlcjpwYXNz";
        let output = redact_secrets(input);
        assert!(!output.contains("dXNlcjpwYXNz"));
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_query_params() {
        let input = "https://api.example.com/v1?token=secret123&other=value";
        let output = redact_secrets(input);
        assert!(!output.contains("secret123"));
        assert!(output.contains("token=[REDACTED]"));
        assert!(output.contains("other=value"));
    }

    #[test]
    fn redacts_access_token() {
        let input = "access_token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        let output = redact_secrets(input);
        assert!(!output.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    }

    #[test]
    fn redacts_url_credentials() {
        let input = "connecting to https://user:secretpass@api.example.com/api";
        let output = redact_secrets(input);
        assert!(!output.contains("secretpass"));
        assert!(output.contains("[REDACTED]"));
        assert!(output.contains("user:"));
        assert!(output.contains("@api.example.com"));
    }

    #[test]
    fn preserves_non_sensitive_data() {
        let input = "Normal log message without secrets";
        let output = redact_secrets(input);
        assert_eq!(output, input);
    }

    #[test]
    fn contains_sensitive_detects_patterns() {
        assert!(contains_sensitive("Authorization: Bearer token"));
        assert!(contains_sensitive("https://user:pass@host.com"));
        assert!(contains_sensitive("api_key=abc123"));
        assert!(!contains_sensitive("normal log message"));
    }

    #[test]
    fn redacts_multiple_occurrences() {
        let input = "token=secret1&access_token=secret2";
        let output = redact_secrets(input);
        assert!(!output.contains("secret1"));
        assert!(!output.contains("secret2"));
    }

    #[test]
    fn redacts_password_param() {
        let input = "login?user=admin&password=hunter2";
        let output = redact_secrets(input);
        assert!(!output.contains("hunter2"));
        assert!(output.contains("password=[REDACTED]"));
    }

    #[test]
    fn handles_lowercase_auth_headers() {
        let input = "authorization: bearer my_token";
        let output = redact_secrets(input);
        assert!(!output.contains("my_token"));
    }
}
