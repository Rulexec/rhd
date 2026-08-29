# Plan: Add Tag Management Commands to rhd_app CLI

## Overview

Add CLI commands to manage chat tags in the `rhd_app` utility. The backend already supports tag operations via the WebSocket API (`updateChat` method with `addTags`/`removeTags` fields), but the CLI currently lacks commands to expose this functionality.

## Current State

### Existing CLI Commands
- `rhd chats list` - List all chats
- `rhd create-chat <title>` - Create a new chat (no tags option)
- `rhd messages <chat_id>` - View chat messages

### Backend API Support
- [`UpdateChatParams`](packages/rhd_chat_api/src/methods/update_chat.rs:19) has:
  - `add_tags: Vec<String>` - Tags to add
  - `remove_tags: Vec<String>` - Tags to remove
- [`CreateChatParams`](packages/rhd_chat_api/src/methods/create_chat.rs:18) has:
  - `tags: Vec<String>` - Initial tags for new chat
- [`ChatClient::update_chat()`](packages/rhd_chat_client/src/client.rs:580) method exists

## Proposed Changes

### 1. Extend `ChatsSubcommand` Enum

**File**: `packages/rhd_app/src/cli.rs`

Add new subcommands to the `ChatsSubcommand` enum:

```rust
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
```

### 2. Add `--tags` Option to `CreateChat`

**File**: `packages/rhd_app/src/cli.rs`

Modify the `CreateChat` command to accept optional tags:

```rust
/// Create a new chat
CreateChat {
    /// Chat title
    title: String,
    /// Optional tags (space-separated)
    #[arg(long, num_args = 0..)]
    tags: Option<Vec<String>>,
},
```

### 3. Implement Command Handlers

**File**: `packages/rhd_app/src/commands/chats.rs`

Add match arms for the new subcommands:

```rust
pub async fn execute(subcommand: ChatsSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    match subcommand {
        ChatsSubcommand::List => {
            // existing implementation
        }
        ChatsSubcommand::AddTag { chat_id, tags } => {
            let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
            let result = client.update_chat(UpdateChatParams {
                chat_id,
                title: None,
                add_tags: tags,
                remove_tags: vec![],
            }).await?;
            println!("Tags added successfully");
            Ok(())
        }
        ChatsSubcommand::RemoveTag { chat_id, tags } => {
            let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
            let result = client.update_chat(UpdateChatParams {
                chat_id,
                title: None,
                add_tags: vec![],
                remove_tags: tags,
            }).await?;
            println!("Tags removed successfully");
            Ok(())
        }
    }
}
```

**File**: `packages/rhd_app/src/commands/create_chat.rs`

Modify to accept optional tags:

```rust
pub async fn execute(title: String, tags: Option<Vec<String>>) -> Result<(), Box<dyn std::error::Error>> {
    let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
    let result = client.create_chat(CreateChatParams {
        title,
        tags: tags.unwrap_or_default(),
    }).await?;
    
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
```

### 4. Update Main Command Router

**File**: `packages/rhd_app/src/main.rs`

Update the `CreateChat` command handler to pass the tags parameter:

```rust
Command::CreateChat { title, tags } => {
    commands::create_chat::execute(title, tags).await?;
}
```

## Usage Examples

After implementation, users will be able to:

```bash
# Create a chat with initial tags
rhd create-chat "My Chat" --tags tag1 tag2 systemPrompt:warhammer

# Add tags to an existing chat
rhd chats add-tag 123 systemPrompt:warhammer important

# Remove tags from a chat
rhd chats remove-tag 123 old-tag deprecated

# List chats (existing command, now shows tags)
rhd chats list
```

## Implementation Steps

1. **Update CLI definitions** (`cli.rs`)
   - Add `AddTag` and `RemoveTag` variants to `ChatsSubcommand`
   - Add `tags` field to `CreateChat` command

2. **Implement tag commands** (`commands/chats.rs`)
   - Add `AddTag` handler using `update_chat()` with `add_tags`
   - Add `RemoveTag` handler using `update_chat()` with `remove_tags`

3. **Update create-chat command** (`commands/create_chat.rs`)
   - Accept optional `tags` parameter
   - Pass tags to `CreateChatParams`

4. **Update main router** (`main.rs`)
   - Pass `tags` parameter to `create_chat::execute()`

5. **Test the implementation**
   - Build the project: `cargo build`
   - Test creating a chat with tags
   - Test adding tags to existing chat
   - Test removing tags from chat
   - Verify tags appear in `rhd chats list` output

## Files to Modify

| File | Changes |
|------|---------|
| `packages/rhd_app/src/cli.rs` | Add `AddTag`, `RemoveTag` subcommands; add `tags` to `CreateChat` |
| `packages/rhd_app/src/commands/chats.rs` | Implement `AddTag` and `RemoveTag` handlers |
| `packages/rhd_app/src/commands/create_chat.rs` | Accept and pass `tags` parameter |
| `packages/rhd_app/src/main.rs` | Update `CreateChat` handler to pass `tags` |

## Success Criteria

- [ ] `rhd create-chat <title> --tags <tag1> <tag2>` creates chat with tags
- [ ] `rhd chats add-tag <chat_id> <tag1> <tag2>` adds tags to existing chat
- [ ] `rhd chats remove-tag <chat_id> <tag1> <tag2>` removes tags from chat
- [ ] All commands return appropriate success/error messages
- [ ] Code compiles without warnings
- [ ] Existing functionality remains unchanged

## Dependencies

- No new dependencies required
- Uses existing `UpdateChatParams` and `CreateChatParams` from `rhd_chat_api`
- Uses existing `ChatClient::update_chat()` method

## Risk Assessment

**Low Risk**: This is a straightforward addition of CLI commands that wrap existing backend API functionality. The backend already supports these operations, so we're just exposing them through the CLI interface.

**Potential Issues**:
- Tag validation: The backend handles tag validation, so invalid tags will return errors
- Concurrent modifications: The backend uses additive/subtractive arrays to avoid race conditions
- User error: Users might try to add/remove non-existent tags (backend handles gracefully)

## Future Enhancements (Out of Scope)

- `rhd chats set-tags <chat_id> <tag1> <tag2>` - Replace all tags (not just add/remove)
- `rhd chats list --tag <tag>` - Filter chats by tag
- Tag completion/suggestions in shell
- Bulk tag operations on multiple chats
