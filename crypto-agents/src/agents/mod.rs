use async_trait::async_trait;
use anyhow::Result;
use rig::{
    completion::Prompt,
    providers::{deepseek, gemini},
    agent::Agent as RigAgent,
};
use common::providers::mistral;
use serde_json::json;
use std::path::PathBuf;
use std::env;

use crate::models::{MarketData, Memory, AgentError};

pub mod technical;
pub mod fundamental;
pub mod extractor;
pub mod synopsis;
pub mod new_top;

pub use technical::TechnicalAgent;
pub use fundamental::FundamentalAgent;
pub use extractor::TokenExtractor;
pub use synopsis::SynopsisAgent;
pub use new_top::NewTopAgent;

const AGENT_MEMORY_DIR: &str = "data/agent_memory";

#[derive(Clone, Debug)]
pub enum ModelProvider {
    DeepSeek,
    Gemini,
    Mistral,
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
    preamble: String,
    temperature: f32,
}

impl BaseAgent {
    pub async fn new(name: String, model: String, preamble: String, provider: ModelProvider) -> Result<Self> {
        // Create memory directory if it doesn't exist
        tokio::fs::create_dir_all(AGENT_MEMORY_DIR).await?;
        
        // Initialize appropriate client and agent based on provider
        let (deepseek_agent, gemini_agent, mistral_agent) = match provider {
            ModelProvider::DeepSeek => {
                let deepseek_key = env::var("DEEPSEEK_API_KEY")
                    .map_err(|_| AgentError::ApiError("DEEPSEEK_API_KEY not found".to_string()))?;
                
                let client = deepseek::Client::new(&deepseek_key);
                let agent = client.agent(deepseek::DEEPSEEK_CHAT)
                    .preamble(&preamble)
                    .temperature(0.7)
                    .build();
                    
                (Some(agent), None, None)
            },
            ModelProvider::Gemini => {
                let gemini_key = env::var("GEMINI_API_KEY")
                    .map_err(|_| AgentError::ApiError("GEMINI_API_KEY not found".to_string()))?;
                    
                let client = gemini::Client::new(&gemini_key);
                let agent = client.agent("gemini-pro")
                    .preamble(&preamble)
                    .temperature(0.7)
                    .build();
                
                (None, Some(agent), None)
            },
            ModelProvider::Mistral => {
                let mistral_key = env::var("MISTRAL_API_KEY")
                    .map_err(|_| AgentError::ApiError("MISTRAL_API_KEY not found".to_string()))?;
                    
                let client = mistral::Client::new(&mistral_key);
                let agent = client.agent(mistral::MISTRAL_LARGE)
                    .preamble(&preamble)
                    .temperature(0.7)
                    .build();
                
                (None, None, Some(agent))
            }
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

        match self.provider {
            ModelProvider::DeepSeek => {
                let agent = self.deepseek_agent.as_ref()
                    .ok_or_else(|| AgentError::ApiError("DeepSeek agent not initialized".to_string()))?;
                
                let response = agent.prompt(&full_prompt).await
                    .map_err(|e| AgentError::ApiError(e.to_string()))?;
                
                Ok(response)
            },
            ModelProvider::Gemini => {
                let agent = self.gemini_agent.as_ref()
                    .ok_or_else(|| AgentError::ApiError("Gemini agent not initialized".to_string()))?;
                
                let response = agent.prompt(&full_prompt).await
                    .map_err(|e| AgentError::ApiError(e.to_string()))?;
                
                Ok(response)
            },
            ModelProvider::Mistral => {
                let agent = self.mistral_agent.as_ref()
                    .ok_or_else(|| AgentError::ApiError("Mistral agent not initialized".to_string()))?;
                
                let response = agent.prompt(&full_prompt).await
                    .map_err(|e| AgentError::ApiError(e.to_string()))?;
                
                Ok(response)
            }
        }
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