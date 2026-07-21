# Phase 5: WebSocket Protocol

## Goal

Add WebSocket events and requests for role management. This phase enables the frontend to interact with the role system via the WebSocket protocol.

## Current State Analysis

### WebSocket Request Types (`packages/rhd_api/src/lib.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum WsRequest {
    // ... existing variants ...
    
    // Chat operations
    #[serde(rename = "createChat")]
    CreateChat { id: String, title: String },
    #[serde(rename = "sendMessage", rename_all = "camelCase")]
    SendMessage { id: String, chat_id: i64, content: String, model: String },
    // ... etc
}
```

### WebSocket Event Types (`packages/rhd_api/src/lib.rs`)

Events are defined as separate structs and sent via `WsEvent::new()`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamChunkEvent {
    pub chat_id: i64,
    pub content: String,
}

// Sent via:
WsEvent::new("chatStreamChunk", serde_json::to_value(&payload)?)
```

### WebSocket Handler (`packages/rhd_app/src/ws.rs`)

```rust
async fn handle_ws_message(text: &str, state: &Arc<DaemonState>) -> WsResponse {
    let request: WsRequest = serde_json::from_str(text)?;
    
    match request {
        WsRequest::SendMessage { id, chat_id, content, model } => {
            handle_send_message(id, chat_id, content, model, state).await
        }
        // ... other handlers
    }
}
```

Chat events are forwarded from `ChatEvent` to WebSocket events in the event loop:

```rust
chat_event = chat_events_rx.recv() => {
    match chat_event {
        Ok(ChatEvent::StreamChunk { chat_id, content }) => {
            let payload = ChatStreamChunkEvent { chat_id, content };
            WsEvent::new("chatStreamChunk", serde_json::to_value(&payload)?)
        }
        // ... other events
    }
}
```

## Implementation Plan

### 5.1 Add RoleInfo DTO

**File: `packages/rhd_api/src/lib.rs`**

Add DTO for role information:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleInfo {
    pub project_name: String,
    pub role_name: String,
    pub when_to_use: String,
}
```

### 5.2 Add WebSocket Request Variants

**File: `packages/rhd_api/src/lib.rs`**

Add new request variants to `WsRequest` enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum WsRequest {
    // ... existing variants ...
    
    // Role operations
    #[serde(rename = "setRole", rename_all = "camelCase")]
    SetRole {
        id: String,
        chat_id: i64,
        project_name: String,
        role_name: String,
    },
    #[serde(rename = "getAvailableRoles", rename_all = "camelCase")]
    GetAvailableRoles { id: String, chat_id: i64 },
    #[serde(rename = "clearActiveRole", rename_all = "camelCase")]
    ClearActiveRole { id: String, chat_id: i64 },
}
```

### 5.3 Add WebSocket Event Payload Structs

**File: `packages/rhd_api/src/lib.rs`**

