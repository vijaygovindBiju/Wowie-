//! Asynchronous client for OpenAI-compatible chat completion endpoints (OpenAI & local Ollama).
//!
//! Features:
//! - Zero OS execution authority: uses pure in-process `reqwest::Client` (no subshells or curl).
//! - Compatible with OpenAI `/v1/chat/completions` and local Ollama.
//! - Redacts API keys in `std::fmt::Debug`.
//! - Supports configurable timeout, model name, temperature, max tokens, and JSON mode.
//! - Maps HTTP and network transport errors directly into `CoachError`.

use crate::error::CoachError;
use crate::models::response::{CoachRawResponse, TokenUsage};
use crate::prompt::CoachPrompt;
use crate::provider::trait_def::CoachProvider;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Maximum character length of HTTP error messages embedded in `CoachError`.
pub const MAX_HTTP_ERROR_CHARS: usize = 500;

/// Output response format mode for OpenAI chat completions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResponseFormatMode {
    /// Enforces JSON object mode: `{"type": "json_object"}`.
    /// Universally supported by OpenAI, local Ollama, vLLM, and LM Studio.
    #[default]
    JsonObject,

    /// Native structured outputs via JSON Schema: `{"type": "json_schema", ...}`.
    JsonSchema,

    /// No explicit response_format parameter in request body.
    None,
}

/// Configuration options for `OpenAiCompatibleClient`.
#[derive(Clone)]
pub struct OpenAiConfig {
    /// Endpoint URL, e.g. "http://localhost:11434/v1/chat/completions" or "https://api.openai.com/v1/chat/completions".
    pub endpoint_url: String,

    /// Model identifier, e.g. "llama3:8b", "gpt-4o-mini", "mistral".
    pub model_name: String,

    /// Optional API key. If None, no Authorization header is sent (standard for local Ollama).
    pub api_key: Option<String>,

    /// Request timeout (default: 15 seconds).
    pub timeout: Duration,

    /// Sampling temperature (0.0 to 2.0, default: 0.2 for deterministic coaching).
    pub temperature: Option<f32>,

    /// Maximum completion tokens (default: 1024).
    pub max_tokens: Option<u32>,

    /// Response format configuration.
    pub response_format: ResponseFormatMode,

    /// Custom headers (e.g. organization ID, custom user-agent).
    pub custom_headers: Vec<(String, String)>,
}

impl std::fmt::Debug for OpenAiConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redacted_headers: Vec<(String, String)> = self
            .custom_headers
            .iter()
            .map(|(k, v)| {
                let lower = k.to_ascii_lowercase();
                if lower.contains("authorization")
                    || lower.contains("key")
                    || lower.contains("token")
                    || lower.contains("secret")
                {
                    (k.clone(), "[REDACTED]".to_string())
                } else {
                    (k.clone(), v.clone())
                }
            })
            .collect();

        f.debug_struct("OpenAiConfig")
            .field("endpoint_url", &self.endpoint_url)
            .field("model_name", &self.model_name)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("timeout", &self.timeout)
            .field("temperature", &self.temperature)
            .field("max_tokens", &self.max_tokens)
            .field("response_format", &self.response_format)
            .field("custom_headers", &redacted_headers)
            .finish()
    }
}

impl OpenAiConfig {
    /// Creates a new configuration with default settings.
    pub fn new(endpoint_url: impl Into<String>, model_name: impl Into<String>) -> Self {
        Self {
            endpoint_url: endpoint_url.into().trim().to_string(),
            model_name: model_name.into().trim().to_string(),
            api_key: None,
            timeout: Duration::from_secs(15),
            temperature: Some(0.2),
            max_tokens: Some(1024),
            response_format: ResponseFormatMode::JsonObject,
            custom_headers: Vec::new(),
        }
    }

    /// Convenience preset for local Ollama running on default port.
    pub fn ollama(model_name: impl Into<String>) -> Self {
        Self::new("http://localhost:11434/v1/chat/completions", model_name)
    }

    /// Convenience preset for official OpenAI API endpoints with an API key.
    pub fn openai(model_name: impl Into<String>, api_key: impl Into<String>) -> Self {
        let mut cfg = Self::new("https://api.openai.com/v1/chat/completions", model_name);
        cfg.api_key = Some(api_key.into().trim().to_string());
        cfg
    }

