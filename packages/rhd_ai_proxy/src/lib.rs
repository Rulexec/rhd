//! OpenAI-compatible reverse proxy that injects per-model `extraBody` fields into
//! completion requests and pipes streaming responses back to the client.

pub mod config;
pub mod proxy;
pub mod transform;
