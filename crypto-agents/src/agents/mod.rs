use async_trait::async_trait;
use anyhow::Result;
use rig::{
    completion::Prompt,
    providers::{deepseek, gemini, openai, cohere},
    agent::Agent as RigAgent,
};
use common::providers::{mistral, openrouter};
use std::path::PathBuf;
use std::env;

use crate::models::{MarketData, Memory, AgentError};

pub mod technical;
pub mod fundamental;
pub mod extractor;
pub mod synopsis;
pub mod new_top;
pub mod sentiment;
pub mod topic;

pub use technical::TechnicalAgent;
pub use fundamental::FundamentalAgent;
pub use extractor::TokenExtractor;
pub use extractor::ExtractorAgent;
pub use synopsis::SynopsisAgent;
pub use new_top::NewTopAgent;
pub use sentiment::SentimentAgent;
pub use topic::TopicAgent;

const AGENT_MEMORY_DIR: &str = "data/agent_memory";

pub const DEEPSEEK_MODELS: &[&str] = &[
    "deepseek-chat",
    "deepseek-reasoner",
];

#[derive(Debug, Clone)]
pub enum ModelProvider {
    DeepSeek,
    Gemini,
    Mistral,
    OpenAI,
    Cohere,
    OpenRouter,
}

impl ModelProvider {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "deepseek" => Some(Self::DeepSeek),
            "gemini" => Some(Self::Gemini),
            "mistral" => Some(Self::Mistral),
            "openai" => Some(Self::OpenAI),
            "cohere" => Some(Self::Cohere),
            "openrouter" => Some(Self::OpenRouter),
            _ => None
        }
    }

    pub fn default_model(&self) -> &str {
        match self {
            Self::DeepSeek => "deepseek-reasoner",
            Self::Gemini => "gemini-pro",
            Self::Mistral => "mistral-large-latest",
            Self::OpenAI => "gpt-4-turbo-preview",
            Self::Cohere => "command-nightly",
            Self::OpenRouter => "anthropic/claude-2",
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Self::DeepSeek => "deepseek",
            Self::Gemini => "gemini",
            Self::Mistral => "mistral",
            Self::OpenAI => "openai",
            Self::Cohere => "cohere",
            Self::OpenRouter => "openrouter",
        }.to_string()
    }
}

#[async_trait]
pub trait Agent: Send + Sync {
    /// Get the agent's name
    fn name(&self) -> &str;
    
    /// Get the agent's model
    fn model(&self) -> &str;
    
    /// Process market data and generate analysis
    async fn think(&mut self, market_data: &MarketData, previous_message: Option<String>) -> Result<String>;
    
    /// Save agent's memory to storage
    async fn save_memory(&self) -> Result<()>;
    
    /// Load agent's memory from storage
    async fn load_memory(&mut self) -> Result<()>;
    
    /// Get memory file path
    fn memory_file(&self) -> PathBuf;
}

/// Base implementation for AI agents using rig-core
pub struct BaseAgent {
    name: String,
    model: String,
    provider: ModelProvider,
    memory: Memory,
    deepseek_agent: Option<RigAgent<deepseek::DeepSeekCompletionModel>>,
    gemini_agent: Option<RigAgent<gemini::completion::CompletionModel>>,
    mistral_agent: Option<RigAgent<mistral::MistralCompletionModel>>,
    openai_agent: Option<RigAgent<openai::CompletionModel>>,
    cohere_agent: Option<RigAgent<cohere::CompletionModel>>,
    openrouter_agent: Option<RigAgent<openrouter::OpenRouterCompletionModel>>,
    #[allow(dead_code)]
    preamble: String,
    temperature: f32,
}

