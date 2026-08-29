//! CLI command definitions

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rhd")]
#[command(about = "CLI for rhd_chat_server")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Chat operations
    Chats {
        #[command(subcommand)]
        subcommand: ChatsSubcommand,
    },
    /// View chat messages
    Messages {
        /// Chat ID
        chat_id: i64,
        /// Show all messages (default: only last)
        #[arg(long)]
        all: bool,
    },
    /// Queue operations
    Queue {
        #[command(subcommand)]
        subcommand: QueueSubcommand,
    },
    /// Create a new chat
    CreateChat {
        /// Chat title
        title: String,
        /// Optional tags (space-separated)
        #[arg(long, num_args = 0..)]
        tags: Option<Vec<String>>,
    },
    /// Plugin operations
    Plugins {
        #[command(subcommand)]
        subcommand: PluginsSubcommand,
    },
    /// Start services from YAML config
    Start {
        /// Path to YAML config file
        config_path: String,
    },
}

#[derive(Subcommand)]
pub enum ChatsSubcommand {
    /// List all chats
    List,
    /// Add tags to a chat
    AddTag {
        /// Chat ID
        chat_id: i64,
        /// Tags to add (space-separated)
        #[arg(required = true, num_args = 1..)]
        tags: Vec<String>,
    },
    /// Remove tags from a chat
    RemoveTag {
        /// Chat ID
        chat_id: i64,
        /// Tags to remove (space-separated)
        #[arg(required = true, num_args = 1..)]
        tags: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum QueueSubcommand {
    /// Show queued messages for a chat
    List {
        /// Chat ID
        chat_id: i64,
    },
    /// Add a message to the queue
    Add {
        /// Chat ID
        chat_id: i64,
        /// Message content
        content: String,
    },
}

#[derive(Subcommand)]
pub enum PluginsSubcommand {
    /// List all plugins
    List,
    /// Remove a plugin
    Remove {
        /// Plugin ID
        plugin_id: String,
    },
}