Add event payload structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleChangedEvent {
    pub chat_id: i64,
    pub project_name: String,
    pub role_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolesUpdatedEvent {
    pub chat_id: i64,
    pub roles: Vec<RoleInfo>,
    pub active_role_project: Option<String>,
    pub active_role_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRoleClearedEvent {
    pub chat_id: i64,
}
```

### 5.4 Add ChatEvent Variants for Role Changes

**File: `packages/rhd_chat/src/event.rs`**

Add new event variants to `ChatEvent` enum:

```rust
pub enum ChatEvent {
    // ... existing variants ...
    
    RoleChanged {
        chat_id: i64,
        project_name: String,
        role_name: String,
    },
    RolesUpdated {
        chat_id: i64,
    },
    ActiveRoleCleared {
        chat_id: i64,
    },
}
```

### 5.5 Emit Role Events from ChatManager

**File: `packages/rhd_chat/src/manager.rs`**

Update `set_active_role()` to emit `RoleChanged` event:

```rust
pub async fn set_active_role(
    &self,
    chat_id: i64,
    project_name: &str,
    role_name: &str,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // ... existing validation and DB update ...
    
    // Inject role's system prompt
    projects::inject_role_system_prompt(
        &self.db,
        &self.project_provider,
        chat_id,
        project_name,
        role_name,
        &event_sender,
    )?;
    
    // NEW: Emit RoleChanged event
    let _ = event_sender.send(ChatEvent::RoleChanged {
        chat_id,
        project_name: project_name.to_string(),
        role_name: role_name.to_string(),
    });
    
    Ok(())
}
```

Add method to clear active role with event:

```rust
pub async fn clear_active_role(
    &self,
    chat_id: i64,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    if self.db.get_chat(chat_id)?.is_none() {
        return Err(ChatError::ChatNotFound);
    }
    self.db.clear_active_role(chat_id)?;
    self.db.reset_roles_list_injected(chat_id)?;
    
    let _ = event_sender.send(ChatEvent::ActiveRoleCleared { chat_id });
    
    Ok(())
}
```

### 5.6 Emit RolesUpdated Event on Project Attach/Detach

**File: `packages/rhd_chat/src/projects.rs`**

Update `attach_project()` to emit `RolesUpdated` event when roles change:

```rust
pub async fn attach_project<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    project_name: &str,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // ... existing code ...
    
    // If project has roles, reset roles list injection flag and emit event
    let roles = project_provider.get_project_roles(project_name);
    if !roles.is_empty() {
        db.reset_roles_list_injected(chat_id)?;
        
        // Emit RolesUpdated event
        let _ = event_sender.send(ChatEvent::RolesUpdated { chat_id });
    }
    
    let _ = event_sender.send(ChatEvent::ProjectAttached {
        chat_id,
        project_name: project_name.to_string(),
    });
    
    Ok(())
}
```

Update `detach_project()` similarly:

```rust
pub fn detach_project(
    db: &Arc<ChatDb>,
    project_provider: &Arc<impl ProjectProvider>,
    chat_id: i64,
    project_name: &str,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // Check if project has roles before detaching
    let had_roles = !project_provider.get_project_roles(project_name).is_empty();
    
    db.detach_project(chat_id, project_name)?;
    
    // If project had roles, emit RolesUpdated event
    if had_roles {
        db.reset_roles_list_injected(chat_id)?;
        let _ = event_sender.send(ChatEvent::RolesUpdated { chat_id });
    }
    
    let _ = event_sender.send(ChatEvent::ProjectDetached {
        chat_id,
        project_name: project_name.to_string(),
    });
    
    Ok(())
}
```

**Note:** The signature of `detach_project()` changes to accept `project_provider`. This requires updating the call site in `ChatManager::detach_project()`.

### 5.7 Add WebSocket Request Handlers

**File: `packages/rhd_app/src/ws.rs`**

Add handlers for new request types:

```rust
async fn handle_ws_message(text: &str, state: &Arc<DaemonState>) -> WsResponse {
    let request: WsRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            return WsResponse::error(
                "unknown".to_string(),
                ErrorCode::InvalidRequest,
                format!("invalid JSON: {}", e),
            );
        }
    };

    match request {
        // ... existing handlers ...
        
        WsRequest::SetRole { id, chat_id, project_name, role_name } => {
            handle_set_role(id, chat_id, project_name, role_name, state).await
        }
        WsRequest::GetAvailableRoles { id, chat_id } => {
            handle_get_available_roles(id, chat_id, state)
        }
        WsRequest::ClearActiveRole { id, chat_id } => {
            handle_clear_active_role(id, chat_id, state).await
        }
    }
}

async fn handle_set_role(
    id: String,
    chat_id: i64,
    project_name: String,
    role_name: String,
    state: &Arc<DaemonState>,
) -> WsResponse {
    match state
        .chat_manager
        .set_active_role(chat_id, &project_name, &role_name, state.chat_event_sender.clone())
        .await
    {
        Ok(()) => WsResponse::success(id, serde_json::json!({ "set": true })),
        Err(rhd_chat::ChatError::ChatNotFound) => WsResponse::error(
            id,
            ErrorCode::ChatNotFound,
            format!("chat not found: {}", chat_id),
        ),
        Err(rhd_chat::ChatError::RoleNotFound(role)) => WsResponse::error(
            id,
            ErrorCode::InvalidRequest,
            format!("role not found: {}", role),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to set role: {}", err),
        ),
    }
}

fn handle_get_available_roles(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.get_available_roles(chat_id) {
        Ok(roles) => {
            let role_infos: Vec<RoleInfo> = roles
                .into_iter()
                .map(|(project_name, role_name, when_to_use)| RoleInfo {
                    project_name,
                    role_name,
                    when_to_use,
                })
                .collect();
            
            let active_role = state.chat_manager.get_active_role(chat_id).ok().flatten();
            
            let data = serde_json::json!({
                "roles": role_infos,
                "activeRoleProject": active_role.as_ref().map(|(p, _)| p),
                "activeRoleName": active_role.as_ref().map(|(_, r)| r),
            });
            WsResponse::success(id, data)
        }
        Err(rhd_chat::ChatError::ChatNotFound) => WsResponse::error(
            id,
            ErrorCode::ChatNotFound,
            format!("chat not found: {}", chat_id),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to get available roles: {}", err),
        ),
    }
}

