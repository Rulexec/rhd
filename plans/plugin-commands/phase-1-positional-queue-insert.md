# Phase 1: Positional Queue Insert (DB + API + Server)

## Overview

Give the message queue a stable, explicit ordering key (`position`) and extend `addQueueMessage` with an optional `beforeMessageId` so clients can insert a queued message directly before another queued message. Today the queue is ordered by auto-increment `id` (`packages/rhd_db/src/chat_db/messages_queue.rs:49` — `ORDER BY id ASC`) and `addQueueMessage` can only append.

**In scope:** `rhd_db` schema + migration + queue functions, `rhd_chat_api` params type, `rhd_chat_server` handler, tests.
**Out of scope:** the `preDrainQueue` event (Phase 2), the commands plugin (Phases 3–4), any frontend change.

**Why a separate column instead of id renumbering:** queue message ids appear in `queueMessageAdded/Updated/Deleted` events and in frontend state; renumbering ids would silently desync live clients. A separate `position` keeps ids immutable. **`position` is a server-internal key — it is deliberately NOT exposed on the wire `Message` payload.**

**Dependencies:** none. Must be completed before Phase 4 (the executor uses `beforeMessageId`).

## Files to Modify

### 1. `packages/rhd_db/src/chat_db/schema.rs`

**Modify — `messages_queue` CREATE TABLE** (currently lines 104–119). Add the `position` column:

```sql
CREATE TABLE IF NOT EXISTS messages_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    model TEXT,
    thinking_content TEXT,
    tool_calls TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
);
```

**Modify — migration section** (the `pragma_table_info` chain, insert after the `tool_call_id`-on-`messages_queue` block at lines 256–264, following the exact existing style):

```rust
// Check if position column exists in messages_queue table
let has_queue_position: bool = conn
    .prepare("SELECT COUNT(*) FROM pragma_table_info('messages_queue') WHERE name='position'")?
    .query_row([], |row| row.get::<_, i64>(0))?
    > 0;

if !has_queue_position {
    conn.execute_batch(
        "ALTER TABLE messages_queue ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
         UPDATE messages_queue SET position = id;",
    )?;
}
```

The backfill `position = id` preserves the existing effective order for pre-migration rows (they were ordered by id). `ALTER TABLE ... ADD COLUMN NOT NULL DEFAULT` is legal in SQLite (constant default).

### 2. `packages/rhd_db/src/chat_db/messages_queue.rs`

**Modify — `add_queue_message`** (line 12). New signature — one extra parameter `before_message_id: Option<i64>` inserted after `chat_id` (keep all other params and the `(message_id, new_version)` return shape):

```rust
pub(crate) fn add_queue_message(
    conn: &Mutex<Connection>,
    chat_id: i64,
    before_message_id: Option<i64>,
    role: &str,
    content: &str,
    model: Option<&str>,
    thinking_content: Option<&str>,
    tool_call_id: Option<&str>,
) -> DbResult<(i64, i64)>
```

Position computation inside the existing transaction (after `let tx = conn.unchecked_transaction()?;`):

```rust
let position: i64 = match before_message_id {
    Some(before_id) => {
        // Resolve the anchor row; it must exist and belong to the same chat.
        let anchor: Option<(i64, i64)> = tx
            .query_row(
                "SELECT chat_id, position FROM messages_queue WHERE id = ?1",
                params![before_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        let (anchor_chat_id, anchor_position) = anchor
            .ok_or_else(|| DbError::NotFound(format!("queue message {}", before_id)))?;
        if anchor_chat_id != chat_id {
            return Err(DbError::NotFound(format!(
                "queue message {} does not belong to chat {}",
                before_id, chat_id
            )));
        }
        // Make room: everything at/after the anchor shifts one position right.
        tx.execute(
            "UPDATE messages_queue SET position = position + 1 WHERE chat_id = ?1 AND position >= ?2",
            params![chat_id, anchor_position],
        )?;
        anchor_position
    }
    None => {
        // Append: one past the current maximum for this chat.
        let max_position: Option<i64> = tx.query_row(
            "SELECT MAX(position) FROM messages_queue WHERE chat_id = ?1",
            params![chat_id],
            |row| row.get(0),
        )?;
        max_position.map(|p| p + 1).unwrap_or(1)
    }
};
```

