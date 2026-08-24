//! RHD Chat Server library — WebSocket server for chat storage and management.
//!
//! This library provides the core functionality for the RHD Chat Server.

pub mod config;
pub mod connection;
pub mod custom_events;
pub mod error;
pub mod events;
pub mod handlers;
pub mod plugins;
pub mod server;
pub mod streams;
pub mod subscriptions;
