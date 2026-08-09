mod builtin;
pub mod fsm_wrapper;
mod messages;
pub mod tool_loop;
mod utils;

#[cfg(test)]
mod tests;

pub use builtin::{
    RHD_SET_ROLE_TOOL_NAME, RHD_SET_TODO_LIST_TOOL_NAME, collect_builtin_tools,
    handle_rhd_set_role, handle_rhd_set_todo_list, inject_todo_list_message, parse_todo_list,
    rhd_set_role_tool_definition, rhd_set_todo_list_tool_definition,
};
pub use fsm_wrapper::FsmToolLoop;
pub use messages::{build_chat_messages, build_chat_messages_for_tools};
pub use tool_loop::{collect_tools_from_projects, execute_tool_call, tool_loop, ToolLoopResult};
pub use utils::{extract_mcp_id_from_tool_name, split_tool_name};
