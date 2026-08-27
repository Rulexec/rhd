# Backend Testing

## Overview

Backend testing is done via unit tests in each package. Run command: `mise run test-cargo`.

## Unit Tests

Each package contains unit tests in `tests.rs` or `tests/` directories. These tests validate individual components and functions.

Run all unit tests:
```bash
mise run test-cargo
# or
cargo test
```

## Manual Testing with CLI

For integration testing, use the CLI tool (`rhd_app`) with `rhd_chat_server`:

1. Start `rhd_chat_server`
2. Use CLI commands to create chats, queue messages, and verify responses
3. Check server logs for plugin activity

See [features/cli.md](frontend/features/cli.md) for CLI usage details.

## Test Data

- Test models: `test_e2e/models/*.yaml`
- Test projects: `test_e2e/projects/`
