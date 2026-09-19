# Configuration

Implementation view of configuration handling. For the product-view of config files see [features/configuration.md](features/configuration.md).

There is no global `rhd.yaml` daemon config anymore — each component takes its own CLI args and/or YAML file.

## `rhd start` — Process Supervisor Config

`packages/rhd_app/src/commands/start.rs`: `StartConfig { children: Vec<ChildConfig> }`, `ChildConfig { name, cmd, cwd?, args? }` (serde_yaml). Relative `cwd` resolves against the config file's directory. Child stdout/stderr are forwarded with `[name]` prefixes.

## Chat Server

`packages/rhd_chat_server/src/config.rs`: clap `Config { host, port, db_path, clear_pending_acks }`. Defaults: `127.0.0.1:8080`, `./rhd_db` (creates `chats.db` inside). `--clear-pending-acks` deletes pending custom-event acknowledgments before startup (recovery utility).

## AI Completions Plugin

`plugins/rhd_plugin_ai_completions/src/config.rs`:

- `PluginConfig { credentialsConfig: String, ai_completions: AiCompletionsConfig }`
- `AiCompletionsConfig { models: HashMap<String, ModelConfig> }`
- `ModelConfig { alias?, baseUrl?, apiKey?: ApiKeyConfig { cred }, model?, sendReasoningContent? (default true) }`
- `load_config(path)`: parses YAML, resolves `credentialsConfig` relative to the config file, validates a `default` model exists, and validates alias targets resolve. `ConfigError` variants: MainConfigFileRead, CredentialsFileRead, Parse, Validation.
- `load_credentials(path)` + `resolve_api_key(config, cred_name)`: credentials file is a flat `HashMap<String, String>` (name → key); missing credential is a validation error.

## System Prompt Plugin

`plugins/rhd_plugin_system_prompt/src/config.rs`: `PluginConfig { systemPrompts: map name → file path }`. Relative paths resolve against the config file's directory; prompts are cached at startup (`CachedPrompt`), missing files fail startup.

## Commands Plugin

`plugins/rhd_plugin_commands/src/config.rs`: `PluginConfig { commands: map name → CommandDef }` with `deny_unknown_fields`. serde untagged model (variant order is load-bearing): `CommandDef = Multi(Vec<CommandStep>) | Single(CommandStep)`, `CommandStep = PromptPath(String) | Spec(CommandSpec)` (a bare string is prompt shorthand, role `user`). `CommandSpec` is internally tagged by `type` — `chat_tags {add,remove}` | `message_tags {add,remove}` | `prompt {role (default user), prompt}` — with `deny_unknown_fields`. `load_config(path) → Result<CommandRegistry, ConfigError>` validates names `^[A-Za-z0-9_]+$`, resolves prompt paths against the config file's directory (absolute paths kept verbatim), reads+caches each prompt file (fail fast on missing), restricts prompt roles to user/system/assistant (`tool` rejected), and errors on tag steps with empty add+remove or empty tag strings. `ConfigError`: MainConfigFileRead, Parse, InvalidCommandName, PromptFileRead, InvalidPromptRole, EmptyTags, EmptyTag.

## MCP Plugin

`plugins/rhd_plugin_mcp/src/config.rs`: `PluginConfig { servers: Vec<ResolvedServer> }`; entries (`McpServerEntry`) carry `id?`, `name`, `cmd`, `args`, `cwd?`, `env`, `registerOnTag?`. Strict parsing via `deny_unknown_fields`.

## AI Proxy

`packages/rhd_ai_proxy/src/config.rs`: YAML config with `Proxy`, `Target`, `EnvApiKey`, `ModelConfig` structs; strict parsing via `deny_unknown_fields`.

## CLI (`rhd_app`)

`packages/rhd_app/src/cli.rs`: clap subcommands for chat ops, `messages`, `queue`, `create-chat`, plugins, tag management, and `start <config>`. Connects to `ws://127.0.0.1:8080/` via `rhd_chat_client`. Outputs JSON.
