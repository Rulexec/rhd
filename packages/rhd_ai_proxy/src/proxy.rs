//! HTTP forwarding: URL building, header filtering, request forwarding, streaming pass-through.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::HeaderName;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Router;
use reqwest::Url;

use crate::config::{ConfigError, ModelConfig, Proxy as ProxyConfig};
use crate::transform::{inject_extra_body, normalize_path};

const MAX_REQUEST_BODY_BYTES: usize = 50 * 1024 * 1024;

/// Shared state for the proxy handler.
pub struct ProxyState {
    client: reqwest::Client,
    target_base: Url,
    models: HashMap<String, ModelConfig>,
}

impl ProxyState {
    pub fn new(config: &ProxyConfig) -> Result<Self, ConfigError> {
        Ok(Self {
            client: reqwest::Client::new(),
            target_base: config.target_url()?,
            models: config.models.clone(),
        })
    }
}

/// Router that forwards every method on every path through the proxy.
pub fn build_router(state: Arc<ProxyState>) -> Router {
    Router::new().fallback(handle_proxy).with_state(state)
}

async fn handle_proxy(State(state): State<Arc<ProxyState>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    tracing::info!(method = %parts.method, path = %parts.uri.path(), "incoming request");
    let normalized_path = normalize_path(parts.uri.path());
    let target_url =
        match build_target_url(&state.target_base, &normalized_path, parts.uri.query()) {
            Ok(url) => url,
            Err(err) => return (StatusCode::BAD_GATEWAY, err).into_response(),
        };

    let raw_body = match axum::body::to_bytes(body, MAX_REQUEST_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("failed to read request body: {err}"),
            )
                .into_response()
        }
    };
    let outcome = inject_extra_body(&normalized_path, &raw_body, &state.models);
    match &outcome.injected_model {
        Some(model) => tracing::info!(%model, %target_url, "applied model extraBody override"),
        None => tracing::debug!(path = %normalized_path, "no extraBody override applied"),
    }

    let mut builder = state.client.request(parts.method, target_url.clone());
    for (name, value) in parts.headers {
        let Some(name) = name else { continue };
        if is_forwardable_request_header(&name) {
            builder = builder.header(name, value);
        }
    }

    match builder.body(outcome.body).send().await {
        Ok(upstream) => {
            tracing::info!(status = %upstream.status(), "upstream responded");
            pipe_upstream_response(upstream).await
        }
        Err(err) => {
            tracing::error!(%err, %target_url, "upstream request failed");
            (
                StatusCode::BAD_GATEWAY,
                format!("upstream request failed: {err}"),
            )
                .into_response()
        }
    }
}

/// Appends the normalized path (and query) to the target base path.
fn build_target_url(
    base: &Url,
    normalized_path: &str,
    query: Option<&str>,
) -> Result<Url, String> {
    let mut url = base.clone();
    let base_path = base.path().trim_end_matches('/');
    let suffix = normalized_path.trim_start_matches('/');
    let joined = if suffix.is_empty() {
        if base_path.is_empty() {
            "/".to_string()
        } else {
            base_path.to_string()
        }
    } else {
        format!("{}/{}", base_path, suffix)
    };
    url.set_path(&joined);
    let _ = url.set_query(query);
    Ok(url)
}

/// Pipes the upstream response (status, headers, raw byte stream) back to the client.
async fn pipe_upstream_response(upstream: reqwest::Response) -> Response {
    let status = upstream.status();
    let headers = upstream.headers().clone();
    let mut builder = Response::builder().status(status);
    if let Some(response_headers) = builder.headers_mut() {
        for (name, value) in &headers {
            if is_forwardable_response_header(name) {
                response_headers.insert(name.clone(), value.clone());
            }
        }
    }
    builder
        .body(Body::from_stream(upstream.bytes_stream()))
        .unwrap_or_else(|err| {
            (
                StatusCode::BAD_GATEWAY,
                format!("invalid upstream response: {err}"),
            )
                .into_response()
        })
}

fn is_hop_by_hop(name: &HeaderName) -> bool {
    const HOP_BY_HOP: [&str; 7] = [
        "connection",
        "keep-alive",
        "proxy-connection",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ];
    HOP_BY_HOP
        .iter()
        .any(|header| name.as_str().eq_ignore_ascii_case(header))
}

fn is_forwardable_request_header(name: &HeaderName) -> bool {
    // `Host` is derived from the target URL by reqwest; `content-length` is recomputed
    // from the (possibly rewritten) body.
    if name.as_str() == "host" || name.as_str() == "content-length" {
        return false;
    }
    !is_hop_by_hop(name)
}

fn is_forwardable_response_header(name: &HeaderName) -> bool {
    !is_hop_by_hop(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn builds_target_url_with_v1_normalized() {
        let base = url("https://example.org/raw/openrouter/v1");
        let normalized = normalize_path("/v1/chat/completions");
        let built = build_target_url(&base, &normalized, None).unwrap();
        assert_eq!(
            built.as_str(),
            "https://example.org/raw/openrouter/v1/chat/completions"
        );
    }

    #[test]
    fn builds_target_url_without_v1_prefix() {
        let base = url("https://example.org/raw/openrouter/v1");
        let normalized = normalize_path("/chat/completions");
        let built = build_target_url(&base, &normalized, None).unwrap();
        assert_eq!(
            built.as_str(),
            "https://example.org/raw/openrouter/v1/chat/completions"
        );
    }

    #[test]
    fn builds_target_url_with_query_and_trailing_slash_base() {
        let base = url("http://127.0.0.1:9000/v1/");
        let normalized = normalize_path("/v1/chat/completions");
        let built = build_target_url(&base, &normalized, Some("foo=bar")).unwrap();
        assert_eq!(built.as_str(), "http://127.0.0.1:9000/v1/chat/completions?foo=bar");
    }

    #[test]
    fn builds_target_url_on_host_only_base() {
        let base = url("http://127.0.0.1:9000");
        let normalized = normalize_path("/v1/chat/completions");
        let built = build_target_url(&base, &normalized, None).unwrap();
        assert_eq!(built.as_str(), "http://127.0.0.1:9000/chat/completions");
    }

    #[test]
    fn request_headers_drop_host_content_length_and_hop_by_hop() {
        assert!(!is_forwardable_request_header(&header::HOST));
        assert!(!is_forwardable_request_header(&header::CONTENT_LENGTH));
        assert!(!is_forwardable_request_header(&header::CONNECTION));
        assert!(!is_forwardable_request_header(&header::TRANSFER_ENCODING));
        assert!(is_forwardable_request_header(&header::AUTHORIZATION));
        assert!(is_forwardable_request_header(&header::CONTENT_TYPE));
        assert!(is_forwardable_request_header(&header::ACCEPT));
    }

    #[test]
    fn response_headers_keep_content_type_but_drop_transfer_encoding() {
        assert!(!is_forwardable_response_header(&header::TRANSFER_ENCODING));
        assert!(!is_forwardable_response_header(&header::UPGRADE));
        assert!(is_forwardable_response_header(&header::CONTENT_TYPE));
        assert!(is_forwardable_response_header(&header::CONTENT_LENGTH));
    }
}
