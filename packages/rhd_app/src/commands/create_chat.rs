//! Create chat command

use rhd_chat_client::ChatClient;
use rhd_chat_api::CreateChatParams;

pub async fn execute(title: String) -> Result<(), Box<dyn std::error::Error>> {
    let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
    let result = client.create_chat(CreateChatParams {
        title,
        tags: vec![],
    }).await?;
    
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
