//! Free polling helpers over [`FullEnv`]: every one is a `wait_for`-style
//! bounded poll (plan note 2) — never a fixed sleep for convergence. The
//! exactly-one-answer counting stays in the scenario files.

use rhd_chat_api::ListChatsParams;
use rhd_chat_client::ChatClient;

use super::full_env::FullEnv;

/// The subchat linked to `tool_call_id` via its `sub_chat:call:` tag.
/// Panics on the pathological duplicate-link state (one call, one chat).
pub async fn find_subchat(client: &ChatClient, tool_call_id: &str) -> Option<i64> {
    let link = rhd_plugin_sub_chat::tags::call_tag(tool_call_id);
    let chats = client
        .list_chats(ListChatsParams {
            tags: vec![link.clone()],
        })
        .await
        .expect("listChats failed")
        .chats;
    let mut matches = chats
        .into_iter()
        .filter(|c| c.tags.iter().any(|t| t == &link))
        .map(|c| c.id);
    let first = matches.next();
    assert!(
        matches.next().is_none(),
        "exactly one subchat may carry link tag {link}"
    );
    first
}

/// Poll until the subchat for `tool_call_id` exists; return its id.
pub async fn wait_subchat(env: &FullEnv, tool_call_id: &str) -> i64 {
    let client = env.client.as_ref().clone();
    let call_id = tool_call_id.to_string();
    env.wait_for(&format!("subchat for {call_id}"), move || {
        let client = client.clone();
        let call_id = call_id.clone();
        Box::pin(async move { find_subchat(&client, &call_id).await.is_some() })
    })
    .await;
    find_subchat(&env.client, tool_call_id)
        .await
        .expect("subchat just observed")
}

/// Poll until `chat_id`'s history ends with a finished assistant message
/// whose content is byte-identical to `content`.
pub async fn wait_final_assistant(env: &FullEnv, chat_id: i64, content: &str) {
    let client = env.client.as_ref().clone();
    let content = content.to_string();
    env.wait_for(&format!("chat {chat_id} final assistant {content:?}"), move || {
        let client = client.clone();
        let content = content.clone();
        Box::pin(async move {
            let messages = super::get_chat(&client, chat_id).await.messages;
            super::final_assistant_message(&messages).is_some_and(|m| m.content == content)
        })
    })
    .await;
}

/// Poll until `chat_id` carries a finished assistant message declaring
/// `tool_call_id` (the parked-loop state a sync spawn / await leaves behind).
pub async fn wait_assistant_call(env: &FullEnv, chat_id: i64, tool_call_id: &str) {
    let client = env.client.as_ref().clone();
    let call_id = tool_call_id.to_string();
    env.wait_for(&format!("chat {chat_id} assistant call {call_id}"), move || {
        let client = client.clone();
        let call_id = call_id.clone();
        Box::pin(async move {
            let messages = super::get_chat(&client, chat_id).await.messages;
            super::history_has_assistant_call(&messages, &call_id)
        })
    })
    .await;
}

/// Poll until `chat_id`'s routed-request count for `marker` reaches `expected`.
pub async fn wait_for_count(env: &FullEnv, marker: &str, expected: usize) {
    let listener = env.listener.clone();
    let marker = marker.to_string();
    env.wait_for(
        &format!("{expected} requests routed to {marker}"),
        move || {
            let listener = listener.clone();
            let marker = marker.clone();
            Box::pin(async move { listener.count_for(&marker) == expected })
        },
    )
    .await;
}

/// Poll until `chat_id` carries (or no longer carries, `want=false`) `tag`.
pub async fn wait_chat_tag(env: &FullEnv, chat_id: i64, tag: &str, want: bool) {
    let client = env.client.as_ref().clone();
    let tag = tag.to_string();
    env.wait_for(
        &format!("chat {chat_id} tag {tag} present={want}"),
        move || {
            let client = client.clone();
            let tag = tag.clone();
            Box::pin(async move {
                super::get_chat(&client, chat_id)
                    .await
                    .chat
                    .tags
                    .iter()
                    .any(|t| *t == tag)
                    == want
            })
        },
    )
    .await;
}

/// Poll until every listed tool call has a persisted answer (callers
/// assert the exactly-one count separately).
pub async fn wait_answers(env: &FullEnv, chat_id: i64, tool_call_ids: &[&str]) {
    let client = env.client.as_ref().clone();
    let ids: Vec<String> = tool_call_ids.iter().map(|id| id.to_string()).collect();
    env.wait_for(&format!("chat {chat_id} answers for {ids:?}"), move || {
        let client = client.clone();
        let ids = ids.clone();
        Box::pin(async move {
            let chat = super::get_chat(&client, chat_id).await;
            ids.iter().all(|id| {
                chat.messages
                    .iter()
                    .any(|m| m.tool_call_id.as_deref() == Some(id.as_str()))
            })
        })
    })
    .await;
}
