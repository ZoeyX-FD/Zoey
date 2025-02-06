use async_trait::async_trait;
use anyhow::Result;
use std::path::PathBuf;

use crate::models::MarketData;
use super::{Agent, BaseAgent, ModelProvider};

const SYNOPSIS_SYSTEM_PROMPT: &str = r#"
You are the Round Synopsis Agent 📊
Your role is to create clear, concise summaries of trading discussions.

Guidelines:
- Summarize key points in 1-2 sentences
- Focus on actionable decisions
- Highlight agreement between agents
- Note significant market observations
- Track progress toward goals

Help keep track of the trading journey! 🎯

Format your response like this:

📝 Round Summary:
[Brief, focused summary of key points and decisions]

🎯 Action Items:
[Clear, actionable next steps]
"#;

pub struct SynopsisAgent {
    base: BaseAgent,
}

impl SynopsisAgent {
    pub async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        Ok(Self {
            base: BaseAgent::new(
                "Synopsis Agent".to_string(),
                model,
                SYNOPSIS_SYSTEM_PROMPT.to_string(),
                provider
            )
            .await?
            .with_temperature(0.3), // Keep temperature low for consistent summaries
        })
    }
    
    pub async fn generate_synopsis(
        &self,
        agent_one_response: &str,
        agent_two_response: &str,
        sentiment_response: Option<&str>,  // Make it optional
    ) -> Result<String> {
        let mut key_points = String::new();
        
        // Add technical analysis
        if let Some(technical) = agent_one_response.split("Key Patterns:").nth(1) {
            if let Some(end) = technical.find("\n\n") {
                key_points.push_str(&technical[..end]);
            }
        }
        
        // Add fundamental analysis
        if let Some(fundamental) = agent_two_response.split("Key Fundamentals:").nth(1) {
            if let Some(end) = fundamental.find("\n\n") {
                key_points.push_str("\n");
                key_points.push_str(&fundamental[..end]);
            }
        }

        // Add sentiment analysis if available
        if let Some(sentiment) = sentiment_response {
            if let Some(sentiment_part) = sentiment.split("Social Media Pulse:").nth(1) {
                if let Some(end) = sentiment_part.find("\n\n") {
                    key_points.push_str("\n");
                    key_points.push_str(&sentiment_part[..end]);
                }
            }
        }
        
        Ok(key_points)
    }
    
    #[allow(dead_code)]
    fn extract_key_points(&self, text: &str) -> String {
        // Extract only sections with key points
        let mut key_points = String::new();
        
        if let Some(opportunities) = text.split("Opportunities:").nth(1) {
            if let Some(end) = opportunities.find("\n\n") {
                key_points.push_str(&opportunities[..end]);
            }
        }
        
        key_points
    }
}

#[async_trait]
impl Agent for SynopsisAgent {
    fn name(&self) -> &str {
        &self.base.name
    }
    
    fn model(&self) -> &str {
        &self.base.model
    }
    
    async fn think(&mut self, _market_data: &MarketData, _previous_message: Option<String>) -> Result<String> {
        Err(anyhow::anyhow!("Synopsis agent doesn't implement think()"))
    }
    
    async fn save_memory(&self) -> Result<()> {
        self.base.save_memory().await
    }
    
    async fn load_memory(&mut self) -> Result<()> {
        self.base.load_memory().await
    }
    
    fn memory_file(&self) -> PathBuf {
        self.base.memory_file()
    }
} 