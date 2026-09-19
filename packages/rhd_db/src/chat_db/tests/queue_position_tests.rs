//! Positional queue insertion tests: `position` ordering and `before_message_id`.

use super::super::*;
use super::helpers::cleanup;
use crate::DbError;
use rusqlite::params;

/// Read the internal `position` column for a chat's queue rows.
fn queue_positions(db: &ChatDb, chat_id: i64) -> Vec<(i64, i64)> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT id, position FROM messages_queue WHERE chat_id = ?1 ORDER BY position ASC, id ASC",
        )
        .unwrap();
    stmt.query_map(params![chat_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(|row| row.unwrap())
        .collect()
}

fn queue_ids(db: &ChatDb, chat_id: i64) -> Vec<i64> {
    db.get_queue_messages(chat_id)
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect()
}

fn append(db: &ChatDb, chat_id: i64, content: &str) -> i64 {
    db.add_queue_message(chat_id, None, "user", content, None, None, None)
        .unwrap()
        .0
}

fn insert_before(db: &ChatDb, chat_id: i64, before: i64, content: &str) -> i64 {
    db.add_queue_message(chat_id, Some(before), "user", content, None, None, None)
        .unwrap()
        .0
}

#[test]
fn test_queue_append_order() {
    let path = "test_queue_append_order.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let a = append(&db, chat_id, "A");
    let b = append(&db, chat_id, "B");
    let c = append(&db, chat_id, "C");

    // Reads return insertion order with strictly increasing positions.
    assert_eq!(queue_ids(&db, chat_id), vec![a, b, c]);
    let positions = queue_positions(&db, chat_id);
    assert_eq!(positions, vec![(a, 1), (b, 2), (c, 3)]);
    assert!(positions.windows(2).all(|w| w[0].1 < w[1].1));

    cleanup(path);
}

#[test]
fn test_queue_insert_before_middle() {
    let path = "test_queue_insert_before_middle.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let a = append(&db, chat_id, "A");
    let b = append(&db, chat_id, "B");
    let c = append(&db, chat_id, "C");
    let x = insert_before(&db, chat_id, b, "X");

    // X lands directly before B; ids of A/B/C are unchanged.
    assert_eq!(queue_ids(&db, chat_id), vec![a, x, b, c]);
    let positions = queue_positions(&db, chat_id);
    assert_eq!(positions, vec![(a, 1), (x, 2), (b, 3), (c, 4)]);
    // Positions stay unique.
    let mut sorted: Vec<i64> = positions.iter().map(|(_, p)| *p).collect();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), positions.len());

    cleanup(path);
}

#[test]
fn test_queue_insert_before_first() {
    let path = "test_queue_insert_before_first.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let a = append(&db, chat_id, "A");
    let b = append(&db, chat_id, "B");
    let c = append(&db, chat_id, "C");
    let x = insert_before(&db, chat_id, a, "X");

    // X becomes the new head; the rest keep their relative order.
    assert_eq!(queue_ids(&db, chat_id), vec![x, a, b, c]);
    assert_eq!(queue_positions(&db, chat_id), vec![(x, 1), (a, 2), (b, 3), (c, 4)]);

    cleanup(path);
}

#[test]
fn test_queue_two_inserts_before_same_anchor() {
    let path = "test_queue_two_inserts_before_anchor.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let m = append(&db, chat_id, "M");
    let p1 = insert_before(&db, chat_id, m, "P1");
    let p2 = insert_before(&db, chat_id, m, "P2");

    // Sequential inserts before the same anchor yield config order: P1, P2, M.
    // This is what the commands plugin relies on for multi-prompt ordering.
    assert_eq!(queue_ids(&db, chat_id), vec![p1, p2, m]);
    assert_eq!(queue_positions(&db, chat_id), vec![(p1, 1), (p2, 2), (m, 3)]);

    cleanup(path);
}

#[test]
fn test_queue_insert_before_unknown_id() {
    let path = "test_queue_insert_unknown.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();
    let a = append(&db, chat_id, "A");

    let result = db.add_queue_message(chat_id, Some(9999), "user", "X", None, None, None);
    assert!(matches!(result, Err(DbError::NotFound(_))));

    // Queue unchanged after the failed insert.
    assert_eq!(queue_ids(&db, chat_id), vec![a]);
    assert_eq!(queue_positions(&db, chat_id), vec![(a, 1)]);

    cleanup(path);
}

#[test]
fn test_queue_insert_before_other_chat() {
    let path = "test_queue_insert_other_chat.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat1 = db.create_chat("Chat 1").unwrap();
    let chat2 = db.create_chat("Chat 2").unwrap();
    let a = append(&db, chat1, "A");
    let m2 = append(&db, chat2, "M2");

    // Anchor from another chat is rejected as NotFound (no cross-chat leak).
    let result = db.add_queue_message(chat1, Some(m2), "user", "X", None, None, None);
    assert!(matches!(result, Err(DbError::NotFound(_))));

    // Both queues unchanged.
    assert_eq!(queue_ids(&db, chat1), vec![a]);
    assert_eq!(queue_ids(&db, chat2), vec![m2]);
    assert_eq!(queue_positions(&db, chat2), vec![(m2, 1)]);

    cleanup(path);
}

#[test]
fn test_queue_position_survives_delete() {
    let path = "test_queue_position_survives_delete.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let a = append(&db, chat_id, "A");
    let b = append(&db, chat_id, "B");
    let c = append(&db, chat_id, "C");
    let x = insert_before(&db, chat_id, b, "X");
    assert_eq!(queue_ids(&db, chat_id), vec![a, x, b, c]);

    db.delete_queue_message(x).unwrap();

    // Remaining order is stable after the delete.
    assert_eq!(queue_ids(&db, chat_id), vec![a, b, c]);
    assert_eq!(queue_positions(&db, chat_id), vec![(a, 1), (b, 3), (c, 4)]);

    // An append goes to the end (MAX+1), not into the gap.
    let d = append(&db, chat_id, "D");
    assert_eq!(queue_ids(&db, chat_id), vec![a, b, c, d]);
    assert_eq!(queue_positions(&db, chat_id), vec![(a, 1), (b, 3), (c, 4), (d, 5)]);

    cleanup(path);
}

#[test]
fn test_queue_position_independent_chats() {
    let path = "test_queue_position_independent.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat1 = db.create_chat("Chat 1").unwrap();
    let chat2 = db.create_chat("Chat 2").unwrap();

    // Interleave adds across the two chats.
    let a1 = append(&db, chat1, "A1");
    let b1 = append(&db, chat2, "B1");
    let a2 = append(&db, chat1, "A2");
    let b2 = append(&db, chat2, "B2");

    // Positions are per-chat: each chat numbers 1..n independently.
    assert_eq!(queue_positions(&db, chat1), vec![(a1, 1), (a2, 2)]);
    assert_eq!(queue_positions(&db, chat2), vec![(b1, 1), (b2, 2)]);

    // A positional insert in chat1 never shifts chat2.
    let x = insert_before(&db, chat1, a1, "X");
    assert_eq!(queue_ids(&db, chat1), vec![x, a1, a2]);
    assert_eq!(queue_positions(&db, chat2), vec![(b1, 1), (b2, 2)]);

    cleanup(path);
}