    /// Sets the API key.
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into().trim().to_string());
        self
    }

    /// Sets the request timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the sampling temperature (validated between 0.0 and 2.0).
    pub fn with_temperature(mut self, temp: f32) -> Result<Self, CoachError> {
        if !temp.is_finite() || !(0.0..=2.0).contains(&temp) {
            return Err(CoachError::schema_validation(
                "temperature",
                format!("Temperature {temp} must be a finite float between 0.0 and 2.0"),
            ));
        }
        self.temperature = Some(temp);
        Ok(self)
    }

    /// Sets the maximum response completion tokens.
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Sets the response format mode.
    #[must_use]
    pub fn with_response_format(mut self, mode: ResponseFormatMode) -> Self {
        self.response_format = mode;
        self
    }

    /// Appends a custom HTTP header.
    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.push((key.into(), value.into()));
        self
    }

    /// Validates configuration parameters.
    pub fn validate(&self) -> Result<(), CoachError> {
        if self.endpoint_url.is_empty() {
            return Err(CoachError::schema_validation(
                "endpoint_url",
                "Endpoint URL cannot be empty",
            ));
        }
        if !self.endpoint_url.starts_with("http://") && !self.endpoint_url.starts_with("https://") {
            return Err(CoachError::schema_validation(
                "endpoint_url",
                "Endpoint URL must start with 'http://' or 'https://'",
            ));
        }
        if self.model_name.is_empty() {
            return Err(CoachError::schema_validation(
                "model_name",
                "Model name cannot be empty",
            ));
        }
        if self.timeout.is_zero() {
            return Err(CoachError::schema_validation(
                "timeout",
                "Request timeout cannot be zero",
            ));
        }
        if let Some(temp) = self.temperature {
            if !temp.is_finite() || !(0.0..=2.0).contains(&temp) {
                return Err(CoachError::schema_validation(
                    "temperature",
                    "Temperature must be between 0.0 and 2.0",
                ));
            }
        }
        Ok(())
    }
}

/// Asynchronous client for OpenAI-compatible chat completion endpoints.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleClient {
    client: reqwest::Client,
    config: OpenAiConfig,
}

impl OpenAiCompatibleClient {
    /// Creates a new `OpenAiCompatibleClient` from configuration.
    pub fn new(config: OpenAiConfig) -> Result<Self, CoachError> {
        config.validate()?;
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| CoachError::network(format!("Failed to build reqwest client: {e}")))?;