Extend the `INSERT` to include the new column:

```sql
INSERT INTO messages_queue (chat_id, role, content, created_at, model, thinking_content, tool_call_id, position)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
```

Chat-version bump and commit stay as-is (the whole thing is one tx — appends and positional inserts are equally atomic).

**Modify — `get_queue_messages`** (line 49): change the trailing clause to `ORDER BY position ASC, id ASC`. The `id ASC` tiebreak matters: rows migrated from pre-`position` databases and the vestigial `insert_queue_message` path could otherwise tie.

**Modify — `insert_queue_message`** (line 140, vestigial but public on `ChatDb`): add `position` to its INSERT with the value `message.id` (`params!` list gains it after `tool_call_id`), matching the migration backfill convention so it cannot collide into the front of the ordering.

No changes needed to `update_queue_message`, `delete_*`, `count_queue_messages`.

### 3. `packages/rhd_db/src/chat_db/mod.rs`

**Modify — `ChatDb::add_queue_message` wrapper** (line ~287): thread the new `before_message_id: Option<i64>` parameter through to `messages_queue::add_queue_message`.

### 4. `packages/rhd_chat_api/src/methods/add_queue_message.rs`

**Modify — `AddQueueMessageParams`** (struct at line 22): add the field after `tags`:

```rust
/// When set, insert this message directly before the queue message with this id
/// instead of appending to the end of the queue.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub before_message_id: Option<i64>,
```

Wire name is `beforeMessageId` (the struct already has `#[serde(rename_all = "camelCase")]`). `default` keeps every existing JSON producer valid.

**Update** the existing test constructors in the module (they build `AddQueueMessageParams` literally — add `before_message_id: None` to each) plus new tests, see Tests.

### 5. `packages/rhd_chat_server/src/handlers/queue_message.rs`

**Modify — `add_queue_message` handler** (line 51). After the existing chat-exists and `tool`-role checks, and BEFORE calling into `db.add_queue_message`, add the user-facing validation (keeps errors as protocol responses rather than internal `?` propagation):

```rust
// Validate positional insertion anchor (must be a queue message of the same chat).
if let Some(before_id) = params.before_message_id {
    match db.get_queue_message(before_id)? {
        None => {
            return Ok(serde_json::to_value(ErrorResponse::message_not_found(
                request_id,
                before_id,
            ))?);
        }
        Some(anchor) if anchor.chat_id != params.chat_id => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!(
                    "beforeMessageId {} belongs to chat {}, not {}",
                    before_id, anchor.chat_id, params.chat_id
                ),
            ))?);
        }
        Some(_) => {}
    }
}
```

And pass the field through:

```rust
let (message_id, mut chat_version) = db.add_queue_message(
    params.chat_id,
    params.before_message_id,
    &params.role,
    &params.content,
    None, // model
    params.reasoning_content.as_deref(),
    params.tool_call_id.as_deref(),
)?;
```

Everything else (tag setting, `queueMessageAdded` broadcast with the full message payload, response) is unchanged — the broadcast already carries the canonical ordered content.

### 6. `packages/rhd_chat_client/src/client.rs`

**Verify, likely no change:** `add_queue_message` (line 711) forwards `AddQueueMessageParams` serialized verbatim, so `beforeMessageId` flows automatically. Confirm there is no manual field-by-field JSON construction; if there is one, add the optional field.

### 7. Existing call sites broken by the two signature/shape changes (mechanical updates)

`ChatDb::add_queue_message` gains a parameter; `AddQueueMessageParams` gains a field. Update every existing literal/call:

- `packages/rhd_db/src/chat_db/tests/message_tests.rs` — lines 189, 213, 214: insert `None` for `before_message_id` after `chat_id`.
- `packages/rhd_db/src/chat_db/tests/migration_tests.rs` — line 146: same.
- `packages/rhd_chat_api/src/methods/add_queue_message.rs` — test constructors at lines ~61 and ~84 region (also covered by Tests section).
- `packages/rhd_chat_server/tests/websocket_tests.rs` — lines 174, 186: add `before_message_id: None` to the params literals.
- `packages/rhd_app/src/commands/queue.rs` — line 17: add `before_message_id: None` (CLI keeps append semantics).

