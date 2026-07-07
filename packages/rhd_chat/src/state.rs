use std::sync::Arc;

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub enum StreamState {
    Running {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
    },
    Paused {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
    },
}

impl StreamState {
    pub fn cancel_token(&self) -> &CancellationToken {
        match self {
            StreamState::Running { cancel_token, .. } => cancel_token,
            StreamState::Paused { cancel_token, .. } => cancel_token,
        }
    }
}
