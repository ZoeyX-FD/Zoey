use serde::{Deserialize, Serialize};
use reqwest::Client as HttpClient;
use anyhow::Result;
use rig::completion::{
    CompletionModel, CompletionRequest, CompletionResponse, 
    Message as RigMessage, PromptError, CompletionError, AssistantContent
};
use rig::message::{Text, UserContent};
use rig::OneOrMany;
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

    pub async fn completion(&self, request: CompletionRequest) -> Result<CompletionResponse<OpenRouterResponse>, CompletionError> {
        // Create a temporary OpenRouterCompletionModel to use its create_request_body method
        let model = OpenRouterCompletionModel {
            client: self.clone(),
            model: "default".to_string(), // This won't be used since we're just using it for request formatting
        };
        
        let body = model.create_request_body(&request);

        let response = self.http_client
            .post(&format!("{}/api/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body) // Use the formatted body instead of request directly
            .send()
            .await?
            .json::<OpenRouterResponse>()
            .await?;

        response.try_into()
    }
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterResponse {
    pub choices: Vec<OpenRouterChoice>,
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterChoice {
    pub message: OpenRouterMessage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterMessage {
    pub role: String,
    pub content: String,
}

impl TryFrom<OpenRouterResponse> for CompletionResponse<OpenRouterResponse> {
    type Error = CompletionError;

    fn try_from(value: OpenRouterResponse) -> Result<Self, Self::Error> {
        match value.choices.first() {
            Some(choice) => {
                let text = Text { text: choice.message.content.clone() };
                Ok(CompletionResponse {
                    choice: OneOrMany::one(AssistantContent::Text(text)),
                    raw_response: value,
                })
            },
            None => Err(CompletionError::ResponseError(
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

impl OpenRouterCompletionModel {
    fn create_request_body(&self, request: &CompletionRequest) -> serde_json::Value {
        let mut messages = Vec::new();

        // Add system message if preamble exists
        if let Some(preamble) = &request.preamble {
            messages.push(json!({
                "role": "system",
                "content": preamble
            }));
        }

        // Add user prompt first if no chat history
        if request.chat_history.is_empty() {
            let prompt_text = match &request.prompt {
                RigMessage::User { content } => content.iter()
                    .map(|c| match c {
                        UserContent::Text(text) => text.text.clone(),
                        _ => String::new(),
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };

            messages.push(json!({
                "role": "user",
                "content": prompt_text
            }));
        } else {
            // Add chat history
            for msg in &request.chat_history {
                let (role, content) = match msg {
                    RigMessage::User { content } => {
                        let text = content.iter()
                            .map(|c| match c {
                                UserContent::Text(text) => text.text.clone(),
                                _ => String::new(),
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        ("user", text)
                    },
                    RigMessage::Assistant { content } => {
                        let text = content.iter()
                            .map(|c| match c {
                                AssistantContent::Text(text) => text.text.clone(),
                                _ => String::new(),
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        ("assistant", text)
                    },
                };
                messages.push(json!({
                    "role": role,
                    "content": content
                }));
            }

            // Add current prompt
            let prompt_text = match &request.prompt {
                RigMessage::User { content } => content.iter()
                    .map(|c| match c {
                        UserContent::Text(text) => text.text.clone(),
                        _ => String::new(),
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };

            messages.push(json!({
                "role": "user",
                "content": prompt_text
            }));
        }

        json!({
            "model": self.model,
            "messages": messages,
            "temperature": request.temperature.unwrap_or(0.7),
        })
    }
}

impl CompletionModel for OpenRouterCompletionModel {
    type Response = OpenRouterResponse;

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<OpenRouterResponse>, CompletionError> {
        let body = self.create_request_body(&request);

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
            return Err(CompletionError::ProviderError(
                format!("OpenRouter API call failed: {status} - {text}")
            ));
        }

        let openrouter_response: OpenRouterResponse = resp.json().await?;

        openrouter_response.try_into()
    }
}

impl rig::completion::Prompt for OpenRouterCompletionModel {
    async fn prompt(&self, prompt: impl Into<RigMessage> + Send) -> Result<String, PromptError> {
        let request = CompletionRequest {
            prompt: prompt.into(),
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
            
        match response.choice.iter().next() {
            Some(AssistantContent::Text(text)) => Ok(text.text.clone()),
            _ => Err(PromptError::from(CompletionError::ResponseError(
                "Unexpected response format".into()
            ))),
        }
    }
} 