        Ok(Self { client, config })
    }

    /// Creates a new client with a custom pre-configured `reqwest::Client`.
    pub fn with_custom_client(
        client: reqwest::Client,
        config: OpenAiConfig,
    ) -> Result<Self, CoachError> {
        config.validate()?;
        Ok(Self { client, config })
    }

    /// Returns a reference to the active configuration.
    pub fn config(&self) -> &OpenAiConfig {
        &self.config
    }

    /// Builds the OpenAI Chat Completions JSON request payload from a `CoachPrompt`.
    pub fn build_request_payload(&self, prompt: &CoachPrompt) -> serde_json::Value {
        let messages = prompt.to_chat_messages();

        let mut payload = serde_json::json!({
            "model": self.config.model_name,
            "messages": messages,
        });

        let obj = payload.as_object_mut().expect("payload is an object");

        if let Some(temp) = self.config.temperature {
            obj.insert("temperature".to_string(), serde_json::json!(temp));
        }

        if let Some(max_tokens) = self.config.max_tokens {
            obj.insert("max_tokens".to_string(), serde_json::json!(max_tokens));
        }

        match self.config.response_format {
            ResponseFormatMode::JsonObject => {
                obj.insert(
                    "response_format".to_string(),
                    serde_json::json!({ "type": "json_object" }),
                );
            }
            ResponseFormatMode::JsonSchema => {
                if let Some(schema) = &prompt.json_schema {
                    obj.insert(
                        "response_format".to_string(),
                        serde_json::json!({
                            "type": "json_schema",
                            "json_schema": {
                                "name": "coach_response",
                                "strict": true,
                                "schema": schema
                            }
                        }),
                    );
                } else {
                    obj.insert(
                        "response_format".to_string(),
                        serde_json::json!({ "type": "json_object" }),
                    );
                }
            }
            ResponseFormatMode::None => {}
        }

        payload
    }

    /// Parses the raw JSON response body from OpenAI / Ollama chat completions API.
    pub fn parse_completion_response(&self, body: &str) -> Result<CoachRawResponse, CoachError> {
        let val: serde_json::Value = serde_json::from_str(body).map_err(|e| {
            CoachError::malformed_json(
                format!("Failed to parse chat completion JSON: {e}"),
                Some(e.line()),
                Some(e.column()),
            )
        })?;

        let choices = val
            .get("choices")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                CoachError::schema_validation("choices", "Response missing 'choices' array")
            })?;

        if choices.is_empty() {
            return Err(CoachError::schema_validation(
                "choices",
                "Chat completion response returned zero choices",
            ));
        }

        let first_choice = &choices[0];
        let message = first_choice.get("message").ok_or_else(|| {
            CoachError::schema_validation("message", "First choice missing 'message' object")
        })?;

        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .ok_or_else(|| {
                CoachError::schema_validation("content", "Choice message missing 'content' string")
            })?;

        // Delegate content parsing and strict schema validation to CoachRawResponse::from_json
        let raw = CoachRawResponse::from_json(content)?;

        // Attach telemetry if present in response
        let token_usage = val.get("usage").map(|u| {
            let prompt_tokens = u.get("prompt_tokens").and_then(|p| p.as_u64()).unwrap_or(0) as u32;
            let completion_tokens = u
                .get("completion_tokens")
                .and_then(|c| c.as_u64())
                .unwrap_or(0) as u32;
            let total_tokens = u.get("total_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
            TokenUsage::new(prompt_tokens, completion_tokens, total_tokens)
        });

        let model = val
            .get("model")
            .and_then(|m| m.as_str())
            .map(ToString::to_string)
            .or_else(|| Some(self.config.model_name.clone()));

        Ok(raw.with_telemetry(
            Some(content.to_string()),
            Some(val),
            token_usage,
            None,
            model,
        ))
    }

    /// Maps a `reqwest::Error` into a strongly-typed `CoachError`.
    pub fn map_reqwest_error(&self, err: reqwest::Error) -> CoachError {
        if err.is_timeout() {
            CoachError::timeout(self.config.timeout.as_millis() as u64)
        } else if err.is_connect() {
            CoachError::network(format!(
                "Failed to connect to provider endpoint '{}': {}",
                self.config.endpoint_url, err
            ))
        } else {
            CoachError::network(format!(
                "Network error communicating with provider '{}': {}",
                self.config.endpoint_url, err
            ))
        }
    }

    /// Maps an HTTP non-2xx status code into a strongly-typed `CoachError`.
    pub fn map_http_error(&self, status_code: u16, body: &str) -> CoachError {
        let error_msg = if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
            if let Some(err_obj) = val.get("error") {
                if let Some(msg) = err_obj.get("message").and_then(|m| m.as_str()) {
                    msg.to_string()
                } else if let Some(msg) = err_obj.as_str() {
                    msg.to_string()
                } else {
                    body.to_string()
                }
            } else {
                body.to_string()
            }
        } else {
            body.to_string()
        };

        let bounded_msg = match error_msg.char_indices().nth(MAX_HTTP_ERROR_CHARS) {
            Some((byte_idx, _)) => format!("{}...", &error_msg[..byte_idx]),
            None => error_msg,
        };

        match status_code {
            401 | 403 => CoachError::provider_unavailable(
                "openai_compatible",
                format!("Authentication failed (HTTP {status_code}): {bounded_msg}"),
            ),
            404 => CoachError::http(
                404,
                format!(
                    "Endpoint or model '{}' not found: {bounded_msg}",
                    self.config.model_name
                ),
            ),
            429 => CoachError::http(429, format!("Rate limit exceeded: {bounded_msg}")),
            _ => CoachError::http(status_code, bounded_msg),
        }
    }
}

#[async_trait::async_trait]
impl CoachProvider for OpenAiCompatibleClient {
    fn name(&self) -> &'static str {
        "openai_compatible"
    }

    fn model_name(&self) -> &str {
        &self.config.model_name
    }

    async fn evaluate(&self, prompt: &CoachPrompt) -> Result<CoachRawResponse, CoachError> {
        prompt.validate()?;

        let payload = self.build_request_payload(prompt);
        let start = std::time::Instant::now();

        let mut req = self.client.post(&self.config.endpoint_url).json(&payload);

        if let Some(api_key) = &self.config.api_key {
            req = req.bearer_auth(api_key);
        }

        for (key, value) in &self.config.custom_headers {
            req = req.header(key, value);
        }

        let resp = req
            .send()
            .await
            .map_err(|err| self.map_reqwest_error(err))?;

        let status = resp.status();
        let status_code = status.as_u16();

        let body_text = resp
            .text()
            .await
            .map_err(|err| self.map_reqwest_error(err))?;

        if !status.is_success() {
            return Err(self.map_http_error(status_code, &body_text));
        }

        let latency_ms = start.elapsed().as_millis() as u64;
        let mut parsed = self.parse_completion_response(&body_text)?;
        parsed.latency_ms = Some(latency_ms);

        Ok(parsed)
    }
}
