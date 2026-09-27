//! Frontier-based chat classification for logged completions requests.
//!
//! Chats are identified purely from request content: the prefix-hash chain of the
//! message history (see [`crate::logging::chat_match`]). Each chat owns the hashes it
//! registered, single-owner and never re-pointed, and its **frontier** is the longest
//! history it has registered. A request is classified against the owner of its longest
//! matching prefix hash:
//!
//! - **Retry** — the full-history hash is already registered: the request resends a
//!   known history and belongs to that chat.
//! - **Continuation** — the longest matching hash is the owner's frontier: the history
//!   strictly extends that chat, which claims the new hashes.
//! - **Branch** — the longest matching hash predates the owner's frontier: the history
//!   forks from an earlier point (sub-chat subset, edited/regenerated resend) and gets
//!   a new independent chat row; the shared hashes stay with their first owner.
//! - **New chat** — no prefix is known at all.
//!
//! Known content-only limit: two clients extending byte-identical histories are
//! indistinguishable — the first extension claims the chat, the later diverger becomes
//! a branch.

use rusqlite::{params, Connection, OptionalExtension};

/// Result of classifying (and registering) one request's history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    /// History identical to one already registered for this chat.
    Retry { chat_id: i64 },
    /// History strictly extends this chat's frontier.
    Continuation { chat_id: i64 },
    /// History forks from a non-frontier point of `parent_chat_id`; a new chat row
    /// was created for it (no lineage is stored on the row itself).
    Branch {
        parent_chat_id: i64,
        chat_id: i64,
    },
    /// No prefix hash was known; a new chat row was created.
    NewChat { chat_id: i64 },
}

impl Classification {
    /// The chat the request is attributed to.
    pub fn chat_id(&self) -> i64 {
        match *self {
            Classification::Retry { chat_id }
            | Classification::Continuation { chat_id }
            | Classification::Branch { chat_id, .. }
            | Classification::NewChat { chat_id } => chat_id,
        }
    }
}

/// Classifies `hashes` and registers the outcome, creating chat rows for branches and
/// new chats. Must run inside the caller's transaction so classification, registration,
/// and request recording commit atomically.
///
/// `hashes` is the full prefix-hash chain of the request's history (element `i` covers
/// messages `0..=i`). `title`/`model` seed newly created chat rows only.
pub fn classify_and_register(
    conn: &Connection,
    hashes: &[[u8; 32]],
    title: &str,
    model: Option<&str>,
) -> Result<Classification, rusqlite::Error> {
    let Some(last) = hashes.last() else {
        // Callers never classify empty histories; a defensive retry of "no chat".
        return Ok(Classification::NewChat {
            chat_id: create_chat(conn, title, model)?,
        });
    };

    // 1. Exact retry: the full-history hash is already registered.
    if let Some(chat_id) = hash_owner(conn, last)? {
        return Ok(Classification::Retry { chat_id });
    }

    // 2. Longest matching prefix hash, probing longest-first.
    let mut matched: Option<(usize, i64)> = None;
    for (index, hash) in hashes.iter().enumerate().rev() {
        if let Some(chat_id) = hash_owner(conn, hash)? {
            matched = Some((index, chat_id));
            break;
        }
    }

    let Some((index, owner_chat_id)) = matched else {
        // 3. Nothing known: a brand-new chat registers the full chain.
        let chat_id = create_chat(conn, title, model)?;
        insert_hashes(conn, chat_id, hashes, 0)?;
        return Ok(Classification::NewChat { chat_id });
    };

    // 4. The matched hash either sits on the owner's frontier (continuation) or at an
    //    earlier point (branch). Either way, hashes beyond the match are unregistered —
    //    the longest-match probe guarantees it — so plain inserts cannot conflict.
    let frontier = frontier_index(conn, owner_chat_id)?;
    let chat_id = if index == frontier {
        owner_chat_id
    } else {
        let chat_id = create_chat(conn, title, model)?;
        insert_hashes(conn, chat_id, hashes, index + 1)?;
        return Ok(Classification::Branch {
            parent_chat_id: owner_chat_id,
            chat_id,
        });
    };
    insert_hashes(conn, chat_id, hashes, index + 1)?;
    Ok(Classification::Continuation { chat_id })
}

