# README.md Plan

## Goal
Write `README.md` documenting `rhd` usage: overview, install, CLI, scenario/model YAML schema (with optional fields marked), example scenario, global wrapper script.

## Source of truth
Verified against code:
- [`packages/rhd_app/src/scenario/mod.rs`](packages/rhd_app/src/scenario/mod.rs) — action structs
- [`packages/rhd_ai/src/config.rs`](packages/rhd_ai/src/config.rs) — `ModelConfig`
- [`packages/rhd_app/src/cli.rs`](packages/rhd_app/src/cli.rs) — CLI flags
- [`packages/rhd_app/src/scenario/placeholder.rs`](packages/rhd_app/src/scenario/placeholder.rs) — placeholder fields

## README sections

1. **Title + short description** — Rust daemon for AI-assisted scenario execution.
2. **Architecture** — daemon/client over Unix socket (`$HOME/rhd.sock` default).
3. **Install**
   - `cargo build --release`
   - Global wrapper script (bash) with mock path `~/projects/rhd/Cargo.toml` → `~/projects/rhd/target/release/rhd`.
4. **CLI**
   - `rhd daemon [--models-dir PATH] [--scenarios-dir PATH] [--default-model NAME] [--socket PATH] [--verbose]`
   - `rhd run <name> [--socket PATH]`
5. **Models** (`models/<name>.yaml`)
   - Table: `baseUrl` (required), `apiKey` (required), `model` (required). All support `$ENV_VAR`.
6. **Scenarios** (`scenarios/<name>/scenario.yaml`)
   - Top-level: `name` (required), `description` (optional), `actions` (required).
   - Action types table with required/optional fields.
   - Placeholder table: `%step.exitCode%`, `%step.stdout%`, `%step.stderr%`, `%step.stdoutStderr%`, `%step.success%`, `%step.message%`, `%step.cwd%`.
   - Env var substitution `$VAR` in all string fields.
7. **Example scenario** — user-provided `example` scenario (runCommand → aiChat → aiChat → output).
8. **Behavior notes**
   - `runCommand` never fails scenario on non-zero exit.
   - Missing placeholders resolve to empty string.
   - `cwd` defaults to client's cwd; scenario `cwd` overrides.
   - Daemon removes stale socket, handles SIGTERM/SIGINT.

## Field tables

### Model YAML
| Field | Required | Notes |
|---|---|---|
| `baseUrl` | yes | OpenAI-compatible endpoint. Supports `$ENV_VAR`. |
| `apiKey` | yes | Supports `$ENV_VAR`. |
| `model` | yes | Model identifier passed to API. Supports `$ENV_VAR`. |

### Scenario top-level
| Field | Required | Notes |
|---|---|---|
| `name` | yes | Scenario identifier. |
| `description` | no | Free text. |
| `actions` | yes | Ordered list. |

### `runCommand`
| Field | Required | Notes |
|---|---|---|
| `type` | yes | `runCommand` |
| `name` | no | Step name for placeholders. |
| `cmd` | yes | Executable. Supports `$ENV_VAR`. |
| `args` | no | Default `[]`. Each arg supports `$ENV_VAR`. |
| `cwd` | no | Default: client's cwd. Supports `$ENV_VAR`. |

### `aiChat`
| Field | Required | Notes |
|---|---|---|
| `type` | yes | `aiChat` |
| `name` | no | Step name for placeholders. |
| `model` | no | Falls back to `--default-model`. Supports `$ENV_VAR`. |
| `systemPrompt` | no | Supports `$ENV_VAR` and `%placeholder%`. |
| `message` | yes | Supports `$ENV_VAR` and `%placeholder%`. |

### `output`
| Field | Required | Notes |
|---|---|---|
| `type` | yes | `output` |
| `name` | no | Step name. |
| `output` | yes | Final text template. Supports `%placeholder%`. |

### Placeholders
| Field | Source | Notes |
|---|---|---|
| `%step.exitCode%` | `runCommand` | Integer. |
| `%step.stdout%` | `runCommand` | Trimmed stdout. |
| `%step.stderr%` | `runCommand` | Trimmed stderr. |
| `%step.stdoutStderr%` | `runCommand` | Concatenated; empty on failure. |
| `%step.success%` | `runCommand` | `true`/`false`. |
| `%step.cwd%` | `runCommand` | Resolved working dir. |
| `%step.message%` | `aiChat` | Assistant reply. |

## Wrapper script
Mock path: `~/projects/rhd/Cargo.toml`, binary `~/projects/rhd/target/release/rhd`.

## Deliverable
Single `README.md` at repo root.
