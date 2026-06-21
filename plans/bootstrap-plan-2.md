# Phase 2: Configuration & Models

## Target Vision
Implement model configuration loading and validation system. Load `models/*.yaml` files at daemon startup, validate structure, cache in memory.

## Key Design Decisions
- `ModelConfig` struct in `rhd_ai`: fields baseUrl, apiKey, model
- YAML deserialization with manual validation for unknown fields
- Error reporting includes exact file path + line number
- Models loaded once at startup into HashMap<String, ModelConfig>
- Daemon exits if any model invalid or missing

## Target File Structure
```
rhd/
├── packages/
│   ├── rhd_ai/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── config.rs
```

## Goal from plans/initial.md
Support model configuration via YAML files. Models define which AI platform endpoints and credentials to use. Configuration loaded at daemon startup, validated strictly, cached for scenario execution.
