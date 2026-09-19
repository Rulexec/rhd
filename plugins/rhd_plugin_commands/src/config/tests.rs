//! Unit tests for the commands plugin configuration model.

use super::*;

/// Create `<dir>/commands/prompt.md`, `<dir>/commands/another_prompt.md`
/// and `<dir>/systemPrompts/warhammer.md` with distinguishable content.
fn write_canonical_prompt_files(dir: &Path) {
    std::fs::create_dir_all(dir.join("commands")).unwrap();
    std::fs::create_dir_all(dir.join("systemPrompts")).unwrap();
    std::fs::write(dir.join("commands/prompt.md"), "# Prompt one\n").unwrap();
    std::fs::write(dir.join("commands/another_prompt.md"), "# Prompt two\n").unwrap();
    std::fs::write(dir.join("systemPrompts/warhammer.md"), "For the Emperor!\n").unwrap();
}

/// Write `config.yaml` into a fresh temp dir and return both handles.
fn temp_config(yaml: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, yaml).unwrap();
    let path_str = path.to_str().unwrap().to_string();
    (dir, path_str)
}

fn s(v: &str) -> String {
    v.to_string()
}

#[test]
fn test_load_config_valid_canonical_example() {
    // The four config shapes from the plan, resolved against the caches.
    let (dir, path) = temp_config(
        r#"
commands:
  tags_example:
    type: chat_tags
    add: [mcp:common]
    remove: [pause]
  prompt_example: ./commands/prompt.md
  system_prompt_example:
    type: prompt
    role: system
    prompt: ./systemPrompts/warhammer.md
  multi_example:
    - type: message_tags
      add: [some_tag]
    - ./commands/prompt.md
    - ./commands/another_prompt.md
"#,
    );
    write_canonical_prompt_files(dir.path());

    let registry = load_config(&path).unwrap();

    assert_eq!(registry.len(), 4);
    assert!(!registry.is_empty());
    assert_eq!(
        registry.names(),
        HashSet::from([
            s("tags_example"),
            s("prompt_example"),
            s("system_prompt_example"),
            s("multi_example"),
        ])
    );

    assert_eq!(
        registry.steps("tags_example"),
        Some(
            &[ResolvedStep::ChatTags {
                add: vec![s("mcp:common")],
                remove: vec![s("pause")],
            }][..]
        )
    );
    assert_eq!(
        registry.steps("prompt_example"),
        Some(
            &[ResolvedStep::Prompt {
                name: s("prompt_example"),
                role: s("user"),
                content: "# Prompt one\n".to_string(),
            }][..]
        )
    );
    assert_eq!(
        registry.steps("system_prompt_example"),
        Some(
            &[ResolvedStep::Prompt {
                name: s("system_prompt_example"),
                role: s("system"),
                content: "For the Emperor!\n".to_string(),
            }][..]
        )
    );
    assert_eq!(
        registry.steps("multi_example"),
        Some(
            &[
                ResolvedStep::MessageTags {
                    add: vec![s("some_tag")],
                    remove: vec![],
                },
                ResolvedStep::Prompt {
                    name: s("multi_example"),
                    role: s("user"),
                    content: "# Prompt one\n".to_string(),
                },
                ResolvedStep::Prompt {
                    name: s("multi_example"),
                    role: s("user"),
                    content: "# Prompt two\n".to_string(),
                },
            ][..]
        )
    );
    // Unknown names resolve to nothing.
    assert!(!registry.has("nope"));
    assert_eq!(registry.steps("nope"), None);
}

#[test]
fn test_load_config_unknown_top_level_field() {
    // deny_unknown_fields: stray top-level keys are a Parse error.
    let (_dir, path) = temp_config(
        r#"
commands: {}
unexpected: 1
"#,
    );

    match load_config(&path) {
        Err(e @ ConfigError::Parse(_)) => {
            assert!(e.to_string().contains("unexpected"), "got: {e}");
        }
        other => panic!("expected Parse error, got {other:?}"),
    }
}

#[test]
fn test_load_config_empty_tags() {
    // chat_tags with neither add nor remove is an EmptyTags error.
    let (_dir, path) = temp_config(
        r#"
commands:
  noop:
    type: chat_tags
"#,
    );

    match load_config(&path) {
        Err(ConfigError::EmptyTags { name }) => assert_eq!(name, "noop"),
        other => panic!("expected EmptyTags error, got {other:?}"),
    }
}

#[test]
fn test_load_config_empty_tag_string() {
    // Empty tag strings are rejected.
    let (_dir, path) = temp_config(
        r#"
commands:
  bad:
    type: message_tags
    add: [""]
"#,
    );

    match load_config(&path) {
        Err(ConfigError::EmptyTag { name }) => assert_eq!(name, "bad"),
        other => panic!("expected EmptyTag error, got {other:?}"),
    }
}

#[test]
fn test_load_config_tool_role_rejected() {
    // role: tool requires a toolCallId — rejected at startup.
    let (_dir, path) = temp_config(
        r#"
commands:
  sneaky:
    type: prompt
    role: tool
    prompt: ./commands/prompt.md
"#,
    );

    match load_config(&path) {
        Err(ConfigError::InvalidPromptRole { name, role }) => {
            assert_eq!(name, "sneaky");
            assert_eq!(role, "tool");
        }
        other => panic!("expected InvalidPromptRole error, got {other:?}"),
    }
}

