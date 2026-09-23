//! Content-routed mock AI listener for the full-pipeline e2e suite
//! (Phase 7). `MockAiListener` is a public trait implemented here because the
//! shipped FIFO listeners (`SimpleListener` / `RecordingListener` /
//! `DynamicListener`) interleave nondeterministically once two or more chats
//! run concurrent tool loops.
//!
//! # Matcher discipline (plan note 5: "marker design discipline")
//!
//! One provider serves every chat, so each chat is identified by a unique
//! marker embedded in its seed / user message text (`"E2E-TASK-A1"`, …). The
//! full chat history is replayed on every request, which makes markers
//! cumulative down the lineage: a parent's *second* request contains the
//! child's marker inside the assistant tool-call arguments. The rule that
//! keeps routing deterministic is therefore:
//!
//! 1. **Register routes in lineage order, ancestor first.** A request is
//!    routed to the FIRST registered route whose marker occurs anywhere in
//!    the serialized `request.messages` (contents, tool-call arguments,
//!    tool results). A request containing both the parent's and the child's
//!    marker belongs to the parent (the child's text only ever enters a
//!    parent's history through the spawn arguments / answer); a request
//!    containing only the child's marker can only come from the child.
//! 2. Every marker is unique per test and appears in exactly one chat's seed.
//!
//! A request that matches **no** route is answered `text("unexpected
//! request")` (loud, non-stalling — assertions on chat contents then fail
//! visibly). A matched route with an exhausted queue **holds the request**
//! until the test pushes the next response, which is what makes
//! id-dependent turns race-free: the plugin cannot proceed past a request
//! the test has not scripted yet (the outer `tokio::time::timeout` of each
//! test turns a forgotten push into a failure).
//!
//! All responses must be **SSE shapes** (`stream_*` / held streams /
//! `Error`): `rhd_plugin_ai_completions` always calls
//! `chat_completion_stream`, and a JSON-body `Completion` would parse to
//! zero chunks. The library's `MockAiResponse::tool_call` hardcodes
//! `id = "call_1"`, which collides across sequential turns of one chat
//! (answers dedup by tool-call id), so [`streamed_tool_call`] builds
//! id-carrying stream responses via the public `StreamBuilder` instead.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use rhd_mock_ai_provider::rhd_ai_client::{
    ChatCompletionRequest, FunctionCallDelta, StreamChunk, ToolCallDelta,
};
use rhd_mock_ai_provider::{MockAiListener, MockAiResponse, StreamBuilder, StreamController};

/// Route key recorded for requests that matched no marker.
pub const FALLBACK: &str = "fallback";

/// One scripted conversation lane: an ordered queue of canned responses,
/// plus the wake channel that unblocks held requests when the test pushes.
struct Route {
    marker: String,
    queue: Mutex<VecDeque<MockAiResponse>>,
    wake: tokio::sync::watch::Sender<u64>,
}

/// Cloneable handle (the mock server boxes it behind `Arc<dyn MockAiListener>`
/// while tests keep their own clone to script routes).
#[derive(Clone, Default)]
pub struct RoutingListener {
    routes: Arc<Mutex<Vec<Arc<Route>>>>,
    requests: Arc<Mutex<Vec<ChatCompletionRequest>>>,
    /// Route marker (or [`FALLBACK`]) per recorded request, same order.
    routed: Arc<Mutex<Vec<String>>>,
}

impl RoutingListener {
    pub fn new() -> Self {
        Self::default()
    }

    /// Script `responses` for `marker`, in service order. Re-registering an
    /// existing marker appends (route priority is the first registration).
    pub fn register(&self, marker: &str, responses: Vec<MockAiResponse>) {
        let existing = {
            let routes = self.routes.lock().unwrap();
            routes.iter().find(|r| r.marker == marker).cloned()
        };
        let route = existing.unwrap_or_else(|| {
            let (wake, _rx) = tokio::sync::watch::channel(0);
            let route = Arc::new(Route {
                marker: marker.to_string(),
                queue: Mutex::new(VecDeque::new()),
                wake,
            });
            self.routes.lock().unwrap().push(Arc::clone(&route));
            route
        });
        route.queue.lock().unwrap().extend(responses);
        route.wake.send_modify(|tick| *tick += 1);
    }

    /// Append a single response to `marker`'s lane, creating the route (at
    /// the END of the priority order) if it does not exist yet. Used for
    /// turns whose arguments carry chat ids only discovered mid-scenario.
    pub fn push(&self, marker: &str, response: MockAiResponse) {
        self.register(marker, vec![response]);
    }

    /// Snapshot of every recorded request, in arrival order.
    pub fn take_requests(&self) -> Vec<ChatCompletionRequest> {
        self.requests.lock().unwrap().clone()
    }

