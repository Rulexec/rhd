# Phase 5: AI Integration

## Target Vision
Implement OpenAI-compatible HTTP client for AI chat completions. Support configurable base URL, API key, model selection. Handle API errors and timeouts.

## Key Design Decisions
- `OpenAiClient` struct in `rhd_ai`: holds reqwest::Client
- Method: `chat(model: &str, system: &str, message: &str) -> Result<String>`
- HTTP POST to `{baseUrl}/chat/completions`
- Parse response, extract `choices[0].message.content`
- Structured errors: distinguish network errors vs API errors, include model name in context

## Target File Structure
```
rhd/
├── packages/
│   └── rhd_ai/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── config.rs
│           └── client.rs
```

## Goal from plans/initial.md
Implement AI platform interaction via OpenAI-compatible API. `rhd_ai` crate provides client for chat completions. Supports configurable endpoints and credentials via model configs.
