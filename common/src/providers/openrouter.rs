use serde::{Deserialize, Serialize};
use reqwest::Client as HttpClient;
use anyhow::Result;
use rig::completion::{CompletionModel, CompletionRequest, CompletionResponse, ModelChoice, PromptError, CompletionError};
use rig::agent::AgentBuilder;
use serde_json::json;

const API_URL: &str = "https://openrouter.ai/api/v1";

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
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .expect("OPENROUTER_API_KEY must be set");
        Ok(Self::new(&api_key))
    }

    pub fn completion_model(&self, model_name: &str) -> OpenRouterCompletionModel {
        OpenRouterCompletionModel {
            client: self.clone(),
            model: model_name.to_string(),
        }
    }

    pub fn agent(&self, model_name: &str) -> AgentBuilder<OpenRouterCompletionModel> {
        let model = self.completion_model(model_name);
        AgentBuilder::new(model)
    }
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterResponse {
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

impl TryFrom<OpenRouterResponse> for CompletionResponse<OpenRouterResponse> {
    type Error = rig::completion::CompletionError;

    fn try_from(value: OpenRouterResponse) -> Result<Self, Self::Error> {
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
pub struct OpenRouterCompletionModel {
    pub client: Client,
    pub model: String,
}

impl CompletionModel for OpenRouterCompletionModel {
    type Response = OpenRouterResponse;

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<OpenRouterResponse>, rig::completion::CompletionError> {
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
            .header("HTTP-Referer", "https://github.com/your-repo") // Required by OpenRouter
            .header("X-Title", "RIG Framework") // Required by OpenRouter
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(rig::completion::CompletionError::ProviderError(
                format!("OpenRouter API call failed: {status} - {text}")
            ));
        }

        let openrouter_response: OpenRouterResponse = resp.json().await?;
        openrouter_response.try_into()
    }
}

impl rig::completion::Prompt for OpenRouterCompletionModel {
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