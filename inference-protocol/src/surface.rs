use serde_json::Value;

use super::Dialect;

/// An operation exposed by the OpenAI-compatible inference surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceOperation {
    ChatCompletions,
    Models,
    Messages,
    Responses,
}

/// A classified public request, independent of any HTTP framework.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InferenceRoute {
    /// Upstream path expected by an OpenAI Chat Completions provider.
    pub upstream_path: &'static str,
    /// Translation required for the caller's request and response bodies.
    pub dialect: Option<Dialect>,
    /// Namespace applied to a bare model on provider-prefixed base URLs.
    pub default_model_namespace: Option<&'static str>,
    pub operation: InferenceOperation,
}

const ROUTES: &[(&str, &str, InferenceOperation, &str, Option<Dialect>)] = &[
    (
        "POST",
        "/v1/chat/completions",
        InferenceOperation::ChatCompletions,
        "v1/chat/completions",
        None,
    ),
    (
        "GET",
        "/v1/models",
        InferenceOperation::Models,
        "v1/models",
        None,
    ),
    (
        "POST",
        "/v1/messages",
        InferenceOperation::Messages,
        "v1/chat/completions",
        Some(Dialect::Anthropic),
    ),
    (
        "POST",
        "/v1/responses",
        InferenceOperation::Responses,
        "v1/chat/completions",
        Some(Dialect::Responses),
    ),
];

/// Routes understood by [`classify_route`]. Provider prefixes are aliases of
/// these paths and intentionally do not duplicate the contract metadata.
pub fn supported_routes() -> &'static [(&'static str, &'static str, InferenceOperation)] {
    &[
        (
            "POST",
            "/v1/chat/completions",
            InferenceOperation::ChatCompletions,
        ),
        ("GET", "/v1/models", InferenceOperation::Models),
        ("POST", "/v1/messages", InferenceOperation::Messages),
        ("POST", "/v1/responses", InferenceOperation::Responses),
    ]
}

/// Classify an inference request and strip an optional provider base prefix.
pub fn classify_route(method: &str, path: &str) -> Option<InferenceRoute> {
    let (default_model_namespace, sub_path) = if let Some(rest) = path.strip_prefix("/openai") {
        (Some("openai"), rest)
    } else if let Some(rest) = path.strip_prefix("/anthropic") {
        (Some("anthropic"), rest)
    } else {
        (None, path)
    };

    ROUTES.iter().find_map(
        |(route_method, route_path, operation, upstream_path, dialect)| {
            (*route_method == method && *route_path == sub_path).then_some(InferenceRoute {
                upstream_path,
                dialect: *dialect,
                default_model_namespace,
                operation: *operation,
            })
        },
    )
}

/// Infer the response error dialect before authentication or route matching.
pub fn dialect_for_path(path: &str) -> Option<Dialect> {
    if path.starts_with("/anthropic") {
        return Some(Dialect::Anthropic);
    }
    let path = path.strip_prefix("/openai").unwrap_or(path);
    match path {
        "/v1/messages" => Some(Dialect::Anthropic),
        "/v1/responses" => Some(Dialect::Responses),
        _ => None,
    }
}

/// Prefix a bare top-level `model` while preserving fully-qualified models.
/// Returns true when the request was changed.
pub fn qualify_model(body: &mut Value, namespace: &str) -> bool {
    let Some(model) = body.get_mut("model") else {
        return false;
    };
    let Some(raw) = model.as_str() else {
        return false;
    };
    if raw.contains('/') || is_bedrock_model(raw) {
        return false;
    }
    *model = Value::String(format!("{namespace}/{raw}"));
    true
}

/// Bedrock IDs are already qualified; region prefixes are inference profiles,
/// not gateway provider namespaces. Keep their version suffixes intact.
pub fn is_bedrock_model(model: &str) -> bool {
    let id = ["us.", "eu.", "apac.", "global."]
        .iter()
        .find_map(|prefix| model.strip_prefix(prefix))
        .unwrap_or(model);
    [
        "anthropic.",
        "meta.",
        "amazon.nova-",
        "mistral.",
        "cohere.",
        "deepseek.",
    ]
    .iter()
    .any(|prefix| id.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_native_and_prefixed_routes() {
        let messages = classify_route("POST", "/anthropic/v1/messages").unwrap();
        assert_eq!(messages.dialect, Some(Dialect::Anthropic));
        assert_eq!(messages.default_model_namespace, Some("anthropic"));
        assert_eq!(messages.upstream_path, "v1/chat/completions");

        let chat = classify_route("POST", "/openai/v1/chat/completions").unwrap();
        assert_eq!(chat.dialect, None);
        assert_eq!(chat.default_model_namespace, Some("openai"));
        assert!(classify_route("GET", "/v1/chat/completions").is_none());
    }

    #[test]
    fn qualifies_only_bare_string_models() {
        let mut bare = serde_json::json!({"model": "gpt-5.1"});
        assert!(qualify_model(&mut bare, "openai"));
        assert_eq!(bare["model"], "openai/gpt-5.1");

        let mut qualified = serde_json::json!({"model": "anthropic/claude"});
        assert!(!qualify_model(&mut qualified, "openai"));
        assert_eq!(qualified["model"], "anthropic/claude");
    }
}

#[cfg(test)]
mod bedrock_tests {
    use super::*;

    #[test]
    fn bedrock_ids_survive_every_provider_base() {
        for id in [
            "anthropic.claude-3-5-sonnet-20241022-v2:0",
            "meta.llama3-70b-instruct-v1:0",
            "amazon.nova-lite-v1:0",
            "mistral.mistral-large-2407-v1:0",
            "cohere.command-r-v1:0",
            "deepseek.r1-v1:0",
        ] {
            for prefix in ["", "us.", "eu.", "apac.", "global."] {
                let model = format!("{prefix}{id}");
                for base in ["", "/openai", "/anthropic"] {
                    let route =
                        classify_route("POST", &format!("{base}/v1/chat/completions")).unwrap();
                    let mut body = serde_json::json!({"model": model});
                    if let Some(namespace) = route.default_model_namespace {
                        assert!(!qualify_model(&mut body, namespace));
                    }
                    assert_eq!(body["model"], model);
                }
            }
        }
        assert!(!is_bedrock_model("claude-sonnet-4"));
        assert!(!is_bedrock_model("gpt-4.1"));
        assert!(classify_route("POST", "/model/amazon.nova-lite-v1:0/invoke").is_none());
    }
}
