# Phase 2: Create New CLI Tool

## Overview

This phase creates a new `rhd_app` package as a CLI utility for interacting with `rhd_chat_server`. The CLI will use `rhd_chat_client` to communicate via WebSocket protocol and provide commands for chat management, message operations, and plugin management.

**Scope:**
- Create new `rhd_app` package structure
- Implement CLI with clap for argument parsing
- Implement all required commands using `rhd_chat_client`
- Test connectivity with `rhd_chat_server`

**Out of scope:**
- Documentation (Phase 3)
- Memory cleanup (Phase 4)

## Files to Create

### 1. `packages/rhd_app/Cargo.toml`

**Create new file:**

```toml
[package]
name = "rhd_app"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "rhd"
path = "src/main.rs"

[dependencies]
tokio = { workspace = true }
clap = { workspace = true, features = ["derive"] }
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }
rhd_chat_client = { path = "../rhd_chat_client" }
rhd_chat_api = { path = "../rhd_chat_api" }
```

### 2. `packages/rhd_app/src/main.rs`

**Create new file:**

```rust
//! RHD CLI - Command-line interface for rhd_chat_server

mod cli;
mod commands;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    match cli.command {
        cli::Command::Chats { subcommand } => {
            commands::chats::execute(subcommand).await?;
        }
        cli::Command::Messages { chat_id, all } => {
            commands::messages::execute(chat_id, all).await?;
        }
        cli::Command::Queue { subcommand } => {
            commands::queue::execute(subcommand).await?;
        }
        cli::Command::CreateChat { title } => {
            commands::create_chat::execute(title).await?;
        }
        cli::Command::Plugins { subcommand } => {
            commands::plugins::execute(subcommand).await?;
        }
    }
    
    Ok(())
}
```

### 3. `packages/rhd_app/src/cli.rs`

**Create new file:**

```rust
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
    },
    /// Plugin operations
    Plugins {
        #[command(subcommand)]
        subcommand: PluginsSubcommand,
    },
}

#[derive(Subcommand)]
pub enum ChatsSubcommand {
    /// List all chats
    List,
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
```

### 4. `packages/rhd_app/src/commands/mod.rs`

**Create new file:**

```rust
//! Command implementations

pub mod chats;
pub mod messages;
pub mod queue;
pub mod create_chat;
pub mod plugins;
```

### 5. `packages/rhd_app/src/commands/chats.rs`

**Create new file:**

```rust
//! Chat list command

use rhd_chat_client::ChatClient;
use rhd_chat_api::ListChatsParams;
use crate::cli::ChatsSubcommand;

pub async fn execute(subcommand: ChatsSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    match subcommand {
        ChatsSubcommand::List => {
            let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
            let result = client.list_chats(ListChatsParams {}).await?;
            
            println!("{}", serde_json::to_string_pretty(&result.chats)?);
            Ok(())
        }
    }
}
```

### 6. `packages/rhd_app/src/commands/messages.rs`

**Create new file:**

```rust
//! Messages view command

use rhd_chat_client::ChatClient;
use rhd_chat_api::GetChatParams;

pub async fn execute(chat_id: i64, all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
    let result = client.get_chat(GetChatParams { chat_id }).await?;
    
    let messages = if all {
        result.messages
    } else {
        // Show only last message
        result.messages.into_iter().last().into_iter().collect()
    };
    
    println!("{}", serde_json::to_string_pretty(&messages)?);
    Ok(())
}
```

### 7. `packages/rhd_app/src/commands/queue.rs`

**Create new file:**

```rust
//! Queue operations command

use rhd_chat_client::ChatClient;
use rhd_chat_api::{GetQueueMessagesParams, AddQueueMessageParams};
use crate::cli::QueueSubcommand;

pub async fn execute(subcommand: QueueSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
    
    match subcommand {
        QueueSubcommand::List { chat_id } => {
            let result = client.get_queue_messages(GetQueueMessagesParams { chat_id }).await?;
            println!("{}", serde_json::to_string_pretty(&result.messages)?);
            Ok(())
        }
        QueueSubcommand::Add { chat_id, content } => {
            let result = client.add_queue_message(AddQueueMessageParams {
                chat_id,
                content,
            }).await?;
            println!("Queued message with ID: {}", result.queue_message_id);
            Ok(())
        }
    }
}
```

### 8. `packages/rhd_app/src/commands/create_chat.rs`

**Create new file:**

```rust
//! Create chat command

use rhd_chat_client::ChatClient;
use rhd_chat_api::CreateChatParams;

pub async fn execute(title: String) -> Result<(), Box<dyn std::error::Error>> {
    let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
    let result = client.create_chat(CreateChatParams {
        title,
        tags: vec![],
    }).await?;
    
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
```

### 9. `packages/rhd_app/src/commands/plugins.rs`

**Create new file:**

```rust
//! Plugin operations command

use rhd_chat_client::ChatClient;
use rhd_chat_api::{GetPluginsParams, RemovePluginParams};
use crate::cli::PluginsSubcommand;

pub async fn execute(subcommand: PluginsSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
    
    match subcommand {
        PluginsSubcommand::List => {
            let result = client.get_plugins(GetPluginsParams {}).await?;
            println!("{}", serde_json::to_string_pretty(&result.plugins)?);
            Ok(())
        }
        PluginsSubcommand::Remove { plugin_id } => {
            client.remove_plugin(RemovePluginParams { plugin_id }).await?;
            println!("Plugin removed successfully");
            Ok(())
        }
    }
}
```

### 10. `Cargo.toml` (workspace root)

**Modifications:**
Add `packages/rhd_app` back to workspace members:

```toml
[workspace]
members = [
    "packages/rhd_util",
    "packages/rhd_ai_client",
    "packages/rhd_mock_ai_provider",
    "packages/rhd_mcp_client",
    "packages/rhd_db",
    "packages/rhd_chat_api",
    "packages/rhd_chat_server",
    "packages/rhd_chat_client",
    "packages/rhd_app",  # ADD THIS LINE
    "plugins/rhd_plugin_ai_completions",
]
```

## Implementation Notes

1. **Server URL:** Currently hardcoded to `ws://127.0.0.1:8080/`. Consider making this configurable via environment variable or CLI flag in the future.

2. **Error handling:** All commands return `Result<(), Box<dyn std::error::Error>>` for simplicity. Consider using custom error types later.

3. **JSON output:** All commands output JSON for easy parsing and scripting.

4. **Connection management:** Each command creates a new connection. For better performance, consider connection pooling or persistent connections in the future.

5. **ChatClient API:** The implementation assumes the following methods exist in `ChatClient`:
   - `list_chats(params)`
   - `get_chat(params)`
   - `get_queue_messages(params)`
   - `add_queue_message(params)`
   - `create_chat(params)`
   - `get_plugins(params)`
   - `remove_plugin(params)`

   Verify these methods exist in `rhd_chat_client` and adjust accordingly.

## Dependencies

- Phase 1 must be complete (legacy packages removed)
- Requires `rhd_chat_client` and `rhd_chat_api` to be available

## Success Criteria

- [ ] New `rhd_app` package created with correct structure
- [ ] All CLI commands implemented
- [ ] CLI compiles successfully
- [ ] CLI can connect to `rhd_chat_server`
- [ ] All commands execute successfully against running server
- [ ] JSON output is correctly formatted
