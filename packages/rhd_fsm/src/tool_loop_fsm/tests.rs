use crate::*;

fn create_test_message(id: i64, role: &str, content: &str) -> ChatMessage {
    ChatMessage {
        id,
        role: role.to_string(),
        content: content.to_string(),
        thinking_content: None,
        tool_calls: None,
    }
}

fn create_test_tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: format!("Test tool {}", name),
        parameters: serde_json::json!({}),
    }
}

fn create_test_tool_call(id: &str, name: &str) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        arguments: "{}".to_string(),
    }
}

fn create_test_tool_result(tool_call_id: &str, content: &str) -> ToolResult {
    ToolResult {
        tool_call_id: tool_call_id.to_string(),
        content: content.to_string(),
        is_error: false,
    }
}

#[test]
fn test_initial_state() {
    let fsm = ToolLoopFsm::new();
    assert_eq!(fsm.state(), &State::Idle);
    assert!(!fsm.is_finished());
    assert!(fsm.messages().is_empty());
    assert!(fsm.tools().is_empty());
}

#[test]
fn test_insert_message() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Hello");

    let mut inputs = vec![ToolLoopInput::InsertMessage { message: msg.clone() }];
    let actions = fsm.run(&mut inputs).unwrap();

    assert!(actions.is_empty());
    assert_eq!(fsm.messages().len(), 1);
    assert_eq!(fsm.messages()[0], msg);
}

#[test]
fn test_remove_message() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Hello");

    let mut inputs = vec![ToolLoopInput::InsertMessage { message: msg.clone() }];
    fsm.run(&mut inputs).unwrap();

    let mut inputs = vec![ToolLoopInput::RemoveMessage { message_id: 1 }];
    let actions = fsm.run(&mut inputs).unwrap();

    assert!(actions.is_empty());
    assert!(fsm.messages().is_empty());
}

#[test]
fn test_remove_nonexistent_message() {
    let mut fsm = ToolLoopFsm::new();

    let mut inputs = vec![ToolLoopInput::RemoveMessage { message_id: 999 }];
    let result = fsm.run(&mut inputs);

    assert!(matches!(result, Err(FsmError::MessageNotFound { message_id: 999 })));
}

#[test]
fn test_replace_message() {
    let mut fsm = ToolLoopFsm::new();
    let msg1 = create_test_message(1, "user", "Hello");
    let msg2 = create_test_message(1, "user", "Hi there");

    let mut inputs = vec![ToolLoopInput::InsertMessage { message: msg1 }];
    fsm.run(&mut inputs).unwrap();

    let mut inputs = vec![ToolLoopInput::ReplaceMessage { message_id: 1, new_message: msg2.clone() }];
    let actions = fsm.run(&mut inputs).unwrap();

    assert!(actions.is_empty());
    assert_eq!(fsm.messages().len(), 1);
    assert_eq!(fsm.messages()[0], msg2);
}

#[test]
fn test_add_tool() {
    let mut fsm = ToolLoopFsm::new();
    let tool = create_test_tool("test_tool");

    let mut inputs = vec![ToolLoopInput::AddTool { tool: tool.clone() }];
    let actions = fsm.run(&mut inputs).unwrap();

    assert!(actions.is_empty());
    assert_eq!(fsm.tools().len(), 1);
    assert_eq!(fsm.tools()[0], tool);
}

#[test]
fn test_remove_tool() {
    let mut fsm = ToolLoopFsm::new();
    let tool = create_test_tool("test_tool");

    let mut inputs = vec![ToolLoopInput::AddTool { tool: tool.clone() }];
    fsm.run(&mut inputs).unwrap();

    let mut inputs = vec![ToolLoopInput::RemoveTool { tool_name: "test_tool".to_string() }];
    let actions = fsm.run(&mut inputs).unwrap();

    assert!(actions.is_empty());
    assert!(fsm.tools().is_empty());
}

#[test]
fn test_remove_nonexistent_tool() {
    let mut fsm = ToolLoopFsm::new();

    let mut inputs = vec![ToolLoopInput::RemoveTool { tool_name: "nonexistent".to_string() }];
    let result = fsm.run(&mut inputs);

    assert!(matches!(result, Err(FsmError::ToolNotFound { .. })));
}

#[test]
fn test_run_from_idle() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Hello");
    let tool = create_test_tool("test_tool");

    let mut inputs = vec![
        ToolLoopInput::InsertMessage { message: msg },
        ToolLoopInput::AddTool { tool },
    ];
    fsm.run(&mut inputs).unwrap();

    let mut inputs = vec![ToolLoopInput::Run];
    let actions = fsm.run(&mut inputs).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::SendToAi { .. }));
    assert!(matches!(fsm.state(), State::AwaitingAiResponse { .. }));
}

#[test]
fn test_complete_without_tool_calls() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Hello");

    let mut inputs = vec![
        ToolLoopInput::InsertMessage { message: msg },
        ToolLoopInput::Run,
    ];
    fsm.run(&mut inputs).unwrap();

    let mut inputs = vec![ToolLoopInput::ProvideAiResponse {
        content: Some("Hi!".to_string()),
        thinking_content: None,
        tool_calls: vec![],
        finish_reason: "stop".to_string(),
    }];
    let actions = fsm.run(&mut inputs).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::Completed { .. }));
    assert!(matches!(fsm.state(), State::Completed { .. }));
    assert!(fsm.is_finished());
}

