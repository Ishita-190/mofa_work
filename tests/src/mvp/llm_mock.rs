use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{Json, Router, extract::State, routing::post};
use serde::{Deserialize, Serialize};

use crate::mvp::runner::LlmBackend;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub test_id: Option<String>,
    pub turn: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PresetKey {
    test_id: String,
    turn: u32,
}

#[derive(Debug, Clone)]
pub struct MockLlmServer {
    preset_responses: Arc<Mutex<HashMap<PresetKey, String>>>,
    fallback_response: Arc<Mutex<String>>,
    requests: Arc<Mutex<Vec<ChatCompletionRequest>>>,
    fixed_timestamp: u64,
}

impl Default for MockLlmServer {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLlmServer {
    pub fn new() -> Self {
        Self {
            preset_responses: Arc::new(Mutex::new(HashMap::new())),
            fallback_response: Arc::new(Mutex::new("mock-default-response".to_string())),
            requests: Arc::new(Mutex::new(Vec::new())),
            fixed_timestamp: 1,
        }
    }

    pub fn with_fallback_response(self, fallback_response: impl Into<String>) -> Self {
        if let Ok(mut lock) = self.fallback_response.lock() {
            *lock = fallback_response.into();
        }
        self
    }

    pub fn with_fixed_timestamp(mut self, fixed_timestamp: u64) -> Self {
        self.fixed_timestamp = fixed_timestamp;
        self
    }

    pub fn register_preset(
        &self,
        test_id: impl Into<String>,
        turn: u32,
        response: impl Into<String>,
    ) {
        let key = PresetKey {
            test_id: test_id.into(),
            turn,
        };

        if let Ok(mut lock) = self.preset_responses.lock() {
            lock.insert(key, response.into());
        }
    }

    pub fn requests(&self) -> Vec<ChatCompletionRequest> {
        self.requests
            .lock()
            .expect("requests mutex should not be poisoned")
            .clone()
    }

    pub fn generate(&self, request: ChatCompletionRequest) -> ChatCompletionResponse {
        if let Ok(mut lock) = self.requests.lock() {
            lock.push(request.clone());
        }

        let key = PresetKey {
            test_id: request
                .test_id
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            turn: request.turn.unwrap_or(0),
        };

        let content = self
            .preset_responses
            .lock()
            .ok()
            .and_then(|lock| lock.get(&key).cloned())
            .or_else(|| self.fallback_response.lock().ok().map(|lock| lock.clone()))
            .unwrap_or_else(|| "mock-default-response".to_string());

        ChatCompletionResponse {
            id: "chatcmpl-mock".to_string(),
            object: "chat.completion".to_string(),
            created: self.fixed_timestamp,
            model: request.model,
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content,
                },
                finish_reason: "stop".to_string(),
            }],
        }
    }

    pub fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/v1/chat/completions", post(chat_completion_handler))
            .with_state(self)
    }
}

impl LlmBackend for MockLlmServer {
    fn complete(&self, request: &ChatCompletionRequest) -> ChatCompletionResponse {
        self.generate(request.clone())
    }
}

async fn chat_completion_handler(
    State(server): State<Arc<MockLlmServer>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Json<ChatCompletionResponse> {
    Json(server.generate(request))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn returns_preset_response_by_test_id_and_turn() {
        let server = MockLlmServer::new().with_fallback_response("fallback");
        server.register_preset("t1", 0, "preset-response");

        let response = server.generate(ChatCompletionRequest {
            model: "mock-model".to_string(),
            messages: vec![],
            test_id: Some("t1".to_string()),
            turn: Some(0),
        });

        assert_eq!(response.choices[0].message.content, "preset-response");
    }

    #[test]
    fn returns_fallback_when_no_preset_exists() {
        let server = MockLlmServer::new().with_fallback_response("fallback");

        let response = server.generate(ChatCompletionRequest {
            model: "mock-model".to_string(),
            messages: vec![],
            test_id: Some("missing".to_string()),
            turn: Some(9),
        });

        assert_eq!(response.choices[0].message.content, "fallback");
    }

    #[test]
    fn records_requests() {
        let server = MockLlmServer::new();

        server.generate(ChatCompletionRequest {
            model: "mock-model".to_string(),
            messages: vec![],
            test_id: Some("t1".to_string()),
            turn: Some(0),
        });

        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].test_id.as_deref(), Some("t1"));
    }

    #[tokio::test]
    async fn http_endpoint_returns_fallback_response() {
        let server = Arc::new(MockLlmServer::new().with_fallback_response("http-fallback"));
        let app = server.clone().router();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("local addr should be available");

        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let response = reqwest::Client::new()
            .post(format!("http://{addr}/v1/chat/completions"))
            .json(&serde_json::json!({
                "model": "mock-model",
                "messages": [],
                "test_id": "missing",
                "turn": 0
            }))
            .send()
            .await
            .expect("request should succeed")
            .json::<serde_json::Value>()
            .await
            .expect("response should parse as json");

        assert_eq!(response["choices"][0]["message"]["content"], "http-fallback");
    }
}
