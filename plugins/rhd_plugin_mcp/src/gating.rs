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
