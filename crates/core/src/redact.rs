use std::sync::LazyLock;

use regex::Regex;

/// Best-effort masking of values that look like API keys, bearer tokens, or
/// other secrets, so they never reach `activity_events`, log files, or a
/// diagnostic export in full (sección 58 del brief).
///
/// This is defense in depth, not the primary guarantee: the real rule is
/// that code must never pass a secret value into a log/summary field to
/// begin with (`SecretStore` values are never read into anything that gets
/// logged). Treat a redaction miss here as a bug to fix, not as evidence
/// this function is unnecessary.
struct RedactionRule {
    pattern: Regex,
    replacement: &'static str,
}

static RULES: LazyLock<Vec<RedactionRule>> = LazyLock::new(|| {
    vec![
        // OpenAI-style secret keys: sk-XXXXXXXXXX...
        RedactionRule {
            pattern: Regex::new(r"\bsk-[A-Za-z0-9]{10,}\b").expect("valid regex"),
            replacement: "sk-******",
        },
        // Authorization: Bearer <token>
        RedactionRule {
            pattern: Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9\-_.]{10,}").expect("valid regex"),
            replacement: "Bearer ******",
        },
        // Discord bot tokens: three dot-separated base64url-ish segments.
        RedactionRule {
            pattern: Regex::new(r"\b[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{4,10}\.[A-Za-z0-9_-]{20,}\b")
                .expect("valid regex"),
            replacement: "******",
        },
        // key/token/secret/password assignments, JSON or query-string style:
        // "api_key": "...", token=..., client_secret="..."
        RedactionRule {
            pattern: Regex::new(
                r#"(?i)("?(?:api[_-]?key|token|secret|password|client[_-]?secret)"?\s*[:=]\s*"?)[A-Za-z0-9\-_.]{8,}"#,
            )
            .expect("valid regex"),
            replacement: "$1******",
        },
    ]
});

pub fn redact_secrets(input: &str) -> String {
    let mut out = input.to_string();
    for rule in RULES.iter() {
        out = rule.pattern.replace_all(&out, rule.replacement).into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_openai_style_key() {
        let input = "using key sk-abcdefghijklmnopqrstuvwxyz to call the API";
        assert_eq!(
            redact_secrets(input),
            "using key sk-****** to call the API"
        );
    }

    #[test]
    fn redacts_bearer_token() {
        let input = "Authorization: Bearer abc123.def456-token_value";
        assert_eq!(redact_secrets(input), "Authorization: Bearer ******");
    }

    #[test]
    fn redacts_key_value_assignment() {
        let input = r#"{"api_key": "abcdefghij1234567890"}"#;
        assert_eq!(redact_secrets(input), r#"{"api_key": "******"}"#);
    }

    #[test]
    fn leaves_ordinary_text_untouched() {
        let input = "El personaje respondió en 340ms usando el modelo local.";
        assert_eq!(redact_secrets(input), input);
    }
}
