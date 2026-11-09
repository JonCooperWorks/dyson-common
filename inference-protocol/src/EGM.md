# dyson-inference-protocol (src)

## Overview

Library source for the `dyson-inference-protocol` crate (`lib` path
`src/mod.rs`). It is a framework-neutral contract for the customer-facing
`/v1/*` inference surface:

- classifying routes, including the `/openai` and `/anthropic` base-prefix aliases
- translating Anthropic Messages and OpenAI Responses requests into OpenAI Chat Completions
- re-encoding upstream Chat Completions responses (single JSON or SSE) back into the client's dialect
- SDK-shaped error envelopes
- a generic deterministic model router

Hosts keep authentication, billing, policy, credentials, and persistence.

## Interface

### Route surface (`surface.rs`, re-exported from `mod.rs`)

- `InferenceOperation`: `ChatCompletions`, `Models`, `Messages`, `Responses` [surface.rs:InferenceOperation]
- `InferenceRoute { upstream_path, dialect, default_model_namespace, operation }`: a classified request, independent of any HTTP framework [surface.rs:InferenceRoute]
- `supported_routes()`: `POST /v1/chat/completions`, `GET /v1/models`, `POST /v1/messages`, `POST /v1/responses` [surface.rs:supported_routes]
- `classify_route(method, path)`: strips an optional `/openai` or `/anthropic` prefix, then matches the route table exactly [surface.rs:classify_route] [surface.rs:ROUTES]
- `dialect_for_path(path)`: picks the error dialect before authentication or routing [surface.rs:dialect_for_path]
- `qualify_model(body, namespace)`: prefixes a bare top-level `model` with `namespace/` [surface.rs:qualify_model]
- `is_bedrock_model(model)`: recognises Bedrock model IDs, with or without a region prefix [surface.rs:is_bedrock_model]

### Dialects and errors (`mod.rs`)

- `Dialect`: `Anthropic`, `Responses`, `BedrockGateway` [mod.rs:Dialect]
- `Dialect::translate_request`: converts a client request into a Chat Completions body [mod.rs:Dialect::translate_request]
- `Dialect::request_error_body`: error envelope for a request that failed translation [mod.rs:Dialect::request_error_body]
- `Dialect::error_type`: per-SDK spelling of each error class [mod.rs:Dialect::error_type]
- `ErrorKind`: `RateLimit`, `Permission`, `Authentication`, `InvalidRequest`, `NotFound`, `Billing`, `Upstream` [mod.rs:ErrorKind]
- `ErrorKind::openai_type`: the OpenAI spelling of each error class [mod.rs:ErrorKind::openai_type]
- `error_body` and `error_body_with_code`: build the dialect error envelope, with an optional `error.code` [mod.rs:error_body] [mod.rs:error_body_with_code]
- `sanitize_upstream_error(dialect, status, raw)`: re-wraps an upstream error body and keeps only its message [mod.rs:sanitize_upstream_error]

### Response re-encoding (`mod.rs`)

- `ResponseTranslator::build`: per-request response encoder [mod.rs:ResponseTranslator::build]
- `ResponseTranslator::build_with_request`: same as `build`, plus tool names from the original request [mod.rs:ResponseTranslator::build_with_request]
- `ResponseTranslator::errors_only`: error-only encoder for the native OpenAI path [mod.rs:ResponseTranslator::errors_only]
- `TranslatingStream::new(inner, translator)`: a `futures::Stream<Item = Result<Bytes, io::Error>>` adapter that applies a `ResponseTranslator` [mod.rs:TranslatingStream]

### Model routing (`model_routing.rs`, `pub mod model_routing`)

- `ModelRoute<T> { pattern, is_regex, enabled, value }` [model_routing.rs:ModelRoute]
- `ModelRouter<T>` and its methods: `build`, `resolve`, `catch_all`, `values`, `len`, `is_empty` [model_routing.rs:ModelRouter]
- `expand_model_rewrite(pattern, model, template)`: expands regex capture templates such as `$1` or `${name}` [model_routing.rs:expand_model_rewrite]

### Dialect shims (internal)

- `anthropic` (`pub mod`, but only `pub(crate)` and `pub(super)` items): Anthropic Messages to and from Chat Completions [anthropic.rs:translate_request] [anthropic.rs:StreamState] [anthropic.rs:completion_to_json]
- `responses` (`pub mod`, but only `pub(crate)` and `pub(super)` items): OpenAI Responses to and from Chat Completions, plus the MCP tool-name restoration helper `ToolNames` [responses.rs:translate_request] [responses.rs:ToolNames]
- `openai` (private): `OpenAiSseParser`, `ChatEvent`, `ChatCompletion`, and `parse_chat_completion`, the decode halves both shims share [openai.rs:OpenAiSseParser] [openai.rs:parse_chat_completion]

## Invariants

- Billing invariant (documented in the module docs): hosts must meter usage from the untranslated upstream OpenAI bytes, and wrap `TranslatingStream` downstream of their metering body. Translation only re-encodes the copy the client sees [mod.rs:TranslatingStream]
- A non-2xx upstream status always selects the buffering `ErrorEncoder`, whatever the `streaming` flag. `BedrockGateway` successes pass through untouched [mod.rs:ResponseTranslator::build_with_request]
- Upstream errors are reduced to `error.message`, or `detail` when present. Sibling keys such as OpenRouter `user_id` are never relayed. The error class comes from the HTTP status: 400/422 map to `InvalidRequest`, 402 to `Billing`, 404 to `NotFound`, 429 to `RateLimit`, and everything else, including 401/403, to `Upstream` [mod.rs:sanitize_upstream_error] [mod.rs:upstream_error_kind]
- `error.code` is never added for `BedrockGateway`. Its envelope is `{"detail": message}`. Anthropic uses `{"type":"error","error":{...}}` and Responses uses `{"error":{...}}` [mod.rs:error_body_with_code] [mod.rs:error_envelope]
- `classify_route` needs an exact method and path match after the prefix is stripped. `/v1/messages` and `/v1/responses` both map upstream to `v1/chat/completions` [surface.rs:ROUTES]
- `qualify_model` leaves the model unchanged if it already contains `/` or is a Bedrock ID [surface.rs:qualify_model]
- The Responses shim is stateless. A non-null `previous_response_id` or `store: true` is rejected with an error. `model` is required [responses.rs:translate_request]
- The Anthropic shim requires `model` and passes `max_tokens` through. `system` becomes a leading system message [anthropic.rs:translate_request]
- Codex custom tools travel upstream with the `swarm_custom__` prefix, which is removed on the way back [responses.rs:CUSTOM_TOOL_PREFIX]
- MCP tool names are restored only for unambiguous aliases of tools declared in the request. Exact names win, and collisions are never guessed [responses.rs:ToolNames]
- Router precedence: exact match, then the longest trailing-`*` prefix (ties broken by pattern), then regexes in lexical pattern order, then the `*` catch-all. Disabled routes are skipped. Invalid regexes are excluded and returned as `(pattern, error)` [model_routing.rs:ModelRouter::build] [model_routing.rs:ModelRouter::resolve]

## Side Effects

- `tracing::debug!` when restoring the spelling of an MCP tool name [responses.rs:ToolNames::restore]
- No network, disk, or environment access. The crate is pure translation over caller-supplied bytes and streams.

## Verification

- Run: `cargo test -p dyson-inference-protocol` (from `dyson-common/`)
