//! Chat eligibility predicates (AD-2): worktree gate AND per-server tag gate.

use crate::config::ResolvedServer;

/// Prefix identifying worktree tags on chats: `worktree:<workTreeId>`.
pub const WORKTREE_TAG_PREFIX: &str = "worktree:";

/// True if the chat carries any `worktree:*` tag.
pub fn has_worktree_tag(chat_tags: &[String]) -> bool {
    chat_tags.iter().any(|t| t.starts_with(WORKTREE_TAG_PREFIX))
}

/// Worktree gate:
/// - `Some(w)`: chat must carry the exact tag `worktree:{w}`.
/// - `None`: chat must carry no `worktree:*` tag at all.
pub fn worktree_gate(chat_tags: &[String], worktree: Option<&str>) -> bool {
    match worktree {
        Some(w) => {
            let required = format!("{}{}", WORKTREE_TAG_PREFIX, w);
            chat_tags.iter().any(|t| t == &required)
        }
        None => !has_worktree_tag(chat_tags),
    }
}

/// Per-server gate: a server with `registerOnTag: T` is eligible only for
/// chats carrying the exact tag `T`; servers without the field are always eligible.
pub fn server_gate(chat_tags: &[String], register_on_tag: Option<&str>) -> bool {
    match register_on_tag {
        Some(tag) => chat_tags.iter().any(|t| t == tag),
        None => true,
    }
}

/// Ids of all servers eligible for a chat (AND of both gates).
pub fn eligible_server_ids(
    servers: &[&ResolvedServer],
    chat_tags: &[String],
    worktree: Option<&str>,
) -> Vec<String> {
    if !worktree_gate(chat_tags, worktree) {
        return Vec::new();
    }
    servers
        .iter()
        .filter(|s| server_gate(chat_tags, s.register_on_tag.as_deref()))
        .map(|s| s.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedServer;
    use std::collections::HashMap;

    fn server(id: &str, register_on_tag: Option<&str>) -> ResolvedServer {
        ResolvedServer {
            id: id.to_string(),
            name: id.to_string(),
            cmd: "x".to_string(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            register_on_tag: register_on_tag.map(|s| s.to_string()),
        }
    }

    fn tags(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn worktree_gate_without_flag_rejects_any_worktree_tag() {
        assert!(worktree_gate(&tags(&[]), None));
        assert!(worktree_gate(&tags(&["mcp:common"]), None));
        assert!(!worktree_gate(&tags(&["worktree:W1"]), None));
    }

    #[test]
    fn worktree_gate_with_flag_requires_exact_tag() {
        assert!(worktree_gate(&tags(&["worktree:W1"]), Some("W1")));
        assert!(!worktree_gate(&tags(&["worktree:W2"]), Some("W1")));
        assert!(!worktree_gate(&tags(&[]), Some("W1")));
        assert!(!worktree_gate(&tags(&["worktree:W1x"]), Some("W1")));
    }

    #[test]
    fn server_gate_exact_tag() {
        assert!(server_gate(&tags(&["mcp:common"]), Some("mcp:common")));
        assert!(!server_gate(&tags(&["mcp:other"]), Some("mcp:common")));
        assert!(!server_gate(&tags(&[]), Some("mcp:common")));
        assert!(server_gate(&tags(&[]), None));
    }

    #[test]
    fn eligible_ids_compose_both_gates() {
        let plain = server("plain", None);
        let tagged = server("tagged", Some("mcp:common"));
        let servers = vec![&plain, &tagged];

        // No worktree filter: plain eligible everywhere except worktree-tagged chats;
        // tagged only with its tag.
        assert_eq!(
            eligible_server_ids(&servers, &tags(&[]), None),
            vec!["plain"]
        );
        assert_eq!(
            eligible_server_ids(&servers, &tags(&["mcp:common"]), None),
            vec!["plain", "tagged"]
        );
        assert_eq!(
            eligible_server_ids(&servers, &tags(&["worktree:W1"]), None),
            Vec::<String>::new()
        );

        // With worktree filter: only exact worktree tag passes, and then per-server gate applies.
        assert_eq!(
            eligible_server_ids(&servers, &tags(&["worktree:W1"]), Some("W1")),
            vec!["plain"]
        );
        assert_eq!(
            eligible_server_ids(
                &servers,
                &tags(&["worktree:W1", "mcp:common"]),
                Some("W1")
            ),
            vec!["plain", "tagged"]
        );
        assert_eq!(
            eligible_server_ids(&servers, &tags(&["worktree:W2"]), Some("W1")),
            Vec::<String>::new()
        );
    }
}