    /// Requests that were routed to `marker`'s lane.
    pub fn requests_for(&self, marker: &str) -> Vec<ChatCompletionRequest> {
        let requests = self.requests.lock().unwrap();
        let routed = self.routed.lock().unwrap();
        requests
            .iter()
            .zip(routed.iter())
            .filter(|(_, route)| *route == marker)
            .map(|(request, _)| request.clone())
            .collect()
    }

    /// How many requests `marker`'s lane has served — the "never received"
    /// and "exactly N provider round trips" assertions.
    pub fn count_for(&self, marker: &str) -> usize {
        self.routed.lock().unwrap().iter().filter(|route| route.as_str() == marker).count()
    }
}

impl MockAiListener for RoutingListener {
    fn on_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Pin<Box<dyn Future<Output = MockAiResponse> + Send>> {
        // Route on the serialized message list: covers user/system/assistant/
        // tool contents AND assistant tool-call arguments in one dumb
        // substring scan — no shape analysis needed with unique markers.
        let haystack = serde_json::to_string(&request.messages).unwrap_or_default();
        let route = {
            let routes = self.routes.lock().unwrap();
            routes.iter().find(|r| haystack.contains(&r.marker)).cloned()
        };
        self.requests.lock().unwrap().push(request.clone());
        self.routed.lock().unwrap().push(
            route.as_ref().map(|r| r.marker.clone()).unwrap_or_else(|| FALLBACK.to_string()),
        );

        match route {
            // Hold until a response is scripted (see module docs). The
            // subscribe-before-pop order makes a push racing the pop visible
            // to the next pop, never lost.
            Some(route) => Box::pin(async move {
                loop {
                    let mut wake_rx = route.wake.subscribe();
                    let popped = {
                        let mut queue = route.queue.lock().unwrap();
                        queue.pop_front()
                    };
                    if let Some(response) = popped {
                        return response;
                    }
                    if wake_rx.changed().await.is_err() {
                        return text_response("route listener shut down");
                    }
                }
            }),
            None => Box::pin(async move { text_response("unexpected request") }),
        }
    }
}

/// A plain (non-streaming-shape) text response is useless to the plugin —
/// wrap as a fully buffered single-chunk stream everywhere.
pub fn text_response(content: &str) -> MockAiResponse {
    MockAiResponse::Stream(
        StreamBuilder::new()
            .chunk(StreamChunk {
                content: Some(content.to_string()),
                ..Default::default()
            })
            .chunk(StreamChunk {
                finish_reason: Some("stop".to_string()),
                ..Default::default()
            })
            .build(),
    )
}

/// Streaming assistant turn carrying exactly one tool call with a caller-
/// chosen id (the library builders hardcode `call_1`, which collides across
/// sequential turns of one chat — see module docs).
pub fn streamed_tool_call(id: &str, name: &str, arguments: &str) -> MockAiResponse {
    MockAiResponse::Stream(
        StreamBuilder::new()
            .chunk(StreamChunk {
                tool_calls: Some(vec![ToolCallDelta {
                    index: 0,
                    id: Some(id.to_string()),
                    call_type: Some("function".to_string()),
                    function: Some(FunctionCallDelta {
                        name: Some(name.to_string()),
                        arguments: Some(arguments.to_string()),
                    }),
                }]),
                ..Default::default()
            })
            .chunk(StreamChunk {
                finish_reason: Some("tool_calls".to_string()),
                ..Default::default()
            })
            .build(),
    )
}

/// A stream that stays open (empty, no finish) until [`release_stream`]
/// pushes its text: models "the subchat is still answering" windows
/// deterministically, without sleeps.
pub fn held_stream() -> (StreamController, MockAiResponse) {
    let (controller, receiver) = StreamController::new();
    (controller, MockAiResponse::Stream(receiver))
}

/// Deliver `content` as one chunk, then the finish chunk.
pub async fn release_stream(controller: &StreamController, content: &str) {
    controller
        .send_text(content)
        .await
        .expect("held stream receiver dropped");
    controller
        .send_chunk(StreamChunk {
            finish_reason: Some("stop".to_string()),
            ..Default::default()
        })
        .await
        .expect("held stream receiver dropped");
}

/// HTTP-level provider failure (drives the `ai_completions:error` park).
pub fn provider_error(status: u16, message: &str) -> MockAiResponse {
    MockAiResponse::Error {
        status,
        message: message.to_string(),
    }
}

/// `rhd_sub_chat` arguments: user-role seed `tasks`, background mode when
/// `spawn_async`.
pub fn spawn_arguments(tasks: &[String], spawn_async: bool) -> String {
    let messages: Vec<_> = tasks
        .iter()
        .map(|task| serde_json::json!({ "role": "user", "content": task }))
        .collect();
    let mut arguments = serde_json::json!({ "messages": messages });
    if spawn_async {
        arguments["async"] = serde_json::json!(true);
    }
    arguments.to_string()
}

/// `rhd_sub_chat_status` / `rhd_sub_chat_await` arguments.
pub fn target_arguments(chat_id: i64) -> String {
    format!(r#"{{"chatId":{chat_id}}}"#)
}
