use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rhd_ai::config::ModelConfig;
use rhd_api::project::ProjectInfo;
use rhd_db::{ChatDb, ChatInfo, Message};
use tokio::sync::{broadcast, Mutex, Notify};
use tokio_util::sync::CancellationToken;

use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::projects;
use crate::state::{ExecutionPhase, MessageQueue, PendingToolCall, QueuedMessage, StreamState, StreamStateInfo};
use crate::stream;
use crate::ProjectProvider;

pub struct ChatManager<P: ProjectProvider> {
    db: Arc<ChatDb>,
    active_streams: Mutex<HashMap<i64, StreamState>>,
    project_provider: Arc<P>,
    log_chats: Option<PathBuf>,
    log_chats_raw: bool,
}

impl<P: ProjectProvider> ChatManager<P> {
    pub fn new(
        db: Arc<ChatDb>,
        project_provider: Arc<P>,
        log_chats: Option<PathBuf>,
        log_chats_raw: bool,
    ) -> Self {
        Self {
            db,
            active_streams: Mutex::new(HashMap::new()),
            project_provider,
            log_chats,
            log_chats_raw,
        }
    }

    pub fn db(&self) -> &Arc<ChatDb> {
        &self.db
    }

    pub fn project_provider(&self) -> &Arc<P> {
        &self.project_provider
    }

    pub fn log_chats(&self) -> &Option<PathBuf> {
        &self.log_chats
    }

    pub fn log_chats_raw(&self) -> bool {
        self.log_chats_raw
    }