`cargo check` across the workspace should be the verification step for this list.

## Tests

### Unit — `packages/rhd_db` (add to `chat_db/tests/`, e.g. extend `message_tests.rs` or new `queue_position_tests.rs` registered in the tests `mod`)

Follow the existing `test_queue_message_round_trip_with_tool_call_id` pattern (temp `.db` path, `ChatDb::new`):

1. `test_queue_append_order` — three appends → `get_queue_messages` returns ids in insertion order with strictly increasing positions.
2. `test_queue_insert_before_middle` — append A, B, C; insert X before B → order `A, X, B, C`; ids unchanged for A/B/C; positions unique.
3. `test_queue_insert_before_first` — insert before A → new head; rest keep relative order.
4. `test_queue_two_inserts_before_same_anchor` — insert P1 before M, then P2 before M → order `P1, P2, M` (this is what the commands plugin relies on for multi-prompt ordering).
5. `test_queue_insert_before_unknown_id` — `Err(DbError::NotFound(_))`.
6. `test_queue_insert_before_other_chat` — anchor id from another chat → `Err(DbError::NotFound(_))` and queue unchanged.
7. `test_queue_position_survives_delete` — delete the anchor after a positional insert; remaining order stable; append goes to the end (`MAX+1`), not the middle.
8. `test_queue_position_independent_chats` — positions are per-chat: interleaved adds in two chats never shift each other.

### Migration — `packages/rhd_db/src/chat_db/tests/migration_tests.rs`

Mirror the existing pre-`tool_call_id` table recreation test: create the old `messages_queue` DDL (no `position`), insert rows, run the schema init/migration, assert `position = id` backfill and that `get_queue_messages` order is unchanged.

### API serialization — in `packages/rhd_chat_api/src/methods/add_queue_message.rs` `mod tests`

1. `test_add_queue_message_params_before_message_id_serialization` — `before_message_id: Some(42)` round-trips; JSON contains `"beforeMessageId":42`.
2. `test_add_queue_message_params_without_before_message_id` — field omitted from JSON when `None`; deserializing JSON without the key yields `None` (backward compatibility).

### Server protocol — `packages/rhd_chat_server/tests/`

Extend `websocket_tests.rs` (follow its existing request/response helper style) with `test_add_queue_message_positional`:
- create chat, append two queue messages, add third with `beforeMessageId` = second message's id → `getQueueMessages` returns the inserted one between them; `queueMessageAdded` event payload matches.
- `beforeMessageId` = unknown id → error response with `message_not_found`-class status; foreign-chat id → invalid-request error.

## Implementation Notes

1. **Unchecked transactions / single mutex:** all queue writes run under the `ChatDb` connection mutex in one transaction, so a position shift + insert is atomic against every other write — no read skew is possible from the commands plugin.
2. **Do not expose `position` in the wire `Message`** (`packages/rhd_chat_api/src/common.rs`): the drain, the frontend stores, and message conversion all consume `Message` unchanged. Internal column only. The frontend may briefly show a positionally-inserted message at the queue end until the next refresh — acceptable, the insert→drain window is milliseconds in our use case.
3. **`DbError::NotFound` for both unknown and cross-chat anchors** — avoids leaking which chat another message belongs to and matches existing error-variant usage (`chat_db/messages.rs:180`).
4. **Version bump semantics:** the whole positional add bumps `chats.version` once (same as today's append); shift-only updates are covered by the same tx, so no extra broadcast is needed.
5. **Gaps in position numbering are fine** (after deletes); ordering only needs the total order. No renumbering — renumbering would race with concurrent inserts for no benefit.
6. **`convert_message_to_api`** in the server handler ignores the new column — leave it; queue reads simply return in the new order.

## Dependencies

- Depends on: nothing.
- Blocks: Phase 4 (executor uses `before_message_id` on `addQueueMessage`), Phase 5 (e2e).
- Can run in parallel with: Phase 2 and Phase 3.
