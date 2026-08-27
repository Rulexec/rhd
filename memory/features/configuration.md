# Configuration

## Purpose
System configuration via YAML files and CLI arguments. The system uses three separate configurations for different components.

## Configuration Files

### `rhd start` Config
Used by the `rhd start` command to spawn child processes. Defines which services to start and how.

```yaml
children:
  - name: chat-server          # Display name for log prefix
    cmd: cargo                 # Command to run
    cwd: .                     # Optional: working directory (relative to config file)
    args:                      # Optional: command arguments
      - run
      - --bin
      - rhd_chat_server
      - --
      - --port
      - "8080"
  
  - name: ai-plugin
    cmd: cargo
    args:
      - run
      - --bin
      - rhd_plugin_ai_completions
      - --
      - --config
      - plugin-config.yaml
```

### `rhd_chat_server` Config
The chat server uses CLI arguments (no config file):

```
--host <HOST>          # Host to bind to (default: 127.0.0.1)
--port <PORT>          # Port to listen on (default: 8080)
--db-path <PATH>       # Path to folder for SQLite database (default: ./rhd_db)
```

The server creates `chats.db` inside the specified `db-path` directory.

### `rhd_plugin_ai_completions` Config
The AI completions plugin uses a YAML config file:

```yaml
credentialsConfig: credentials.yaml    # Path to credentials file (relative to this config)

ai_completions:
  models:
    default:                           # Required: default model configuration
      alias: gpt4                      # Optional: alias to another model
      baseUrl: https://api.openai.com/v1
      apiKey:
        cred: openaiApiKey             # Reference to credentials file
      model: gpt-4
    
    gpt4:                              # Additional model definitions
      baseUrl: https://api.openai.com/v1
      apiKey:
        cred: openaiApiKey
      model: gpt-4
```

**Model Configuration:**
- `default` model is required and must exist in the models map
- Models can use `alias` to reference another model
- `apiKey.cred` references a key in the credentials file
- `baseUrl` and `model` are optional if using an alias

### `credentials.yaml` — Secrets
Separates API keys from shareable config files. Used by the AI completions plugin.

```yaml
openaiApiKey: "sk-..."
anthropicApiKey: "sk-ant-..."
```

Referenced from plugin config via `credentialsConfig: credentials.yaml` (path relative to plugin config file).

## CLI Commands

The `rhd` CLI provides command-line access to the chat server:

### `rhd start <config_path>`
Starts the chat server and all configured child processes from a YAML config file.

### `rhd chats list`
Lists all chats in JSON format.

### `rhd messages <chat_id> [--all]`
View chat messages. Default shows only the last message; `--all` shows all messages.

### `rhd queue list <chat_id>`
Shows queued messages for a chat.

### `rhd queue add <chat_id> <content>`
Adds a message to the chat's processing queue.

### `rhd create-chat <title>`
Creates a new chat with the specified title.

### `rhd plugins list`
Lists all registered plugins.

### `rhd plugins remove <plugin_id>`
Removes a plugin by ID.

## Startup Validation

### Chat Server
- Validates database path is accessible
- Creates `chats.db` if it doesn't exist

### AI Completions Plugin
- Validates `default` model exists in configuration
- Resolves model aliases at startup
- Validates credentials file exists and is readable
- Validates all referenced credentials exist in the credentials file