#[test]
fn test_load_config_missing_prompt_file() {
    // Missing prompt files fail fast for all three step shapes,
    // and the error message carries the resolved path.
    let cases = [
        ("shorthand", "commands:\n  c: ./commands/gone.md\n"),
        (
            "tagged",
            "commands:\n  c:\n    type: prompt\n    prompt: ./commands/gone.md\n",
        ),
        (
            "multi",
            "commands:\n  c:\n    - ./commands/prompt.md\n    - ./commands/gone.md\n",
        ),
    ];

    for (shape, yaml) in cases {
        let (dir, path) = temp_config(yaml);
        // Provide prompt.md so the multi case reaches the missing file.
        write_canonical_prompt_files(dir.path());

        match load_config(&path) {
            Err(ConfigError::PromptFileRead {
                name,
                path: resolved,
                ..
            }) => {
                assert_eq!(name, "c", "shape {shape}: wrong command name");
                assert!(
                    resolved.ends_with("commands/gone.md"),
                    "shape {shape}: message must carry the resolved path, got {resolved}"
                );
                assert!(
                    Path::new(&resolved).is_absolute(),
                    "shape {shape}: expected absolute resolved path, got {resolved}"
                );
            }
            other => panic!("shape {shape}: expected PromptFileRead error, got {other:?}"),
        }
    }
}

#[test]
fn test_load_config_invalid_command_name() {
    // Command keys must match ^[A-Za-z0-9_]+$ (checked before I/O).
    let (_dir, path) = temp_config(
        r#"
commands:
  bad-name: ./whatever.md
"#,
    );

    match load_config(&path) {
        Err(ConfigError::InvalidCommandName(name)) => assert_eq!(name, "bad-name"),
        other => panic!("expected InvalidCommandName error, got {other:?}"),
    }
}

#[test]
fn test_load_config_spec_without_type() {
    // Untagged fallthrough: a spec map lacking `type` matches neither
    // PromptPath (not a string) nor a tagged CommandSpec variant -> Parse.
    let cases = [
        ("no type", "commands:\n  broken:\n    add: [some_tag]\n"),
        // deny_unknown_fields on CommandSpec must reject stray keys too,
        // without tripping over the `type` tag itself.
        (
            "stray field",
            "commands:\n  broken:\n    type: chat_tags\n    addx: [some_tag]\n",
        ),
    ];

    for (shape, yaml) in cases {
        let (_dir, path) = temp_config(yaml);

        match load_config(&path) {
            Err(ConfigError::Parse(_)) => {}
            other => panic!("shape {shape}: expected Parse error, got {other:?}"),
        }
    }
}

#[test]
fn test_load_config_paths_resolve_against_config_dir() {
    // A config in a subdir resolves ./prompts/x.md next to itself, and an
    // absolute prompt path is accepted as-is.
    let root = tempfile::TempDir::new().unwrap();
    let conf_dir = root.path().join("nested/conf");
    std::fs::create_dir_all(conf_dir.join("prompts")).unwrap();
    std::fs::write(conf_dir.join("prompts/x.md"), "relative content").unwrap();

    let abs_file = root.path().join("absolute.md");
    std::fs::write(&abs_file, "absolute content").unwrap();

    let yaml = format!(
        "commands:\n  rel: ./prompts/x.md\n  abs: {}\n",
        abs_file.to_str().unwrap()
    );
    let config_path = conf_dir.join("config.yaml");
    std::fs::write(&config_path, yaml).unwrap();

    let registry = load_config(config_path.to_str().unwrap()).unwrap();

    assert_eq!(
        registry.steps("rel"),
        Some(
            &[ResolvedStep::Prompt {
                name: s("rel"),
                role: s("user"),
                content: "relative content".to_string(),
            }][..]
        )
    );
    assert_eq!(
        registry.steps("abs"),
        Some(
            &[ResolvedStep::Prompt {
                name: s("abs"),
                role: s("user"),
                content: "absolute content".to_string(),
            }][..]
        )
    );
}

#[test]
fn test_is_valid_command_name() {
    assert!(is_valid_command_name("a"));
    assert!(is_valid_command_name("Foo_9"));
    assert!(is_valid_command_name("multi_example"));
    assert!(!is_valid_command_name(""));
    assert!(!is_valid_command_name("bad-name"));
    assert!(!is_valid_command_name("bad.name"));
    assert!(!is_valid_command_name("bad name"));
    assert!(!is_valid_command_name("привет"));
}

#[test]
fn test_resolve_path_relative_and_absolute() {
    let base = Path::new("/config/dir");
    assert_eq!(
        resolve_path("./prompts/test.md", base),
        "/config/dir/prompts/test.md"
    );
    assert_eq!(
        resolve_path("prompts/test.md", base),
        "/config/dir/prompts/test.md"
    );
    assert_eq!(resolve_path("/absolute/test.md", base), "/absolute/test.md");
}
