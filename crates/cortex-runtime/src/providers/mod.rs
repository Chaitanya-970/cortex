//! Live model inference provider implementations and factory functions.

pub mod anthropic;
pub mod openai;

pub use anthropic::AnthropicProvider;
pub use openai::OpenAiCompatibleProvider;

use crate::model::ModelProvider;
use cortex_core::Result;
use std::sync::Arc;

/// Approximate inference cost in USD based on token counts for known model architectures.
pub fn estimate_cost(model_name: &str, prompt_tokens: usize, completion_tokens: usize) -> f64 {
    let lower = model_name.to_lowercase();
    let (prompt_rate, completion_rate) = match lower.as_str() {
        m if m.contains("gpt-4o-mini") => (0.15, 0.60),
        m if m.contains("gpt-4o") => (2.50, 10.00),
        m if m.contains("o1") || m.contains("o3") => (15.00, 60.00),
        m if m.contains("claude-3-5-sonnet") => (3.00, 15.00),
        m if m.contains("claude-3-5-haiku") => (0.80, 4.00),
        m if m.contains("deepseek") => (0.14, 0.28),
        m if m.contains("gemini-1.5-pro") => (1.25, 5.00),
        m if m.contains("gemini-1.5-flash") => (0.075, 0.30),
        m if m.contains("ollama") || m.contains("local") => (0.0, 0.0),
        _ => (1.00, 3.00),
    };

    (prompt_tokens as f64 * prompt_rate + completion_tokens as f64 * completion_rate) / 1_000_000.0
}

/// Create an active [`ModelProvider`] instance matching the specified model name and configuration.
pub fn create_model_provider(
    model: &str,
    api_key: Option<String>,
    base_url: Option<String>,
) -> Result<Arc<dyn ModelProvider>> {
    let lower = model.to_lowercase();

    if lower.starts_with("claude") {
        let key = api_key
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .or_else(|| std::env::var("anthropic_api_key").ok());
        let url = base_url.unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());
        Ok(Arc::new(AnthropicProvider::new(model, key, Some(url))))
    } else if lower.starts_with("ollama/") {
        let stripped = model.strip_prefix("ollama/").unwrap_or(model);
        let url = base_url.unwrap_or_else(|| "http://localhost:11434/v1".to_string());
        Ok(Arc::new(OpenAiCompatibleProvider::new(
            stripped,
            None,
            Some(url),
        )))
    } else {
        let key = api_key
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .or_else(|| std::env::var("openai_api_key").ok());
        let url = base_url
            .or_else(|| std::env::var("OPENAI_API_BASE").ok())
            .or_else(|| std::env::var("openai_api_base").ok())
            .or_else(|| std::env::var("CORTEX_API_BASE").ok())
            .or_else(|| std::env::var("cortex_api_base").ok());
        Ok(Arc::new(OpenAiCompatibleProvider::new(model, key, url)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_cost_pricing() {
        let cost = estimate_cost("gpt-4o-mini", 1_000_000, 1_000_000);
        assert!((cost - 0.75).abs() < 1e-4);

        let local_cost = estimate_cost("ollama/qwen", 500_000, 500_000);
        assert_eq!(local_cost, 0.0);
    }

    #[test]
    fn test_create_model_provider_factory() {
        let claude = create_model_provider("claude-3-5-sonnet", None, None).unwrap();
        assert_eq!(claude.descriptor().provider, "anthropic");

        let ollama = create_model_provider("ollama/llama3.1", None, None).unwrap();
        assert_eq!(ollama.descriptor().provider, "ollama");

        let openai = create_model_provider("gpt-4o", None, None).unwrap();
        assert_eq!(openai.descriptor().provider, "openai");
    }
}
