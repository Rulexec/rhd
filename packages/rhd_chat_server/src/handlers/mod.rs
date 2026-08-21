//! Request handlers for the chat WebSocket protocol.

pub mod chat;
pub mod message;

use serde_json::Value;

use rhd_chat_api::protocol::Request;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;

/// Route a request to the appropriate handler.
pub async fn handle_request(
    request: Request,
    db: &ChatDb,
) -> Result<Value, ServerError> {
    let request_id = request.id.clone();
    
    match request.method.as_str() {
        // Chat methods
        "createChat" => chat::create_chat(request.params, db, &request_id).await,
        "listChats" => chat::list_chats(request.params, db, &request_id).await,
        "getChat" => chat::get_chat(request.params, db, &request_id).await,
        "deleteChat" => chat::delete_chat(request.params, db, &request_id).await,
        "updateChat" => chat::update_chat(request.params, db, &request_id).await,
        
        // Message methods
        "addMessage" => message::add_message(request.params, db, &request_id).await,
        "updateMessage" => message::update_message(request.params, db, &request_id).await,
        "deleteMessage" => message::delete_message(request.params, db, &request_id).await,
        
        // Unknown method
        _ => Ok(serde_json::to_value(ErrorResponse::invalid_request(
            request_id,
            format!("Unknown method: {}", request.method),
        ))?),
    }
}
