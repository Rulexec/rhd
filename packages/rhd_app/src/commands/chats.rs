//! Chat list command

use rhd_chat_client::ChatClient;
use rhd_chat_api::ListChatsParams;
use crate::cli::ChatsSubcommand;

pub async fn execute(subcommand: ChatsSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    match subcommand {
        ChatsSubcommand::List => {
            let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
            let result = client.list_chats(ListChatsParams { tags: vec![] }).await?;
            
            println!("{}", serde_json::to_string_pretty(&result.chats)?);
            Ok(())
        }
    }
}
