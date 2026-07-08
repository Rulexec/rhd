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
use crate::state::StreamState;
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

    pub async fn abort_chat(&self, chat_id: i64) -> bool {
        let active = self.active_streams.lock().await;
        if let Some(state) = active.get(&chat_id) {
            state.cancel_token().cancel();
            true
        } else {
            false
        }
    }

    pub async fn pause_chat(&self, chat_id: i64) -> bool {
        let mut active = self.active_streams.lock().await;
        if let Some(StreamState::Running { cancel_token, pause_notify }) = active.get(&chat_id) {
            let cancel_token = cancel_token.clone();
            let pause_notify = pause_notify.clone();
            active.insert(
                chat_id,
                StreamState::Paused {
                    cancel_token,
                    pause_notify,
                },
            );
            true
        } else {
            false
        }
    }

    pub async fn resume_chat(&self, chat_id: i64) -> bool {
        let mut active = self.active_streams.lock().await;
        if let Some(StreamState::Paused { cancel_token, pause_notify }) = active.get(&chat_id) {
            let cancel_token = cancel_token.clone();
            let pause_notify = pause_notify.clone();
            pause_notify.notify_one();
            active.insert(
                chat_id,
                StreamState::Running {
                    cancel_token,
                    pause_notify,
                },
            );
            true
        } else {
            false
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
        projects::detach_project(&self.db, chat_id, project_name, event_sender)
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
    ) -> Result<i64, ChatError> {
        stream::send_message(self, chat_id, content, model, models, event_sender, reload_lock)
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
    ) -> Result<i64, ChatError> {
        stream::edit_and_resend(
            self,
            message_id,
            new_content,
            model,
            models,
            event_sender,
            reload_lock,
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
            },
        );
        (cancel_token, pause_notify)
    }

    pub(crate) async fn unregister_stream(&self, chat_id: i64) {
        let mut active = self.active_streams.lock().await;
        active.remove(&chat_id);
    }

    pub(crate) async fn check_pause_state(
        &self,
        chat_id: i64,
        event_sender: &broadcast::Sender<ChatEvent>,
    ) {
        let pause_notify = {
            let active = self.active_streams.lock().await;
            if let Some(StreamState::Paused { pause_notify, .. }) = active.get(&chat_id) {
                Some(pause_notify.clone())
            } else {
                None
            }
        };

        if let Some(notify) = pause_notify {
            let _ = event_sender.send(ChatEvent::ChatPaused { chat_id });
            notify.notified().await;
            let _ = event_sender.send(ChatEvent::ChatResumed { chat_id });
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
}