#[test]
fn test_tool_calls_flow() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Use tool");
    let tool = create_test_tool("test_tool");

    let mut inputs = vec![
        ToolLoopInput::InsertMessage { message: msg },
        ToolLoopInput::AddTool { tool },
        ToolLoopInput::Run,
    ];
    fsm.run(&mut inputs).unwrap();

    // Provide AI response with tool calls
    let tool_call = create_test_tool_call("call_1", "test_tool");
    let mut inputs = vec![ToolLoopInput::ProvideAiResponse {
        content: Some("Using tool".to_string()),
        thinking_content: None,
        tool_calls: vec![tool_call.clone()],
        finish_reason: "tool_calls".to_string(),
    }];
    let actions = fsm.run(&mut inputs).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(&actions[0], ToolLoopAction::ExecuteToolCall { tool_call: tc } if tc.id == "call_1"));
    assert!(matches!(fsm.state(), State::AwaitingToolResults { .. }));

    // Provide tool result
    let tool_result = create_test_tool_result("call_1", "Tool result");
    let mut inputs = vec![ToolLoopInput::ProvideToolResult {
        tool_call_id: "call_1".to_string(),
        result: tool_result,
    }];
    let actions = fsm.run(&mut inputs).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::SendToAi { .. }));
    assert!(matches!(fsm.state(), State::AwaitingAiResponse { .. }));
}

#[test]
fn test_multiple_tool_calls() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Use tools");

    let mut inputs = vec![
        ToolLoopInput::InsertMessage { message: msg },
        ToolLoopInput::Run,
    ];
    fsm.run(&mut inputs).unwrap();

    // Provide AI response with multiple tool calls
    let tool_call1 = create_test_tool_call("call_1", "tool1");
    let tool_call2 = create_test_tool_call("call_2", "tool2");
    let mut inputs = vec![ToolLoopInput::ProvideAiResponse {
        content: Some("Using tools".to_string()),
        thinking_content: None,
        tool_calls: vec![tool_call1, tool_call2],
        finish_reason: "tool_calls".to_string(),
    }];
    let actions = fsm.run(&mut inputs).unwrap();

    assert_eq!(actions.len(), 2);
    assert!(matches!(&actions[0], ToolLoopAction::ExecuteToolCall { tool_call: tc } if tc.id == "call_1"));
    assert!(matches!(&actions[1], ToolLoopAction::ExecuteToolCall { tool_call: tc } if tc.id == "call_2"));

    // Provide first tool result
    let tool_result1 = create_test_tool_result("call_1", "Result 1");
    let mut inputs = vec![ToolLoopInput::ProvideToolResult {
        tool_call_id: "call_1".to_string(),
        result: tool_result1,
    }];
    let actions = fsm.run(&mut inputs).unwrap();

    assert!(actions.is_empty()); // Still waiting for second result

    // Provide second tool result
    let tool_result2 = create_test_tool_result("call_2", "Result 2");
    let mut inputs = vec![ToolLoopInput::ProvideToolResult {
        tool_call_id: "call_2".to_string(),
        result: tool_result2,
    }];
    let actions = fsm.run(&mut inputs).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::SendToAi { .. }));
}

#[test]
fn test_unknown_tool_call_result() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Use tool");

    let mut inputs = vec![
        ToolLoopInput::InsertMessage { message: msg },
        ToolLoopInput::Run,
    ];
    fsm.run(&mut inputs).unwrap();

    let tool_call = create_test_tool_call("call_1", "test_tool");
    let mut inputs = vec![ToolLoopInput::ProvideAiResponse {
        content: Some("Using tool".to_string()),
        thinking_content: None,
        tool_calls: vec![tool_call],
        finish_reason: "tool_calls".to_string(),
    }];
    fsm.run(&mut inputs).unwrap();

    // Provide result for unknown tool call
    let tool_result = create_test_tool_result("unknown_call", "Result");
    let mut inputs = vec![ToolLoopInput::ProvideToolResult {
        tool_call_id: "unknown_call".to_string(),
        result: tool_result,
    }];
    let result = fsm.run(&mut inputs);

    assert!(matches!(result, Err(FsmError::UnknownToolCall { .. })));
}

#[test]
fn test_pause_during_ai_call() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Hello");

    let mut inputs = vec![
        ToolLoopInput::InsertMessage { message: msg },
        ToolLoopInput::Run,
    ];
    fsm.run(&mut inputs).unwrap();

    let mut inputs = vec![ToolLoopInput::Pause];
    let actions = fsm.run(&mut inputs).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::Paused));
    assert!(matches!(fsm.state(), State::Paused { .. }));
}

