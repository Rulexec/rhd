# Phase 1: Project Structure

## Target Vision
Initialize multi-crate Rust workspace with foundational crate structure. Establish `rhd_util`, `rhd_ai`, `rhd_app` crates under `packages/` directory with workspace-level dependency management.

## Key Design Decisions
- Workspace root `Cargo.toml` manages shared dependencies: tokio, serde, serde_yaml, rkyv, reqwest, thiserror
- All crates prefixed `rhd_` and placed in `packages/` folder
- `rhd_util` contains shared error types and common traits
- `rhd_ai` depends on reqwest, serde, rhd_util
- `rhd_app` depends on tokio, clap, rhd_ai, rhd_util, serde_yaml, rkyv

## Target File Structure
```
rhd/
├── Cargo.toml (workspace)
├── packages/
│   ├── rhd_util/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── rhd_ai/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── rhd_app/
│       ├── Cargo.toml
│       └── src/main.rs
```

## Goal from plans/initial.md
Establish multi-crate workspace foundation. All crates prefixed `rhd_` placed in `packages/` folder, linked as workspace crates in root Cargo.toml. Main crate `rhd_app` contains binary, `rhd_util` stores shared utils, `rhd_ai` implements AI platform interaction (OpenAI compatible API initially).
