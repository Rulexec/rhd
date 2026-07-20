# Phase 4: Backend - WebSocket Protocol

## Overview

This phase adds WebSocket events for real-time todo list updates to the frontend. When the AI calls the `rhd_set_todo_list` tool, the frontend receives immediate notification to update the UI.

## Files to Modify

### 1. `packages/rhd_api/src/lib.rs`

**Add TodoListUpdated event DTO:**

```rust
// ============================================================================
// Todo list event types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItemDto {
    pub content: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoListUpdatedEvent {
    pub chat_id: i64,
    pub items: Vec<TodoItemDto>,
}
```

**Add serialization tests:**

```rust
#[test]
fn test_todo_item_dto_serialization() {
    let item = TodoItemDto {
        content: "Test task".to_string(),
        status: "completed".to_string(),
    };
    let json = serde_json::to_string(&item).unwrap();
    assert!(json.contains(r#""content":"Test task""#));
    assert!(json.contains(r#""status":"completed""#));
}

#[test]
fn test_todo_list_updated_event_serialization() {
    let event = TodoListUpdatedEvent {
        chat_id: 42,
        items: vec![
            TodoItemDto {
                content: "Task 1".to_string(),
                status: "completed".to_string(),
            },
            TodoItemDto {
                content: "Task 2".to_string(),
                status: "in_progress".to_string(),
            },
        ],
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""chatId":42"#));
    assert!(json.contains(r#""items":["#));
    assert!(json.contains(r#""content":"Task 1""#));
    assert!(json.contains(r#""status":"completed""#));
}
```

### 2. `packages/rhd_chat/src/event.rs`

**Add TodoListUpdated variant:**

```rust
#[derive(Debug, Clone)]
pub enum ChatEvent {
    // ... existing variants ...
    TodoListUpdated {
        chat_id: i64,
        items: Vec<crate::TodoItem>,
    },
}
```

### 3. `packages/rhd_app/src/ws.rs`

**Handle TodoListUpdated event:**

```rust
use rhd_api::{
    // ... existing imports ...
    TodoItemDto, TodoListUpdatedEvent,
};

// In the chat_events_rx handler match statement:
ChatEvent::TodoListUpdated { chat_id, items } => {
    let dto_items: Vec<TodoItemDto> = items
        .into_iter()
        .map(|item| TodoItemDto {
            content: item.content,
            status: item.status.as_str().to_string(),
        })
        .collect();
    
    let payload = TodoListUpdatedEvent {
        chat_id,
        items: dto_items,
    };
    WsEvent::new("todoListUpdated", serde_json::to_value(&payload)?)
}
```

### 4. `packages/rhd_chat/src/tools.rs`

**Emit TodoListUpdated event in handler:**

```rust
pub async fn handle_rhd_set_todo_list(
    manager: &ChatManager<impl ProjectProvider>,
    chat_id: i64,
    arguments: &str,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> ToolResult {
    // ... existing argument parsing ...
    
    // Parse and validate the todo list
    let todo_items = match parse_todo_list(&todos) {
        Ok(items) => items,
        Err(e) => {
            return ToolResult {
                content: format!("Error: failed to parse todo list: {}", e),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };
    
    // Store in database
    match manager.db().set_todo_list(chat_id, &todos) {
        Ok(()) => {
            // Emit event for frontend update
            let _ = event_sender.send(ChatEvent::TodoListUpdated {
                chat_id,
                items: todo_items.clone(),
            });
            
            ToolResult {
                content: "Todo list updated successfully.".to_string(),
                is_error: Some(false),
                raw_response: None,
            }
        }
        Err(e) => ToolResult {
            content: format!("Error: failed to save todo list: {}", e),
            is_error: Some(true),
            raw_response: None,
        },
    }
}
```

## Event Format

### WebSocket Event Message

```json
{
  "type": "event",
  "event": "todoListUpdated",
  "data": {
    "chatId": 123,
    "items": [
      { "content": "Analyze requirements", "status": "completed" },
      { "content": "Design solution", "status": "in_progress" },
      { "content": "Implement changes", "status": "pending" },
      { "content": "Old task no longer needed", "status": "discarded" }
    ]
  }
}
```

### Status Values

| Status | Description |
|--------|-------------|
| `pending` | Task not yet started |
| `in_progress` | Task currently being worked on |
| `completed` | Task finished |
| `discarded` | Task no longer needed |

## Event Flow

```
1. AI calls rhd_set_todo_list tool
2. handle_rhd_set_todo_list() parses and validates input
3. Todo list is saved to database
4. ChatEvent::TodoListUpdated is emitted
5. ws.rs receives the event
6. Event is converted to WsEvent with "todoListUpdated" name
7. WebSocket sends event to frontend
8. Frontend updates todo list store and UI
```