async fn handle_clear_active_role(
    id: String,
    chat_id: i64,
    state: &Arc<DaemonState>,
) -> WsResponse {
    match state
        .chat_manager
        .clear_active_role(chat_id, state.chat_event_sender.clone())
        .await
    {
        Ok(()) => WsResponse::success(id, serde_json::json!({ "cleared": true })),
        Err(rhd_chat::ChatError::ChatNotFound) => WsResponse::error(
            id,
            ErrorCode::ChatNotFound,
            format!("chat not found: {}", chat_id),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to clear active role: {}", err),
        ),
    }
}
```

### 5.8 Forward Role Events to WebSocket

**File: `packages/rhd_app/src/ws.rs`**

Update the chat event forwarding loop to handle new role events:

```rust
chat_event = chat_events_rx.recv() => {
    match chat_event {
        Ok(chat_evt) => {
            let ws_event = match chat_evt {
                // ... existing event handling ...
                
                ChatEvent::RoleChanged { chat_id, project_name, role_name } => {
                    let payload = RoleChangedEvent {
                        chat_id,
                        project_name,
                        role_name,
                    };
                    WsEvent::new("roleChanged", serde_json::to_value(&payload)?)
                }
                ChatEvent::RolesUpdated { chat_id } => {
                    // Build roles list for the event
                    let inner = state.inner.read().await;
                    let roles = inner.chat_manager.get_available_roles(chat_id).unwrap_or_default();
                    let active_role = inner.chat_manager.get_active_role(chat_id).ok().flatten();
                    
                    let role_infos: Vec<RoleInfo> = roles
                        .into_iter()
                        .map(|(project_name, role_name, when_to_use)| RoleInfo {
                            project_name,
                            role_name,
                            when_to_use,
                        })
                        .collect();
                    
                    let payload = RolesUpdatedEvent {
                        chat_id,
                        roles: role_infos,
                        active_role_project: active_role.as_ref().map(|(p, _)| p.clone()),
                        active_role_name: active_role.as_ref().map(|(_, r)| r.clone()),
                    };
                    WsEvent::new("rolesUpdated", serde_json::to_value(&payload)?)
                }
                ChatEvent::ActiveRoleCleared { chat_id } => {
                    let payload = ActiveRoleClearedEvent { chat_id };
                    WsEvent::new("activeRoleCleared", serde_json::to_value(&payload)?)
                }
            };
            let event_text = serde_json::to_string(&ws_event)?;
            write.send(Message::Text(event_text)).await?;
        }
        Err(broadcast::error::RecvError::Lagged(_)) => continue,
        Err(broadcast::error::RecvError::Closed) => break,
    }
}
```

### 5.9 Add Unit Tests

**File: `packages/rhd_api/src/lib.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

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
}
```

## Files to Modify

1. **`packages/rhd_api/src/lib.rs`**
   - Add `RoleInfo` DTO
   - Add `SetRole`, `GetAvailableRoles`, `ClearActiveRole` variants to `WsRequest`
   - Add `RoleChangedEvent`, `RolesUpdatedEvent`, `ActiveRoleClearedEvent` structs
   - Add unit tests

2. **`packages/rhd_chat/src/event.rs`**
   - Add `RoleChanged`, `RolesUpdated`, `ActiveRoleCleared` variants to `ChatEvent`

3. **`packages/rhd_chat/src/manager.rs`**
   - Update `set_active_role()` to emit `RoleChanged` event
   - Update `clear_active_role()` to emit `ActiveRoleCleared` event

4. **`packages/rhd_chat/src/projects.rs`**
   - Update `attach_project()` to emit `RolesUpdated` event
   - Update `detach_project()` signature and emit `RolesUpdated` event

5. **`packages/rhd_app/src/ws.rs`**
   - Add `handle_set_role()`, `handle_get_available_roles()`, `handle_clear_active_role()` handlers
   - Update `handle_ws_message()` to route new request types
   - Update chat event forwarding to handle role events

## Dependencies

- Phase 1: Role struct and loading
- Phase 2: ProjectProvider trait extensions, DB methods
- Phase 3: Role injection logic, ChatManager role methods
- Phase 4: rhd_set_role tool (for consistency, though not strictly required)

## Success Criteria

1. ✅ `RoleInfo` DTO defined and serializable
2. ✅ `SetRole` request variant added and handled
3. ✅ `GetAvailableRoles` request variant added and handled
4. ✅ `ClearActiveRole` request variant added and handled
5. ✅ `RoleChanged` event emitted when role changes
6. ✅ `RolesUpdated` event emitted when roles list changes
7. ✅ `ActiveRoleCleared` event emitted when role is cleared
8. ✅ WebSocket handlers return appropriate responses
9. ✅ Error handling for invalid requests (chat not found, role not found)
10. ✅ All unit tests pass
11. ✅ Existing WebSocket tests still pass

## Design Decisions

1. **Separate events for role changes**: `RoleChanged` is emitted when the active role changes. `RolesUpdated` is emitted when the list of available roles changes (project attach/detach). This allows the frontend to handle these cases differently.

2. **RolesUpdated includes full roles list**: The event includes the complete list of available roles and the current active role. This simplifies frontend state management.

3. **ClearActiveRole as separate request**: Instead of setting role to null via `SetRole`, we have a separate `ClearActiveRole` request. This makes the intent clearer.

4. **Event forwarding from ChatEvent to WsEvent**: Role events follow the same pattern as other chat events. They're emitted as `ChatEvent` variants and forwarded to WebSocket as `WsEvent`.

## Next Steps

After this phase:
- Phase 6 will implement the frontend role selector UI
- Phase 7 will handle edge cases and integration testing
