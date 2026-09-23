//! Chat tag vocabulary for the sub-chat plugin.
//!
//! Single source of truth for every tag this plugin reads or writes. Pure —
//! no client dependencies — so it is unit-testable in isolation.

/// Platform-level pause tag honored by ai_completions (exact match, bare name).
pub const PAUSED_TAG: &str = "paused";

/// Direct-parent link prefix: `parent:<chatId>`.
pub const PARENT_PREFIX: &str = "parent:";
/// Topmost-ancestor lineage prefix: `root:<chatId>`.
pub const ROOT_PREFIX: &str = "root:";
/// Tool-call → subchat link prefix: `sub_chat:call:<toolCallId>`.
pub const CALL_PREFIX: &str = "sub_chat:call:";

/// Prefix reserved for the whole `sub_chat:*` namespace (validation only).
const SUB_CHAT_NAMESPACE_PREFIX: &str = "sub_chat:";

/// Tool-call → subchat link tag (set atomically at createChat).
pub fn call_tag(tool_call_id: &str) -> String {
    format!("{CALL_PREFIX}{tool_call_id}")
}

/// Parses `sub_chat:call:<id>` → Some(id). Malformed and near-miss tags are not ours.
pub fn parse_call_tag(tag: &str) -> Option<&str> {
    let id = tag.strip_prefix(CALL_PREFIX)?;
    if id.is_empty() {
        None // `sub_chat:call:` alone carries no tool-call id — not ours
    } else {
        Some(id)
    }
}

/// Direct-parent tag pointing at `chat_id`.
pub fn parent_tag(chat_id: i64) -> String {
    format!("{PARENT_PREFIX}{chat_id}")
}

/// Root-lineage tag pointing at `chat_id`.
pub fn root_tag(chat_id: i64) -> String {
    format!("{ROOT_PREFIX}{chat_id}")
}

/// The topmost root id declared in a tag set, if any.
///
/// Only well-formed `root:<int>` tags count (malformed → not ours). If several
/// are present (corrupt state), the smallest id wins — chat ids are assigned
/// in creation order, so the earliest ancestor carries the smallest one.
pub fn root_of(tags: &[String]) -> Option<i64> {
    tags.iter().filter_map(|tag| parse_root(tag)).min()
}

fn parse_root(tag: &str) -> Option<i64> {
    tag.strip_prefix(ROOT_PREFIX)?.parse::<i64>().ok()
}

/// True if the tagged chat is a direct child of `parent_chat_id`
/// (carries exactly `parent:<parent_chat_id>`).
pub fn is_direct_child(tags: &[String], parent_chat_id: i64) -> bool {
    let expected = parent_tag(parent_chat_id);
    tags.iter().any(|tag| tag == &expected)
}

/// True if the chat carries the bare `paused` tag (exact match).
pub fn has_paused_tag(tags: &[String]) -> bool {
    tags.iter().any(|tag| tag == PAUSED_TAG)
}

/// Validate user-supplied spawn tags: reject reserved shapes —
/// any of `parent:` / `root:` / `sub_chat:` prefixes, exact `paused`,
/// empty string, or embedded whitespace. Returns a joined error message.
pub fn validate_user_tags(tags: &[String]) -> Result<(), String> {
    let mut errors = Vec::new();

    for tag in tags {
        if tag.is_empty() {
            errors.push("empty tag".to_string());
        } else if tag == PAUSED_TAG {
            errors.push(format!("tag '{tag}' is reserved"));
        } else if tag.starts_with(PARENT_PREFIX)
            || tag.starts_with(ROOT_PREFIX)
            || tag.starts_with(SUB_CHAT_NAMESPACE_PREFIX)
        {
            errors.push(format!("tag '{tag}' uses a reserved prefix"));
        } else if tag.chars().any(char::is_whitespace) {
            errors.push(format!("tag '{tag}' contains whitespace"));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("invalid tags: {}", errors.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn call_tag_builder_parser_roundtrip() {
        let tag = call_tag("call_abc123");
        assert_eq!(tag, "sub_chat:call:call_abc123");
        assert_eq!(parse_call_tag(&tag), Some("call_abc123"));
    }

    #[test]
    fn parent_and_root_tag_builders_roundtrip() {
        assert_eq!(parent_tag(42), "parent:42");
        assert_eq!(root_tag(42), "root:42");
        assert_eq!(root_of(&tags(&[&root_tag(42)])), Some(42));
    }

    #[test]
    fn parse_call_tag_rejects_near_misses_and_foreign_tags() {
        assert_eq!(parse_call_tag("sub_chat:call_"), None);
        assert_eq!(parse_call_tag("sub_chat:call_x"), None);
        assert_eq!(parse_call_tag("sub_chat:call:"), None);
        assert_eq!(parse_call_tag("parent:5"), None);
        assert_eq!(parse_call_tag("paused"), None);
    }

    #[test]
    fn root_of_returns_declared_root_and_ignores_malformed() {
        assert_eq!(root_of(&tags(&["root:7", "parent:3", "paused"])), Some(7));
        // Non-numeric `root:` is not ours.
        assert_eq!(root_of(&tags(&["root:abc"])), None);
        assert_eq!(root_of(&tags(&["root:", "root:1x", "parent:9"])), None);
        assert_eq!(root_of(&tags(&[])), None);
    }

    #[test]
    fn is_direct_child_matches_exact_id() {
        // A child of chat 12 is not a child of chat 1 (no prefix confusion).
        let child = tags(&[&parent_tag(12), &root_tag(12)]);
        assert!(is_direct_child(&child, 12));
        assert!(!is_direct_child(&child, 1));
        assert!(!is_direct_child(&tags(&[&parent_tag(1)]), 12));
        assert!(!is_direct_child(&tags(&["paused"]), 12));
    }

    #[test]
    fn has_paused_tag_is_exact_match() {
        assert!(has_paused_tag(&tags(&["paused"])));
        assert!(has_paused_tag(&tags(&["worktree:wt1", "paused"])));
        assert!(!has_paused_tag(&tags(&["paused ", "unpaused", "pause"])));
        assert!(!has_paused_tag(&tags(&[])));
    }

    #[test]
    fn validate_user_tags_rejects_reserved_shapes() {
        for bad in ["paused", "parent:5", "root:9", "sub_chat:call:x", "", "has space"] {
            let result = validate_user_tags(&tags(&[bad]));
            assert!(result.is_err(), "expected rejection of {bad:?}, got {result:?}");
        }
    }

    #[test]
    fn validate_user_tags_accepts_ordinary_tags() {
        assert!(validate_user_tags(&tags(&["systemPrompt:plan"])).is_ok());
        assert!(validate_user_tags(&tags(&["worktree:wt1"])).is_ok());
        assert!(validate_user_tags(&tags(&["mcp:common"])).is_ok());
        assert!(validate_user_tags(&tags(&["systemPrompt:plan", "worktree:wt1"])).is_ok());
        assert!(validate_user_tags(&tags(&[])).is_ok());
    }

    #[test]
    fn validate_user_tags_error_lists_all_offenders() {
        let err = validate_user_tags(&tags(&["paused", "parent:5", "mcp:common"])).unwrap_err();
        assert!(err.contains("paused"), "message mentions first offender: {err}");
        assert!(err.contains("parent:5"), "message mentions second offender: {err}");
        assert!(!err.contains("mcp:common"), "valid tag not mentioned: {err}");
    }
}