#[test]
fn test_pause_during_tool_execution() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Use tool");

    let mut inputs = vec![
        ToolLoopInput::InsertMessage { message: msg },
        ToolLoopInput::Run,
    ];
    fsm.run(&mut inputs).unwrap();

    let tool_call = create_test_tool_call("call_1", "test_tool");
    let mut inputs = vec![ToolLoopInput::ProvideAiResponse {
        content: Some("Using tool".to_string()),
        thinking_content: None,
        tool_calls: vec![tool_call],
        finish_reason: "tool_calls".to_string(),
    }];
    fsm.run(&mut inputs).unwrap();

    let mut inputs = vec![ToolLoopInput::Pause];
    let actions = fsm.run(&mut inputs).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::Paused));
    assert!(matches!(fsm.state(), State::Paused { .. }));
}

#[test]
fn test_resume_from_pause() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Hello");

    let mut inputs = vec![
        ToolLoopInput::InsertMessage { message: msg },
        ToolLoopInput::Run,
        ToolLoopInput::Pause,
    ];
    fsm.run(&mut inputs).unwrap();

    let mut inputs = vec![ToolLoopInput::Resume];
    let actions = fsm.run(&mut inputs).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::SendToAi { .. }));
    assert!(matches!(fsm.state(), State::AwaitingAiResponse { .. }));
}

#[test]
fn test_abort_from_idle() {
    let mut fsm = ToolLoopFsm::new();

    let mut inputs = vec![ToolLoopInput::Abort];
    let actions = fsm.run(&mut inputs).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::Aborted));
    assert!(matches!(fsm.state(), State::Aborted));
    assert!(fsm.is_finished());
}

#[test]
fn test_abort_during_ai_call() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Hello");

    let mut inputs = vec![
        ToolLoopInput::InsertMessage { message: msg },
        ToolLoopInput::Run,
    ];
    fsm.run(&mut inputs).unwrap();

    let mut inputs = vec![ToolLoopInput::Abort];
    let actions = fsm.run(&mut inputs).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::Aborted));
    assert!(fsm.is_finished());
}

#[test]
fn test_input_after_finished() {
    let mut fsm = ToolLoopFsm::new();
    let msg = create_test_message(1, "user", "Hello");

    let mut inputs = vec![
        ToolLoopInput::InsertMessage { message: msg },
        ToolLoopInput::Run,
    ];
    fsm.run(&mut inputs).unwrap();

    let mut inputs = vec![ToolLoopInput::ProvideAiResponse {
        content: Some("Hi!".to_string()),
        thinking_content: None,
        tool_calls: vec![],
        finish_reason: "stop".to_string(),
    }];
    fsm.run(&mut inputs).unwrap();

    // Try to provide another input after completion
    let mut inputs = vec![ToolLoopInput::Run];
    let result = fsm.run(&mut inputs);

    assert!(matches!(result, Err(FsmError::FsmFinished { .. })));
}

#[test]
fn test_invalid_transition() {
    let mut fsm = ToolLoopFsm::new();

    // Try to provide AI response without running first
    let mut inputs = vec![ToolLoopInput::ProvideAiResponse {
        content: Some("Hi!".to_string()),
        thinking_content: None,
        tool_calls: vec![],
        finish_reason: "stop".to_string(),
    }];
    let result = fsm.run(&mut inputs);

    assert!(matches!(result, Err(FsmError::UnexpectedInput { .. })));
}

#[test]
fn test_complete_tool_loop_scenario() {
    let mut fsm = ToolLoopFsm::new();

    // Setup
    let user_msg = create_test_message(1, "user", "What's the weather?");
    let tool = create_test_tool("get_weather");

    let mut inputs = vec![
        ToolLoopInput::InsertMessage { message: user_msg },
        ToolLoopInput::AddTool { tool },
    ];
    fsm.run(&mut inputs).unwrap();

    // Start tool loop
    let mut inputs = vec![ToolLoopInput::Run];
    let actions = fsm.run(&mut inputs).unwrap();
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::SendToAi { .. }));

    // AI responds with tool call
    let tool_call = create_test_tool_call("call_1", "get_weather");
    let mut inputs = vec![ToolLoopInput::ProvideAiResponse {
        content: Some("Let me check the weather".to_string()),
        thinking_content: None,
        tool_calls: vec![tool_call],
        finish_reason: "tool_calls".to_string(),
    }];
    let actions = fsm.run(&mut inputs).unwrap();
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::ExecuteToolCall { .. }));

    // Tool result
    let tool_result = create_test_tool_result("call_1", "Sunny, 25°C");
    let mut inputs = vec![ToolLoopInput::ProvideToolResult {
        tool_call_id: "call_1".to_string(),
        result: tool_result,
    }];
    let actions = fsm.run(&mut inputs).unwrap();
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::SendToAi { .. }));

    // AI provides final response
    let mut inputs = vec![ToolLoopInput::ProvideAiResponse {
        content: Some("The weather is sunny and 25°C".to_string()),
        thinking_content: None,
        tool_calls: vec![],
        finish_reason: "stop".to_string(),
    }];
    let actions = fsm.run(&mut inputs).unwrap();
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], ToolLoopAction::Completed { .. }));
    assert!(fsm.is_finished());

    // Verify messages were added correctly
    assert_eq!(fsm.messages().len(), 4); // user, assistant with tool call, tool result, final assistant
}
