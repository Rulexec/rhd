# Testing Infrastructure

## Purpose
Testing for backend (Rust). Tests validate unit functionality and integration workflows.

## Unit Tests

### Cargo Unit Tests
Run with: `mise run test-cargo`

These tests validate individual components and functions within each package.

## Manual Testing with CLI

The CLI tool (`rhd_app`) provides a manual testing workflow for the chat server and plugins:

1. **Start server**: Run `rhd_chat_server`
2. **Create chat**: Use `rhd create-chat <title>`
3. **Queue message**: Use `rhd queue add <chat_id> <content>`
4. **Verify processing**: Check server logs for plugin activity
5. **Check results**: Use `rhd messages --all <chat_id>` to view responses

This workflow is particularly useful for testing `rhd_plugin_ai_completions`.

## Running Tests

### Mise Commands
```bash
mise run test-cargo              # cargo unit tests
mise run check-cargo             # cargo compilation check
```

### Manual Execution
```bash
# Unit tests
cargo test

# Compilation check
cargo check
```
