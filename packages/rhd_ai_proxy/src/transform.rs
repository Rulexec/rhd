//! Request body transformation: per-model `extraBody` injection.

use std::collections::HashMap;

use serde_json::Value;

use crate::config::ModelConfig;

/// Strips a leading `/v1` segment so the remainder can be appended to the target base path.
///
/// `/v1/chat/completions` -> `/chat/completions`; `/chat/completions` -> `/chat/completions`.
pub fn normalize_path(path: &str) -> String {
    let trimmed = path.trim_start_matches('/');
    let without_prefix: &str = if trimmed == "v1" || trimmed.starts_with("v1/") {
        trimmed[2..].trim_start_matches('/')
    } else {
        trimmed
    };
    format!("/{}", without_prefix)
}

/// True for OpenAI-compatible completion endpoints (`/chat/completions`, `/completions`).
pub fn is_completions_path(path: &str) -> bool {
    path.ends_with("/completions")
}

/// Outcome of [`inject_extra_body`].
#[derive(Debug)]
pub struct TransformOutcome {
    /// Body to forward to the target.
    pub body: Vec<u8>,
    /// Set when the matched model's `extraBody` was merged into the body.
    pub injected_model: Option<String>,
}

fn unchanged(body: &[u8]) -> TransformOutcome {
    TransformOutcome {
        body: body.to_vec(),
        injected_model: None,
    }
}

/// Merges the matching model's `extraBody` keys into the top level of a completions request body.
///
/// Returns the original bytes unchanged when the path is not a completions endpoint, the body is
/// not a JSON object, the `model` field is missing or unconfigured, or `extraBody` is empty.
pub fn inject_extra_body(
    normalized_path: &str,
    body: &[u8],
    models: &HashMap<String, ModelConfig>,
) -> TransformOutcome {
    if models.is_empty() || !is_completions_path(normalized_path) {
        return unchanged(body);
    }
    let Ok(Value::Object(mut parsed)) = serde_json::from_slice::<Value>(body) else {
        return unchanged(body);
    };
    let Some(model) = parsed
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return unchanged(body);
    };
    let Some(model_config) = models.get(&model) else {
        return unchanged(body);
    };
    if model_config.extra_body.is_empty() {
        return unchanged(body);
    }
    for (key, value) in &model_config.extra_body {
        parsed.insert(key.clone(), value.clone());
    }
    match serde_json::to_vec(&Value::Object(parsed)) {
        Ok(forwarded) => TransformOutcome {
            body: forwarded,
            injected_model: Some(model),
        },
        Err(_) => unchanged(body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn models_with(model: &str, extra_body: Value) -> HashMap<String, ModelConfig> {
        let extra: serde_json::Map<String, Value> = serde_json::from_value(extra_body).unwrap();
        HashMap::from([(model.to_string(), ModelConfig { extra_body: extra })])
    }

    #[test]
    fn normalize_path_strips_v1_prefix() {
        assert_eq!(normalize_path("/v1/chat/completions"), "/chat/completions");
        assert_eq!(normalize_path("/chat/completions"), "/chat/completions");
        assert_eq!(normalize_path("/v1"), "/");
        assert_eq!(normalize_path("/v1/models"), "/models");
        assert_eq!(normalize_path("/v1foo/bar"), "/v1foo/bar");
        assert_eq!(normalize_path("/"), "/");
    }

    #[test]
    fn injects_extra_body_at_top_level() {
        let models = models_with(
            "gpt-4o",
            json!({"provider": {"sort": "throughput", "max_price": {"prompt": 1, "completion": 2}}}),
        );
        let body = json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello"}]});
        let outcome = inject_extra_body("/chat/completions", body.to_string().as_bytes(), &models);
        assert_eq!(outcome.injected_model.as_deref(), Some("gpt-4o"));
        let parsed: Value = serde_json::from_slice(&outcome.body).unwrap();
        assert_eq!(parsed["model"], "gpt-4o");
        assert_eq!(parsed["messages"][0]["content"], "Hello");
        assert_eq!(parsed["provider"]["sort"], "throughput");
        assert_eq!(parsed["provider"]["max_price"]["prompt"], 1);
        assert_eq!(parsed["provider"]["max_price"]["completion"], 2);
    }

    #[test]
    fn preserves_unknown_body_fields() {
        let models = models_with("gpt-4o", json!({"provider": {"sort": "latency"}}));
        let body = json!({"model": "gpt-4o", "temperature": 0.7, "max_tokens": 100});
        let outcome = inject_extra_body("/chat/completions", body.to_string().as_bytes(), &models);
        assert_eq!(outcome.injected_model.as_deref(), Some("gpt-4o"));
        let parsed: Value = serde_json::from_slice(&outcome.body).unwrap();
        assert_eq!(parsed["temperature"], 0.7);
        assert_eq!(parsed["max_tokens"], 100);
        assert_eq!(parsed["provider"]["sort"], "latency");
    }

    #[test]
    fn leaves_non_matching_model_unchanged() {
        let models = models_with("z-ai/glm-5.3", json!({"provider": {"sort": "throughput"}}));
        let body = json!({"model": "gpt-4o", "messages": []});
        let original = body.to_string().into_bytes();
        let outcome = inject_extra_body("/chat/completions", &original, &models);
        assert_eq!(outcome.body, original);
        assert_eq!(outcome.injected_model, None);
    }

    #[test]
    fn leaves_non_completions_path_unchanged() {
        let models = models_with("gpt-4o", json!({"provider": {"sort": "throughput"}}));
        let body = json!({"model": "gpt-4o"});
        let original = body.to_string().into_bytes();
        let outcome = inject_extra_body("/models", &original, &models);
        assert_eq!(outcome.body, original);
        assert_eq!(outcome.injected_model, None);
    }

    #[test]
    fn leaves_invalid_json_unchanged() {
        let models = models_with("gpt-4o", json!({"provider": {}}));
        let original = b"not json".to_vec();
        let outcome = inject_extra_body("/chat/completions", &original, &models);
        assert_eq!(outcome.body, original);
        assert_eq!(outcome.injected_model, None);
    }

    #[test]
    fn leaves_json_array_unchanged() {
        let models = models_with("gpt-4o", json!({"provider": {}}));
        let original = b"[1,2,3]".to_vec();
        let outcome = inject_extra_body("/chat/completions", &original, &models);
        assert_eq!(outcome.body, original);
        assert_eq!(outcome.injected_model, None);
    }

    #[test]
    fn leaves_body_without_model_field_unchanged() {
        let models = models_with("gpt-4o", json!({"provider": {}}));
        let original = json!({"messages": []}).to_string().into_bytes();
        let outcome = inject_extra_body("/chat/completions", &original, &models);
        assert_eq!(outcome.body, original);
        assert_eq!(outcome.injected_model, None);
    }

    #[test]
    fn empty_extra_body_is_noop() {
        let models = models_with("gpt-4o", json!({}));
        let original = json!({"model": "gpt-4o", "messages": []}).to_string().into_bytes();
        let outcome = inject_extra_body("/chat/completions", &original, &models);
        assert_eq!(outcome.body, original);
        assert_eq!(outcome.injected_model, None);
    }

    #[test]
    fn no_models_configured_is_noop() {
        let original = json!({"model": "gpt-4o"}).to_string().into_bytes();
        let outcome = inject_extra_body("/chat/completions", &original, &HashMap::new());
        assert_eq!(outcome.body, original);
        assert_eq!(outcome.injected_model, None);
    }
}
