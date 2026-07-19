use super::*;
use rhd_api::{TodoItemDto, TodoListUpdatedEvent, WsEvent};
use rhd_chat::{ChatEvent, TodoStatus};

#[test]
fn test_ws_event_conversion() {
    let chat_event = ChatEvent::TodoListUpdated {
        chat_id: 42,
        items: vec![
            rhd_chat::TodoItem {
                content: "Test task".to_string(),
                status: TodoStatus::Completed,
            },
        ],
    };
    
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