/// The chat owning `hash`, if registered.
fn hash_owner(conn: &Connection, hash: &[u8]) -> Result<Option<i64>, rusqlite::Error> {
    let mut stmt = conn.prepare_cached("SELECT chat_id FROM prefix_hashes WHERE hash = ?1")?;
    stmt.query_row(params![hash], |row| row.get::<_, i64>(0)).optional()
}

/// Index of the chat's frontier: the message count of its longest registered history,
/// minus one. Chats always register at least one hash at creation, so this cannot be
/// empty for a chat found via a hash probe.
fn frontier_index(conn: &Connection, chat_id: i64) -> Result<usize, rusqlite::Error> {
    let mut stmt =
        conn.prepare_cached("SELECT COALESCE(MAX(len), 0) FROM prefix_hashes WHERE chat_id = ?1")?;
    let max_len: i64 = stmt.query_row(params![chat_id], |row| row.get(0))?;
    Ok(max_len.saturating_sub(1).max(0) as usize)
}

/// Inserts a chat row and returns its id.
fn create_chat(conn: &Connection, title: &str, model: Option<&str>) -> Result<i64, rusqlite::Error> {
    let now = crate::logging::db::now_rfc3339();
    conn.execute(
        "INSERT INTO chats (title, model, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![title, model, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Registers `hashes[from..]` to `chat_id`, recording each hash's covered message count.
fn insert_hashes(
    conn: &Connection,
    chat_id: i64,
    hashes: &[[u8; 32]],
    from: usize,
) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare_cached(
        "INSERT INTO prefix_hashes (hash, chat_id, len) VALUES (?1, ?2, ?3)",
    )?;
    for (index, hash) in hashes.iter().enumerate().skip(from) {
        stmt.execute(params![hash.as_slice(), chat_id, (index + 1) as i64])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::chat_match::prefix_hashes;
    use serde_json::json;
    use std::path::PathBuf;

    fn temp_db(name: &str) -> LoggingDbGuard {
        let dir = std::env::temp_dir().join(format!(
            "rhd_ai_proxy_classify_test_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        LoggingDbGuard {
            db: crate::logging::db::LoggingDb::open(&dir).unwrap(),
            dir,
        }
    }

    /// Cleans up the temp directory on drop.
    struct LoggingDbGuard {
        db: crate::logging::db::LoggingDb,
        dir: PathBuf,
    }

    impl Drop for LoggingDbGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn messages(turns: &[&str]) -> Vec<serde_json::Value> {
        let mut messages = vec![json!({"role": "system", "content": "sys"})];
        for turn in turns {
            messages.push(json!({"role": "user", "content": turn}));
        }
        messages
    }

    /// Runs `classify_and_register` through `LoggingDb`'s connection.
    fn classify(
        guard: &LoggingDbGuard,
        history: &[serde_json::Value],
    ) -> Classification {
        let hashes = prefix_hashes(history);
        let conn = guard.db.conn_for_tests();
        let tx = conn.unchecked_transaction().unwrap();
        let result = classify_and_register(&tx, &hashes, "title", None).unwrap();
        tx.commit().unwrap();
        result
    }

    #[test]
    fn first_history_creates_new_chat_with_full_chain() {
        let guard = temp_db("new");
        let result = classify(&guard, &messages(&["hello"]));
        assert!(matches!(result, Classification::NewChat { .. }));
    }

    #[test]
    fn appending_history_is_a_continuation() {
        let guard = temp_db("continuation");
        let first = messages(&["first"]);
        assert!(matches!(
            classify(&guard, &first),
            Classification::NewChat { .. }
        ));
        let mut extended = first.clone();
        extended.push(json!({"role": "assistant", "content": "reply"}));
        extended.push(json!({"role": "user", "content": "second"}));
        match classify(&guard, &extended) {
            Classification::Continuation { chat_id } => {
                // The chat's frontier advanced: a further extension still continues it.
                let mut more = extended.clone();
                more.push(json!({"role": "user", "content": "third"}));
                assert_eq!(classify(&guard, &more).chat_id(), chat_id);
            }
            other => panic!("expected continuation, got {other:?}"),
        }
    }

    #[test]
    fn identical_resend_is_a_retry_attributed_to_same_chat() {
        let guard = temp_db("retry");
        let history = messages(&["hello"]);
        let Classification::NewChat { chat_id } = classify(&guard, &history) else {
            panic!("expected new chat");
        };
        // Retry of the current frontier...
        assert_eq!(
            classify(&guard, &history),
            Classification::Retry { chat_id }
        );
        // ...and of an older point after the chat advanced.
        let mut extended = history.clone();
        extended.push(json!({"role": "user", "content": "more"}));
        assert!(matches!(
            classify(&guard, &extended),
            Classification::Continuation { .. }
        ));
        assert_eq!(
            classify(&guard, &history),
            Classification::Retry { chat_id }
        );
    }

    #[test]
    fn subset_extending_an_old_point_branches() {
        let guard = temp_db("subset");
        let parent = messages(&["first"]);
        let Classification::NewChat { chat_id: parent_id } = classify(&guard, &parent) else {
            panic!("expected new chat");
        };
        let mut advanced = parent.clone();
        advanced.push(json!({"role": "assistant", "content": "parent reply"}));
        advanced.push(json!({"role": "user", "content": "next"}));
        assert!(matches!(
            classify(&guard, &advanced),
            Classification::Continuation { .. }
        ));

        // A sub-chat spawned with the parent's seed messages: its first request shares
        // the old frontier exactly (a retry), but its continuation extends a
        // non-frontier point and must become its own chat.
        assert_eq!(
            classify(&guard, &parent),
            Classification::Retry { chat_id: parent_id }
        );
        let mut sub = parent.clone();
        sub.push(json!({"role": "user", "content": "sub task"}));
        match classify(&guard, &sub) {
            Classification::Branch {
                parent_chat_id,
                chat_id,
            } => {
                assert_eq!(parent_chat_id, parent_id);
                assert_ne!(chat_id, parent_id);
                // The branch owns its own frontier from here on.
                let mut sub_more = sub.clone();
                sub_more.push(json!({"role": "user", "content": "sub next"}));
                assert_eq!(classify(&guard, &sub_more).chat_id(), chat_id);
                // And the parent still continues its own line.
                let mut parent_more = advanced.clone();
                parent_more.push(json!({"role": "user", "content": "parent next"}));
                assert_eq!(classify(&guard, &parent_more).chat_id(), parent_id);
            }
            other => panic!("expected branch, got {other:?}"),
        }
    }

    #[test]
    fn same_length_divergence_branches() {
        let guard = temp_db("divergence");
        let original = messages(&["first"]);
        let Classification::NewChat { chat_id: original_id } = classify(&guard, &original)
        else {
            panic!("expected new chat");
        };
        let mut advanced = original.clone();
        advanced.push(json!({"role": "user", "content": "second"}));
        assert!(matches!(
            classify(&guard, &advanced),
            Classification::Continuation { .. }
        ));

        // An edited resend diverges at the second user message: new chat, same length.
        let edited = messages(&["edited"]);
        match classify(&guard, &edited) {
            Classification::Branch { parent_chat_id, .. } => {
                assert_eq!(parent_chat_id, original_id);
            }
            other => panic!("expected branch, got {other:?}"),
        }
    }

    #[test]
    fn chats_sharing_only_the_system_message_stay_separate() {
        let guard = temp_db("system_only");
        let first = messages(&["hello"]);
        let Classification::NewChat { chat_id: first_id } = classify(&guard, &first) else {
            panic!("expected new chat");
        };
        let second = messages(&["unrelated"]);
        match classify(&guard, &second) {
            // The only shared prefix is the system message, which predates the first
            // chat's frontier once it advanced — so this is a branch, not a merge.
            Classification::Branch { chat_id, .. } => assert_ne!(chat_id, first_id),
            other => panic!("expected branch, got {other:?}"),
        }
    }
}
