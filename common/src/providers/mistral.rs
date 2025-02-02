use serde::{Deserialize, Serialize};
use reqwest::Client as HttpClient;
use anyhow::Result;
use rig::completion::{CompletionModel, CompletionRequest, CompletionResponse, ModelChoice, PromptError, CompletionError};
use rig::agent::AgentBuilder;
use serde_json::json;

// Mistral AI Models
pub const MISTRAL_TINY: &str = "mistral-tiny";
pub const MISTRAL_SMALL: &str = "mistral-small-latest";
pub const MISTRAL_MEDIUM: &str = "mistral-medium";
pub const MISTRAL_LARGE: &str = "mistral-large-latest";

const API_URL: &str = "https://api.mistral.ai/v1";

#[derive(Clone)]
pub struct Client {
    pub base_url: String,
    pub api_key: String,
    http_client: HttpClient,
}

impl Client {
    pub fn new(api_key: &str) -> Self {
        Self {
            base_url: API_URL.to_string(),
            api_key: api_key.to_string(),
            http_client: HttpClient::new(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("MISTRAL_API_KEY")
            .expect("MISTRAL_API_KEY must be set");
        Ok(Self::new(&api_key))
    }

    pub fn completion_model(&self, model_name: &str) -> MistralCompletionModel {
        MistralCompletionModel {
            client: self.clone(),
            model: model_name.to_string(),
        }
    }

    pub fn agent(&self, model_name: &str) -> AgentBuilder<MistralCompletionModel> {
        let model = self.completion_model(model_name);
        AgentBuilder::new(model)
    }
}

#[derive(Debug, Deserialize)]
pub struct MistralResponse {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: Message,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl TryFrom<MistralResponse> for CompletionResponse<MistralResponse> {
    type Error = rig::completion::CompletionError;

    fn try_from(value: MistralResponse) -> Result<Self, Self::Error> {
        match value.choices.first() {
            Some(choice) => Ok(CompletionResponse {
                choice: ModelChoice::Message(choice.message.content.clone()),
                raw_response: value,
            }),
            None => Err(rig::completion::CompletionError::ResponseError(
                "No completion choices returned".into(),
            )),
        }
    }
}

#[derive(Clone)]
pub struct MistralCompletionModel {
    pub client: Client,
    pub model: String,
}

impl CompletionModel for MistralCompletionModel {
    type Response = MistralResponse;

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<MistralResponse>, rig::completion::CompletionError> {
        let mut messages = Vec::new();

        // Add system message if preamble exists
        if let Some(preamble) = &request.preamble {
            messages.push(json!({
                "role": "system",
                "content": preamble
            }));
        }

        // Add chat history
        for msg in &request.chat_history {
            messages.push(json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        // Add user prompt
        messages.push(json!({
            "role": "user",
            "content": request.prompt_with_context()
        }));

        let body = json!({
            "model": self.model,
            "messages": messages,
            "temperature": request.temperature.unwrap_or(0.7),
        });

        let url = format!("{}/chat/completions", self.client.base_url);
        let resp = self.client
            .http_client
            .post(url)
            .bearer_auth(&self.client.api_key)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(rig::completion::CompletionError::ProviderError(
                format!("Mistral API call failed: {status} - {text}")
            ));
        }

        let mistral_response: MistralResponse = resp.json().await?;
        mistral_response.try_into()
    }
}

impl rig::completion::Prompt for MistralCompletionModel {
    async fn prompt(&self, prompt: &str) -> Result<String, PromptError> {
        let request = CompletionRequest {
            prompt: prompt.to_string(),
            preamble: None,
            chat_history: vec![],
            documents: vec![],
            temperature: Some(0.7),
            max_tokens: None,
            tools: vec![],
            additional_params: None,
        };
        
        let response = self.completion(request).await
            .map_err(PromptError::from)?;
            
        match response.choice {
            ModelChoice::Message(content) => Ok(content),
            _ => Err(PromptError::from(CompletionError::ResponseError(
                "Unexpected response type".into()
            ))),
        }
    }
} 