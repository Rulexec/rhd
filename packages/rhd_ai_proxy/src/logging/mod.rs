//! Optional chat logging: record every completions request passing through the proxy
//! into a standalone SQLite database for later inspection.
//!
//! Enabled by the `proxy.logging` config section. The database lives in its own file
//! (`chats.sqlite3` inside the configured folder) and is fully independent of `rhd_db`.
//!
//! Chats are identified purely from request content: prefix hashes of the message
//! history are classified against each chat's frontier (see [`classify`]), so
//! continuations, retries, and branches (sub-chats, edited resends) are attributed
//! correctly without any client cooperation. Tool calls and tool results are captured
//! into a normalized [`messages`] table alongside the raw bodies.
//!
//! Logging is best-effort: every failure is reported via `tracing` and never propagates
//! to the proxied request.

pub mod capture;
pub mod chat_match;
pub mod classify;
pub mod db;
pub mod messages;
pub mod schema;
pub mod sse;

pub use capture::{capture_body, fail_logging, start_logging, LoggingContext};
pub use chat_match::{chat_title, extract_candidate, prefix_hashes, ChatCandidate};
pub use classify::Classification;
pub use db::{LoggingDb, LoggingError, NewRequest, RecordOutcome, RequestCompletion, DB_FILE_NAME};
pub use schema::SCHEMA_VERSION;