impl BaseAgent {
    pub async fn new(name: String, model: String, preamble: String, provider: ModelProvider) -> Result<Self> {
        // Create memory directory if it doesn't exist
        tokio::fs::create_dir_all(AGENT_MEMORY_DIR).await?;
        
        // Initialize appropriate client and agent based on provider
        let (deepseek_agent, gemini_agent, mistral_agent, openai_agent, cohere_agent, openrouter_agent) = match provider {
            ModelProvider::DeepSeek => {
                let deepseek_key = env::var("DEEPSEEK_API_KEY")
                    .map_err(|_| AgentError::ApiError("DEEPSEEK_API_KEY not found".to_string()))?;
                
                let client = deepseek::Client::new(&deepseek_key);
                let agent = client.agent(deepseek::DEEPSEEK_CHAT)
                    .preamble(&preamble)
                    .temperature(0.7)
                    .build();
                    
                (Some(agent), None, None, None, None, None)
            },
            ModelProvider::Gemini => {
                let gemini_key = env::var("GEMINI_API_KEY")
                    .map_err(|_| AgentError::ApiError("GEMINI_API_KEY not found".to_string()))?;
                    
                let client = gemini::Client::new(&gemini_key);
                let agent = client.agent("gemini-1.5-pro")
                    .preamble(&preamble)
                    .temperature(0.7)
                    .build();
                
                (None, Some(agent), None, None, None, None)
            },
            ModelProvider::Mistral => {
                let mistral_key = env::var("MISTRAL_API_KEY")
                    .map_err(|_| AgentError::ApiError("MISTRAL_API_KEY not found".to_string()))?;
                    
                let client = mistral::Client::new(&mistral_key);
                let agent = client.agent(mistral::MISTRAL_LARGE)
                    .preamble(&preamble)
                    .temperature(0.7)
                    .build();
                
                (None, None, Some(agent), None, None, None)
            },
            ModelProvider::OpenAI => {
                let openai_key = env::var("OPENAI_API_KEY")
                    .map_err(|_| AgentError::ApiError("OPENAI_API_KEY not found".to_string()))?;
                    
                let client = openai::Client::new(&openai_key);
                let agent = client.agent("gpt-4-turbo-preview")
                    .preamble(&preamble)
                    .temperature(0.7)
                    .build();
                
                (None, None, None, Some(agent), None, None)
            },
            ModelProvider::Cohere => {
                let cohere_key = env::var("COHERE_API_KEY")
                    .map_err(|_| AgentError::ApiError("COHERE_API_KEY not found".to_string()))?;
                    
                let client = cohere::Client::new(&cohere_key);
                let agent = client.agent("command-nightly")
                    .preamble(&preamble)
                    .temperature(0.7)
                    .build();
                
                (None, None, None, None, Some(agent), None)
            },
            ModelProvider::OpenRouter => {
                let openrouter_key = env::var("OPENROUTER_API_KEY")
                    .map_err(|_| AgentError::ApiError("OPENROUTER_API_KEY not found".to_string()))?;
                    
                let client = openrouter::Client::new(&openrouter_key);
                let agent = client.agent(&model)
                    .preamble(&preamble)
                    .temperature(0.7)
                    .build();
                
                (None, None, None, None, None, Some(agent))
            },
        };
        
        let mut agent = Self {
            name,
            model,
            provider,
            memory: Memory {
                conversations: Vec::new(),
                decisions: Vec::new(),
                portfolio_history: Vec::new(),
            },
            deepseek_agent,
            gemini_agent,
            mistral_agent,
            openai_agent,
            cohere_agent,
            openrouter_agent,
            preamble,
            temperature: 0.7,
        };
        
        agent.load_memory().await?;
        Ok(agent)
    }
    
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }
    
    pub async fn generate_response(&self, prompt: &str, context: Option<&str>) -> Result<String> {
        let full_prompt = if let Some(ctx) = context {
            format!("{}\n\nContext:\n{}", prompt, ctx)
        } else {
            prompt.to_string()
        };

        let result = match self.provider {
            ModelProvider::DeepSeek => {
                let agent = self.deepseek_agent.as_ref()
                    .ok_or_else(|| AgentError::ApiError("DeepSeek agent not initialized".to_string()))?;
                agent.prompt(&full_prompt).await
            },
            ModelProvider::Gemini => {
                let agent = self.gemini_agent.as_ref()
                    .ok_or_else(|| AgentError::ApiError("Gemini agent not initialized".to_string()))?;
                agent.prompt(&full_prompt).await
            },
            ModelProvider::Mistral => {
                let agent = self.mistral_agent.as_ref()
                    .ok_or_else(|| AgentError::ApiError("Mistral agent not initialized".to_string()))?;
                agent.prompt(&full_prompt).await
            },
            ModelProvider::OpenAI => {
                let agent = self.openai_agent.as_ref()
                    .ok_or_else(|| AgentError::ApiError("OpenAI agent not initialized".to_string()))?;
                agent.prompt(&full_prompt).await
            },
            ModelProvider::Cohere => {
                let agent = self.cohere_agent.as_ref()
                    .ok_or_else(|| AgentError::ApiError("Cohere agent not initialized".to_string()))?;
                agent.prompt(&full_prompt).await
            },
            ModelProvider::OpenRouter => {
                let agent = self.openrouter_agent.as_ref()
                    .ok_or_else(|| AgentError::ApiError("OpenRouter agent not initialized".to_string()))?;
                agent.prompt(&full_prompt).await
            },
        };

        // Convert the provider-specific error to anyhow::Error
        result.map_err(|e| anyhow::anyhow!("Agent error: {}", e))
    }
    
    pub async fn save_memory(&self) -> Result<()> {
        let memory_path = self.memory_file();
        let memory_json = serde_json::to_string_pretty(&self.memory)?;
        tokio::fs::write(memory_path, memory_json).await?;
        Ok(())
    }
    
    pub async fn load_memory(&mut self) -> Result<()> {
        let memory_path = self.memory_file();
        if memory_path.exists() {
            let memory_json = tokio::fs::read_to_string(memory_path).await?;
            self.memory = serde_json::from_str(&memory_json)?;
        }
        Ok(())
    }
    
    pub fn memory_file(&self) -> PathBuf {
        PathBuf::from(AGENT_MEMORY_DIR).join(format!("{}_memory.json", self.name.to_lowercase().replace(' ', "_")))
    }
} 