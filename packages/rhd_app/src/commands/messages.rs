//! Messages view command

use rhd_chat_client::ChatClient;
use rhd_chat_api::GetChatParams;

pub async fn execute(chat_id: i64, all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
    let result = client.get_chat(GetChatParams { chat_id, if_version_higher_than: None }).await?;
    
    let messages = if all {
        result.messages
    } else {
        // Show only last message
        result.messages.into_iter().last().into_iter().collect()
    };
    
    println!("{}", serde_json::to_string_pretty(&messages)?);
    Ok(())
}
