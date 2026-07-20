use super::*;

#[test]
fn test_ws_request_create_chat_serialization() {
    let req = WsRequest::CreateChat {
        id: "req-1".to_string(),
        title: "Test Chat".to_string(),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains(r#""type":"createChat""#));
    assert!(json.contains(r#""title":"Test Chat""#));

    let deserialized: WsRequest = serde_json::from_str(&json).unwrap();
    match deserialized {
        WsRequest::CreateChat { id, title } => {
            assert_eq!(id, "req-1");
            assert_eq!(title, "Test Chat");
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_ws_request_send_message_serialization() {
    let req = WsRequest::SendMessage {
        id: "req-2".to_string(),
        chat_id: 42,
        content: "Hello".to_string(),
        model: "gpt-4".to_string(),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains(r#""type":"sendMessage""#));
    assert!(json.contains(r#""chatId":42"#));

    let deserialized: WsRequest = serde_json::from_str(&json).unwrap();
    match deserialized {
        WsRequest::SendMessage { id, chat_id, content, model } => {
            assert_eq!(id, "req-2");
            assert_eq!(chat_id, 42);
            assert_eq!(content, "Hello");
            assert_eq!(model, "gpt-4");
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_ws_request_edit_message_serialization() {
    let req = WsRequest::EditMessage {
        id: "req-3".to_string(),
        message_id: 100,
        content: "Edited".to_string(),
        model: "gpt-4".to_string(),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains(r#""type":"editMessage""#));
    assert!(json.contains(r#""messageId":100"#));
}

#[test]
fn test_ws_request_abort_chat_serialization() {
    let req = WsRequest::AbortChat {
        id: "req-4".to_string(),
        chat_id: 5,
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains(r#""type":"abortChat""#));

    let deserialized: WsRequest = serde_json::from_str(&json).unwrap();
    match deserialized {
        WsRequest::AbortChat { id, chat_id } => {
            assert_eq!(id, "req-4");
            assert_eq!(chat_id, 5);
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_error_code_chat_variants_serialization() {
    let code = ErrorCode::ChatNotFound;
    let json = serde_json::to_string(&code).unwrap();
    assert_eq!(json, r#""CHAT_NOT_FOUND""#);

    let code = ErrorCode::MessageNotFound;
    let json = serde_json::to_string(&code).unwrap();
    assert_eq!(json, r#""MESSAGE_NOT_FOUND""#);

    let code = ErrorCode::ChatStreamFailed;
    let json = serde_json::to_string(&code).unwrap();
    assert_eq!(json, r#""CHAT_STREAM_FAILED""#);
}

#[test]
fn test_chat_stream_chunk_event_serialization() {
    let event = ChatStreamChunkEvent {
        chat_id: 1,
        content: "Hello".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""chatId":1"#));
    assert!(json.contains(r#""content":"Hello""#));
}

#[test]
fn test_chat_stream_finished_event_serialization() {
    let event = ChatStreamFinishedEvent {
        chat_id: 1,
        message_id: 42,
        finish_reason: "stop".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""chatId":1"#));
    assert!(json.contains(r#""messageId":42"#));
    assert!(json.contains(r#""finishReason":"stop""#));
}

#[test]
fn test_chat_message_dto_serialization() {
    let msg = ChatMessageDto {
        id: 42,
        chat_id: 1,
        role: "assistant".to_string(),
        content: "Hello!".to_string(),
        created_at: "2026-06-28T15:00:00Z".to_string(),
        model: Some("gpt-4".to_string()),
        thinking_content: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""chatId":1"#));
    assert!(json.contains(r#""role":"assistant""#));
    assert!(json.contains(r#""createdAt":"2026-06-28T15:00:00Z""#));
    assert!(json.contains(r#""model":"gpt-4""#));
}

#[test]
fn test_chat_message_added_event_serialization() {
    let event = ChatMessageAddedEvent {
        chat_id: 1,
        message: ChatMessageDto {
            id: 42,
            chat_id: 1,
            role: "user".to_string(),
            content: "Hi".to_string(),
            created_at: "2026-06-28T15:00:00Z".to_string(),
            model: Some("gpt-4".to_string()),
            thinking_content: None,
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""chatId":1"#));
    assert!(json.contains(r#""role":"user""#));
    assert!(json.contains(r#""model":"gpt-4""#));
}

#[test]
fn test_flat_cost_calculation() {
    let usage = TokenUsage {
        prompt_tokens: 1000,
        completion_tokens: 500,
        total_tokens: 1500,
    };

    let cost = calculate_cost(&usage, Some(10.0), Some(20.0), None).unwrap();
    assert!((cost - 0.02).abs() < 1e-10);
}

#[test]
fn test_tiered_cost_calculation() {
    let usage = TokenUsage {
        prompt_tokens: 300_000,
        completion_tokens: 100_000,
        total_tokens: 400_000,
    };

    let tiers = vec![
        TokenPriceTier {
            after_tokens: 250_000,
            input_token_price: 5.0,
            output_token_price: 15.0,
        },
        TokenPriceTier {
            after_tokens: 500_000,
            input_token_price: 10.0,
            output_token_price: 30.0,
        },
    ];

    let cost = calculate_cost(&usage, None, None, Some(&tiers)).unwrap();

    let expected = (250_000.0 * 5.0 / 1_000_000.0)
        + (50_000.0 * 10.0 / 1_000_000.0)
        + (100_000.0 * 15.0 / 1_000_000.0);
    assert!((cost - expected).abs() < 1e-10);
}

#[test]
fn test_no_pricing_returns_none() {
    let usage = TokenUsage {
        prompt_tokens: 1000,
        completion_tokens: 500,
        total_tokens: 1500,
    };

    assert!(calculate_cost(&usage, None, None, None).is_none());
}

#[test]
fn test_event_type_debug_lowercase_serialization() {
    let event = EventType::ScenarioResumed;
    let debug_str = format!("{:?}", event);
    let lowercase = debug_str.to_lowercase();
    assert_eq!(lowercase, "scenarioresumed");
}

#[test]
fn test_event_type_serde_serialization() {
    let event = EventType::ScenarioResumed;
    let json = serde_json::to_string(&event).unwrap();
    assert_eq!(json, r#""scenarioResumed""#);
}

#[test]
fn test_ws_request_set_role_serialization() {
    let req = WsRequest::SetRole {
        id: "req-1".to_string(),
        chat_id: 42,
        project_name: "project-a".to_string(),
        role_name: "developer".to_string(),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains(r#""type":"setRole""#));
    assert!(json.contains(r#""chatId":42"#));
    assert!(json.contains(r#""projectName":"project-a""#));
    assert!(json.contains(r#""roleName":"developer""#));

    let deserialized: WsRequest = serde_json::from_str(&json).unwrap();
    match deserialized {
        WsRequest::SetRole { id, chat_id, project_name, role_name } => {
            assert_eq!(id, "req-1");
            assert_eq!(chat_id, 42);
            assert_eq!(project_name, "project-a");
            assert_eq!(role_name, "developer");
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_ws_request_get_available_roles_serialization() {
    let req = WsRequest::GetAvailableRoles {
        id: "req-2".to_string(),
        chat_id: 42,
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains(r#""type":"getAvailableRoles""#));
    assert!(json.contains(r#""chatId":42"#));
}

#[test]
fn test_role_info_serialization() {
    let info = RoleInfo {
        project_name: "project-a".to_string(),
        role_name: "developer".to_string(),
        when_to_use: "Use for coding tasks.".to_string(),
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains(r#""projectName":"project-a""#));
    assert!(json.contains(r#""roleName":"developer""#));
    assert!(json.contains(r#""whenToUse":"Use for coding tasks.""#));
}

#[test]
fn test_role_changed_event_serialization() {
    let event = RoleChangedEvent {
        chat_id: 42,
        project_name: "project-a".to_string(),
        role_name: "developer".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""chatId":42"#));
    assert!(json.contains(r#""projectName":"project-a""#));
    assert!(json.contains(r#""roleName":"developer""#));
}

#[test]
fn test_roles_updated_event_serialization() {
    let event = RolesUpdatedEvent {
        chat_id: 42,
        roles: vec![
            RoleInfo {
                project_name: "project-a".to_string(),
                role_name: "developer".to_string(),
                when_to_use: "Use for coding.".to_string(),
            },
        ],
        active_role_project: Some("project-a".to_string()),
        active_role_name: Some("developer".to_string()),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""chatId":42"#));
    assert!(json.contains(r#""roles":["#));
    assert!(json.contains(r#""activeRoleProject":"project-a""#));
    assert!(json.contains(r#""activeRoleName":"developer""#));
}

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