## Tests

### Event Serialization Tests

```rust
#[test]
fn test_todo_list_updated_event_format() {
    let event = TodoListUpdatedEvent {
        chat_id: 42,
        items: vec![
            TodoItemDto {
                content: "Task 1".to_string(),
                status: "completed".to_string(),
            },
            TodoItemDto {
                content: "Task 2".to_string(),
                status: "in_progress".to_string(),
            },
            TodoItemDto {
                content: "Task 3".to_string(),
                status: "pending".to_string(),
            },
            TodoItemDto {
                content: "Task 4".to_string(),
                status: "discarded".to_string(),
            },
        ],
    };
    
    let json = serde_json::to_value(&event).unwrap();
    
    assert_eq!(json["chatId"], 42);
    assert_eq!(json["items"].as_array().unwrap().len(), 4);
    assert_eq!(json["items"][0]["content"], "Task 1");
    assert_eq!(json["items"][0]["status"], "completed");
    assert_eq!(json["items"][1]["status"], "in_progress");
    assert_eq!(json["items"][2]["status"], "pending");
    assert_eq!(json["items"][3]["status"], "discarded");
}

#[test]
fn test_todo_list_updated_event_empty_items() {
    let event = TodoListUpdatedEvent {
        chat_id: 1,
        items: vec![],
    };
    
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["chatId"], 1);
    assert_eq!(json["items"].as_array().unwrap().len(), 0);
}
```

### Event Emission Tests

```rust
#[tokio::test]
async fn test_handle_rhd_set_todo_list_emits_event() {
    let db = Arc::new(ChatDb::new("test_todo_event.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    let (event_sender, mut event_receiver) = broadcast::channel(100);
    
    let args = r#"{"todos": "[x] Task 1\n[-] Task 2"}"#;
    let result = handle_rhd_set_todo_list(&manager, chat_id, args, &event_sender).await;
    
    assert_eq!(result.is_error, Some(false));
    
    // Verify event was emitted
    let event = event_receiver.recv().await.unwrap();
    match event {
        ChatEvent::TodoListUpdated { chat_id: id, items } => {
            assert_eq!(id, chat_id);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].content, "Task 1");
            assert_eq!(items[0].status, crate::TodoStatus::Completed);
            assert_eq!(items[1].content, "Task 2");
            assert_eq!(items[1].status, crate::TodoStatus::InProgress);
        }
        _ => panic!("Expected TodoListUpdated event"),
    }
    
    cleanup("test_todo_event.db");
}
```

### WebSocket Handler Tests

```rust
#[test]
fn test_ws_event_conversion() {
    let chat_event = ChatEvent::TodoListUpdated {
        chat_id: 42,
        items: vec![
            crate::TodoItem {
                content: "Test task".to_string(),
                status: crate::TodoStatus::Completed,
            },
        ],
    };
    
    // Simulate the conversion logic from ws.rs
    let ws_event = match chat_event {
        ChatEvent::TodoListUpdated { chat_id, items } => {
            let dto_items: Vec<TodoItemDto> = items
                .into_iter()
                .map(|item| TodoItemDto {
                    content: item.content,
                    status: item.status.as_str().to_string(),
                })
                .collect();
            
            let payload = TodoListUpdatedEvent {
                chat_id,
                items: dto_items,
            };
            WsEvent::new("todoListUpdated", serde_json::to_value(&payload).unwrap())
        }
        _ => panic!("Wrong event type"),
    };
    
    assert_eq!(ws_event.r#type, "event");
    assert_eq!(ws_event.event, "todoListUpdated");
    assert_eq!(ws_event.data["chatId"], 42);
    assert_eq!(ws_event.data["items"].as_array().unwrap().len(), 1);
}
```

## Implementation Notes

1. **Event naming**: The WebSocket event is named `todoListUpdated` (camelCase) to match frontend conventions.

2. **Status mapping**: The `TodoStatus` enum is converted to lowercase strings (`pending`, `in_progress`, `completed`, `discarded`) for JSON serialization.

3. **Real-time updates**: The event is emitted immediately after the tool succeeds, ensuring the frontend reflects changes without delay.

4. **Event propagation**: Uses the existing `broadcast::Sender<ChatEvent>` channel, so no new infrastructure is needed.

5. **DTO conversion**: The `TodoItem` type from `rhd_chat` is converted to `TodoItemDto` from `rhd_api` for clean separation of concerns.

6. **Error handling**: If event emission fails (e.g., no receivers), the tool still succeeds. The event is best-effort for frontend updates.

## Dependencies

- This phase depends on Phase 1 (Tool Definition & Storage) for the `TodoItem` type and event emission
- This phase must be completed before Phase 5 (Frontend Stores)
- Phase 6 (Frontend UI) depends on the WebSocket event format defined here
