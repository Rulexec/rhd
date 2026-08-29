//! Chat commands

use rhd_chat_client::ChatClient;
use rhd_chat_api::{ListChatsParams, UpdateChatParams};
use crate::cli::ChatsSubcommand;

pub async fn execute(subcommand: ChatsSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    match subcommand {
        ChatsSubcommand::List => {
            let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
            let result = client.list_chats(ListChatsParams { tags: vec![] }).await?;
            
            println!("{}", serde_json::to_string_pretty(&result.chats)?);
            Ok(())
        }
        ChatsSubcommand::AddTag { chat_id, tags } => {
            let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
            client.update_chat(UpdateChatParams {
                chat_id,
                title: None,
                add_tags: tags,
                remove_tags: vec![],
            }).await?;
            println!("Tags added successfully");
            Ok(())
        }
        ChatsSubcommand::RemoveTag { chat_id, tags } => {
            let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
            client.update_chat(UpdateChatParams {
                chat_id,
                title: None,
                add_tags: vec![],
                remove_tags: tags,
            }).await?;
            println!("Tags removed successfully");
            Ok(())
        }
    }
}
