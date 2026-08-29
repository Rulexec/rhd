//! Create chat command

use rhd_chat_client::ChatClient;
use rhd_chat_api::CreateChatParams;

pub async fn execute(title: String, tags: Option<Vec<String>>) -> Result<(), Box<dyn std::error::Error>> {
    let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
    let result = client.create_chat(CreateChatParams {
        title,
        tags: tags.unwrap_or_default(),
    }).await?;
    
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
