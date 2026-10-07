//! Secret and credential redaction engine for logs, traces, and stored runs.

use regex::Regex;

/// Redactor identifying and masking credentials and sensitive tokens.
pub struct Redactor {
    patterns: Vec<Regex>,
}

impl Redactor {
    /// Create a new [`Redactor`] initialized with security masking patterns.
    pub fn new() -> Self {
        let patterns = vec![
            // OpenAI / generic API keys (sk-...)
            Regex::new(r"sk-[a-zA-Z0-9_\-]{20,}").unwrap(),
            // GitHub personal access tokens
            Regex::new(r"gh[pousr]_[a-zA-Z0-9]{36,}").unwrap(),
            Regex::new(r"github_pat_[a-zA-Z0-9_]{60,}").unwrap(),
            // AWS Access Key ID
            Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
            // Bearer tokens in Authorization headers
            Regex::new(r"(?i)bearer\s+[a-zA-Z0-9_\-\.]{15,}").unwrap(),
            // Generic password/token assignments in key-value strings
            Regex::new(r#"(?i)(api[_-]?key|password|secret|token)\s*[:=]\s*["']?([a-zA-Z0-9_\-]{8,})["']?"#).unwrap(),
        ];

        Self { patterns }
    }

    /// Redact recognized secrets in text, replacing them with `[REDACTED]`.
    pub fn redact(&self, input: &str) -> String {
        let mut result = input.to_string();

        for regex in &self.patterns {
            result = regex
                .replace_all(&result, |caps: &regex::Captures| {
                    if caps.len() > 2 {
                        // Key-value capture: preserve the key name, mask only the secret value
                        format!("{}: \"[REDACTED]\"", &caps[1])
                    } else {
                        "[REDACTED]".to_string()
                    }
                })
                .to_string();
        }

        result
    }

    /// Recursively redact strings inside a JSON value.
    pub fn redact_json(&self, val: &serde_json::Value) -> serde_json::Value {
        match val {
            serde_json::Value::String(s) => serde_json::Value::String(self.redact(s)),
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|item| self.redact_json(item)).collect())
            }
            serde_json::Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    let key_lower = k.to_lowercase();
                    if key_lower.contains("secret")
                        || key_lower.contains("token")
                        || key_lower.contains("password")
                        || key_lower.contains("key")
                    {
                        new_map.insert(
                            k.clone(),
                            serde_json::Value::String("[REDACTED]".to_string()),
                        );
                    } else {
                        new_map.insert(k.clone(), self.redact_json(v));
                    }
                }
                serde_json::Value::Object(new_map)
            }
            other => other.clone(),
        }
    }
}

impl Default for Redactor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_redact_api_keys() {
        let redactor = Redactor::new();

        let text = "My OpenAI key is sk-1234567890abcdef1234567890 and AWS is AKIAIOSFODNN7EXAMPLE";
        let cleaned = redactor.redact(text);
        assert!(!cleaned.contains("sk-1234567890abcdef1234567890"));
        assert!(!cleaned.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(cleaned.contains("[REDACTED]"));
    }

    #[test]
    fn test_redact_json_sensitive_keys() {
        let redactor = Redactor::new();

        let val = json!({
            "username": "admin",
            "api_key": "super_secret_token_12345",
            "nested": {
                "password": "my_hidden_password"
            }
        });

        let cleaned = redactor.redact_json(&val);
        assert_eq!(cleaned["username"], "admin");
        assert_eq!(cleaned["api_key"], "[REDACTED]");
        assert_eq!(cleaned["nested"]["password"], "[REDACTED]");
    }
}
