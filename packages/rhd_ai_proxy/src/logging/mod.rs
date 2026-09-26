//! Optional chat logging: record every completions request passing through the proxy
//! into a standalone SQLite database for later inspection.
//!
//! Enabled by the `proxy.logging` config section. The database lives in its own file
//! (`chats.sqlite3` inside the configured folder) and is fully independent of `rhd_db`.
//!
//! Logging is best-effort: every failure is reported via `tracing` and never propagates
//! to the proxied request.

pub mod capture;
pub mod chat_match;
pub mod db;
pub mod sse;

pub use capture::{capture_body, fail_logging, start_logging, LoggingContext};
pub use chat_match::{chat_title, extract_candidate, prefix_hashes, ChatCandidate};
pub use db::{
    LoggingDb, LoggingError, NewRequest, RequestCompletion, DB_FILE_NAME,
};
pub use sse::{assemble_sse_message, extract_message_json};
