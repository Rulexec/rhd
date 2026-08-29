//! Queue operations command

use rhd_chat_client::ChatClient;
use rhd_chat_api::{GetQueueMessagesParams, AddQueueMessageParams};
use crate::cli::QueueSubcommand;

pub async fn execute(subcommand: QueueSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
    
    match subcommand {
        QueueSubcommand::List { chat_id } => {
            let result = client.get_queue_messages(GetQueueMessagesParams { chat_id }).await?;
            println!("{}", serde_json::to_string_pretty(&result.messages)?);
            Ok(())
        }
        QueueSubcommand::Add { chat_id, content } => {
            let result = client.add_queue_message(AddQueueMessageParams {
                chat_id,
                role: "user".to_string(),
                content,
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
            }).await?;
            println!("Queued message with ID: {}", result.message_id);
            Ok(())
        }
    }
}