    /// Adds a message to the database and emits a MessageAdded event.
    /// This is the single point of message addition to ensure consistency.
    pub fn add_message_and_notify(
        &self,
        chat_id: i64,
        role: &str,
        content: &str,
        model: Option<&str>,
        thinking_content: Option<&str>,
        event_sender: &broadcast::Sender<ChatEvent>,
    ) -> Result<Message, ChatError> {
        let message_id = self.db.add_message(chat_id, role, content, model, thinking_content)?;
        let message = Message {
            id: message_id,
            chat_id,
            role: role.to_string(),
            content: content.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            model: model.map(|s| s.to_string()),
            thinking_content: thinking_content.map(|s| s.to_string()),
            tool_calls: None,
        };
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: message.clone(),
        });
        Ok(message)
    }

    pub fn create_chat(&self, title: &str) -> Result<i64, ChatError> {
        Ok(self.db.create_chat(title)?)
    }

    pub fn list_chats(&self) -> Result<Vec<ChatInfo>, ChatError> {
        Ok(self.db.list_chats()?)
    }

    pub fn get_chat(&self, id: i64) -> Result<Option<(ChatInfo, Vec<Message>)>, ChatError> {
        let chat = self.db.get_chat(id)?;
        match chat {
            Some(chat_info) => {
                let messages = self.db.get_messages(id)?;
                Ok(Some((chat_info, messages)))
            }
            None => Ok(None),
        }
    }

    pub fn delete_chat(&self, id: i64) -> Result<(), ChatError> {
        self.db.delete_chat(id)?;
        Ok(())
    }

    pub async fn delete_all_chats(&self) -> Result<(), ChatError> {
        self.db.delete_all_chats()?;
        let mut active = self.active_streams.lock().await;
        for (_, state) in active.drain() {
            state.cancel_token().cancel();
        }
        Ok(())
    }

    pub async fn abort_chat(&self, chat_id: i64, aborted_tool_ids: Vec<String>) -> bool {
        let mut active = self.active_streams.lock().await;
        if let Some(state) = active.get(&chat_id) {
            let cancel_token = state.cancel_token().clone();
            cancel_token.cancel();
            active.insert(
                chat_id,
                StreamState::Aborted {
                    cancel_token,
                    aborted_tool_ids,
                    message_queue: MessageQueue::new(),
                },
            );
            true
        } else {
            false
        }
    }

    pub async fn pause_chat(
        &self,
        chat_id: i64,
    ) -> bool {
        let mut active = self.active_streams.lock().await;
        if let Some(StreamState::Running { cancel_token, pause_notify, phase }) = active.get(&chat_id) {
            let cancel_token = cancel_token.clone();
            let pause_notify = pause_notify.clone();
            let phase = phase.clone();
            active.insert(
                chat_id,
                StreamState::Paused {
                    cancel_token,
                    pause_notify,
                    phase,
                    message_queue: MessageQueue::new(),
                },
            );
            true
        } else {
            false
        }
    }

    pub async fn resume_chat(&self, chat_id: i64) -> Option<ResumeInfo> {
        let mut active = self.active_streams.lock().await;

        let resume_data = if let Some(StreamState::Paused { cancel_token, pause_notify, phase, message_queue }) = active.get(&chat_id) {
            Some((cancel_token.clone(), pause_notify.clone(), phase.clone(), message_queue.clone(), true))
        } else if let Some(StreamState::Aborted { cancel_token, message_queue, .. }) = active.get(&chat_id) {
            let cancel_token = cancel_token.clone();
            let pause_notify = Arc::new(Notify::new());
            let message_queue = message_queue.clone();
            Some((cancel_token, pause_notify, ExecutionPhase::AiCall, message_queue, false))
        } else {
            None
        };

        if let Some((cancel_token, pause_notify, phase, message_queue, was_paused)) = resume_data {
            if was_paused {
                pause_notify.notify_one();
            }
            active.insert(
                chat_id,
                StreamState::Running {
                    cancel_token,
                    pause_notify,
                    phase: ExecutionPhase::AiCall,
                },
            );
            Some(ResumeInfo {
                previous_phase: phase,
                message_queue,
            })
        } else {
            None
        }
    }

    pub async fn queue_message(
        &self,
        chat_id: i64,
        content: String,
        model: String,
        event_sender: &broadcast::Sender<ChatEvent>,
    ) -> Result<(), ChatError> {
        let mut active = self.active_streams.lock().await;
        
        if let Some(state) = active.get_mut(&chat_id) {
            if let Some(queue) = state.message_queue_mut() {
                let queued_message = QueuedMessage {
                    content: content.clone(),
                    model: model.clone(),
                    queued_at: chrono::Utc::now(),
                };
                queue.push(queued_message);
                
                let _ = event_sender.send(ChatEvent::MessageQueued {
                    chat_id,
                    content,
                    model,
                });
                
                Ok(())
            } else {
                Err(ChatError::InvalidState("Chat is not paused or aborted".to_string()))
            }
        } else {
            Err(ChatError::ChatNotFound)
        }
    }

    pub async fn process_queued_messages(
        &self,
        chat_id: i64,
        event_sender: &broadcast::Sender<ChatEvent>,
    ) -> Result<Vec<(String, String)>, ChatError> {
        let mut active = self.active_streams.lock().await;
        
        if let Some(state) = active.get_mut(&chat_id) {
            if let Some(queue) = state.message_queue_mut() {
                let messages = queue.drain();
                let result: Vec<(String, String)> = messages
                    .into_iter()
                    .map(|m| (m.content, m.model))
                    .collect();
                
                Ok(result)
            } else {
                Ok(Vec::new())
            }
        } else {
            Err(ChatError::ChatNotFound)
        }
    }

    pub async fn attach_project(
        &self,
        chat_id: i64,
        project_name: &str,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<(), ChatError> {
        projects::attach_project(
            &self.db,
            &self.project_provider,
            chat_id,
            project_name,
            event_sender,
        )
        .await
    }

    pub async fn detach_project(
        &self,
        chat_id: i64,
        project_name: &str,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<(), ChatError> {
        projects::detach_project(&self.db, &self.project_provider, chat_id, project_name, event_sender)
    }

    pub fn get_chat_projects(&self, chat_id: i64) -> Result<Vec<ProjectInfo>, ChatError> {
        projects::get_chat_projects(&self.db, &self.project_provider, chat_id)
    }

    pub async fn send_message(
        &self,
        chat_id: i64,
        content: String,
        model: &str,
        models: &HashMap<String, ModelConfig>,
        event_sender: broadcast::Sender<ChatEvent>,
        reload_lock: &tokio::sync::RwLock<()>,
        template_loader: &stream::TemplateLoaderRef,
    ) -> Result<i64, ChatError> {
        stream::send_message(self, chat_id, content, model, models, event_sender, reload_lock, template_loader)
            .await
    }

    pub async fn edit_and_resend(
        &self,
        message_id: i64,
        new_content: String,
        model: &str,
        models: &HashMap<String, ModelConfig>,
        event_sender: broadcast::Sender<ChatEvent>,
        reload_lock: &tokio::sync::RwLock<()>,
        template_loader: &stream::TemplateLoaderRef,
    ) -> Result<i64, ChatError> {
        stream::edit_and_resend(
            self,
            message_id,
            new_content,
            model,
            models,
            event_sender,
            reload_lock,
            template_loader,
        )
        .await
    }

    pub async fn resume_stream(
        &self,
        chat_id: i64,
        model: &str,
        models: &HashMap<String, ModelConfig>,
        event_sender: broadcast::Sender<ChatEvent>,
        reload_lock: &tokio::sync::RwLock<()>,
        template_loader: &stream::TemplateLoaderRef,
    ) -> Result<i64, ChatError> {
        stream::resume_stream(
            self,
            chat_id,
            model,
            models,
            event_sender,
            reload_lock,
            template_loader,
        )
        .await
    }

    pub(crate) async fn register_stream(
        &self,
        chat_id: i64,
    ) -> (CancellationToken, Arc<Notify>) {
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());
        let mut active = self.active_streams.lock().await;
        if let Some(existing) = active.get(&chat_id) {
            existing.cancel_token().cancel();
        }
        active.insert(
            chat_id,
            StreamState::Running {
                cancel_token: cancel_token.clone(),
                pause_notify: pause_notify.clone(),
                phase: ExecutionPhase::AiCall,
            },
        );
        (cancel_token, pause_notify)
    }

    pub async fn unregister_stream(&self, chat_id: i64) {
        let mut active = self.active_streams.lock().await;
        active.remove(&chat_id);
    }

    pub async fn get_stream_state(&self, chat_id: i64) -> Option<StreamStateInfo> {
        let active = self.active_streams.lock().await;
        active.get(&chat_id).map(|state| StreamStateInfo::from(state))
    }

    pub async fn is_aborted(&self, chat_id: i64) -> bool {
        let active = self.active_streams.lock().await;
        active.get(&chat_id).map(|s| s.is_aborted()).unwrap_or(false)
    }

    pub async fn set_execution_phase(&self, chat_id: i64, phase: ExecutionPhase) {
        let mut active = self.active_streams.lock().await;
        if let Some(StreamState::Running { cancel_token, pause_notify, .. }) = active.get(&chat_id) {
            let cancel_token = cancel_token.clone();
            let pause_notify = pause_notify.clone();
            active.insert(
                chat_id,
                StreamState::Running {
                    cancel_token,
                    pause_notify,
                    phase,
                },
            );
        }
    }

    pub(crate) async fn get_paused_notify(&self, chat_id: i64) -> Option<Arc<Notify>> {
        let active = self.active_streams.lock().await;
        if let Some(StreamState::Paused { pause_notify, .. }) = active.get(&chat_id) {
            Some(pause_notify.clone())
        } else {
            None
        }
    }

    pub async fn set_active_role(
        &self,
        chat_id: i64,
        project_name: &str,
        role_name: &str,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<(), ChatError> {
        if self.db.get_chat(chat_id)?.is_none() {
            return Err(ChatError::ChatNotFound);
        }

        if self
            .project_provider
            .get_role_system_prompt(project_name, role_name)
            .is_none()
        {
            return Err(ChatError::RoleNotFound(format!(
                "{}:{}",
                project_name, role_name
            )));
        }

        self.db.set_active_role(chat_id, project_name, role_name)?;
        self.db.set_role_prompt_pending(chat_id, true)?;

        let _ = event_sender.send(ChatEvent::RoleChanged {
            chat_id,
            project_name: project_name.to_string(),
            role_name: role_name.to_string(),
        });

        Ok(())
    }

    pub fn clear_active_role(
        &self,
        chat_id: i64,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<(), ChatError> {
        if self.db.get_chat(chat_id)?.is_none() {
            return Err(ChatError::ChatNotFound);
        }
        self.db.clear_active_role(chat_id)?;
        self.db.reset_roles_list_injected(chat_id)?;

        let _ = event_sender.send(ChatEvent::ActiveRoleCleared { chat_id });

        Ok(())
    }

    pub fn get_active_role(&self, chat_id: i64) -> Result<Option<(String, String)>, ChatError> {
        Ok(self.db.get_active_role(chat_id)?)
    }

    pub fn get_available_roles(
        &self,
        chat_id: i64,
    ) -> Result<Vec<(String, String, String)>, ChatError> {
        if self.db.get_chat(chat_id)?.is_none() {
            return Err(ChatError::ChatNotFound);
        }

        let attached_projects = self.db.get_chat_projects(chat_id)?;
        let mut roles = Vec::new();

        for (project_name, _) in attached_projects {
            for role in self.project_provider.get_project_roles(&project_name) {
                roles.push((project_name.clone(), role.name, role.when_to_use));
            }
        }

        Ok(roles)
    }
}

#[derive(Debug, Clone)]
pub struct ResumeInfo {
    pub previous_phase: ExecutionPhase,
    pub message_queue: MessageQueue,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ExecutionPhase;
    use rhd_api::project::{McpRef, Role};
    use rhd_mcp_client::client::McpClient;
    use crate::{McpStatus, ProjectProvider};

    struct MockProjectProvider;

    #[async_trait::async_trait]
    impl ProjectProvider for MockProjectProvider {
        async fn get_mcp_status(&self, _project_name: &str) -> Vec<(String, McpStatus)> {
            vec![]
        }

        fn get_project_system_prompt(&self, _project_name: &str) -> Option<String> {
            None
        }

        fn get_project_mcp_refs(&self, _project_name: &str) -> Vec<McpRef> {
            vec![]
        }

        async fn get_mcp_clients(&self, _project_name: &str) -> Vec<(String, Arc<McpClient>)> {
            vec![]
        }

        async fn spawn_project_mcp(&self, _project_name: &str) -> Result<(), String> {
            Ok(())
        }

        fn get_project_roles(&self, _project_name: &str) -> Vec<Role> {
            vec![]
        }

        fn get_role_system_prompt(&self, _project_name: &str, _role_name: &str) -> Option<String> {
            None
        }
    }

    #[tokio::test]
    async fn test_pause_chat() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider);
        let manager = ChatManager::new(db, project_provider, None, false);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (_cancel_token, _) = manager.register_stream(chat_id).await;

        let result = manager.pause_chat(chat_id).await;
        assert!(result);

        let state = manager.get_stream_state(chat_id).await.unwrap();
        assert!(state.is_paused);
    }

    #[tokio::test]
    async fn test_abort_chat_transitions_to_aborted() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider);
        let manager = ChatManager::new(db, project_provider, None, false);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (cancel_token, _) = manager.register_stream(chat_id).await;

        let aborted_tools = vec!["call_1".to_string(), "call_2".to_string()];
        let result = manager.abort_chat(chat_id, aborted_tools.clone()).await;
        assert!(result);

        let state = manager.get_stream_state(chat_id).await.unwrap();
        assert!(state.is_aborted);
        assert_eq!(state.aborted_tool_ids, aborted_tools);
        assert!(cancel_token.is_cancelled());
    }

    #[tokio::test]
    async fn test_resume_chat() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider);
        let manager = ChatManager::new(db, project_provider, None, false);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (_cancel_token, _pause_notify) = manager.register_stream(chat_id).await;

        manager.pause_chat(chat_id).await;
        let result = manager.resume_chat(chat_id).await;
        assert!(result.is_some());

        let resume_info = result.unwrap();
        assert_eq!(resume_info.previous_phase, ExecutionPhase::AiCall);

        let state = manager.get_stream_state(chat_id).await.unwrap();
        assert!(state.is_running);
        assert!(!state.is_paused);
    }

    #[tokio::test]
    async fn test_queue_message_when_paused() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider);
        let manager = ChatManager::new(db, project_provider, None, false);
        let (event_sender, _) = broadcast::channel(100);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (_cancel_token, _) = manager.register_stream(chat_id).await;

        manager.pause_chat(chat_id).await;

        let result = manager.queue_message(
            chat_id,
            "Hello".to_string(),
            "gpt-4".to_string(),
            &event_sender,
        ).await;
        assert!(result.is_ok());

        let state = manager.get_stream_state(chat_id).await.unwrap();
        assert_eq!(state.queued_messages.len(), 1);
        assert_eq!(state.queued_messages[0].content, "Hello");
    }

    #[tokio::test]
    async fn test_queue_message_when_running_fails() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider);
        let manager = ChatManager::new(db, project_provider, None, false);
        let (event_sender, _) = broadcast::channel(100);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (_cancel_token, _) = manager.register_stream(chat_id).await;

        let result = manager.queue_message(
            chat_id,
            "Hello".to_string(),
            "gpt-4".to_string(),
            &event_sender,
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_process_queued_messages() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider);
        let manager = ChatManager::new(db, project_provider, None, false);
        let (event_sender, _) = broadcast::channel(100);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (_cancel_token, _) = manager.register_stream(chat_id).await;

        manager.pause_chat(chat_id).await;

        manager.queue_message(chat_id, "First".to_string(), "gpt-4".to_string(), &event_sender).await.unwrap();
        manager.queue_message(chat_id, "Second".to_string(), "gpt-4".to_string(), &event_sender).await.unwrap();

        let messages = manager.process_queued_messages(chat_id, &event_sender).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].0, "First");
        assert_eq!(messages[1].0, "Second");

        let state = manager.get_stream_state(chat_id).await.unwrap();
        assert_eq!(state.queued_messages.len(), 0);
    }

    #[tokio::test]
    async fn test_resume_preserves_message_queue() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider);
        let manager = ChatManager::new(db, project_provider, None, false);
        let (event_sender, _) = broadcast::channel(100);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (_cancel_token, _) = manager.register_stream(chat_id).await;

        manager.pause_chat(chat_id).await;

        manager.queue_message(chat_id, "Hello".to_string(), "gpt-4".to_string(), &event_sender).await.unwrap();

        let resume_info = manager.resume_chat(chat_id).await.unwrap();
        assert_eq!(resume_info.message_queue.len(), 1);
        assert_eq!(resume_info.message_queue.messages[0].content, "Hello");
    }
}
