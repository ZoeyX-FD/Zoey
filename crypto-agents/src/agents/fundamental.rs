use async_trait::async_trait;
use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::path::PathBuf;

use crate::models::{MarketData, Conversation, AgentError};
use super::{Agent, BaseAgent, ModelProvider};

const FUNDAMENTAL_SYSTEM_PROMPT: &str = r#"
You are Agent Two - The Fundamental Analysis Expert 🌍
Your role is to analyze macro trends, project fundamentals, and long-term potential.

Focus on:
- Project fundamentals and technology
- Team and development activity
- Market trends and sentiment
- Competitor analysis
- Long-term growth potential

Format your response like this:

🤖 Hey! Fundamental Analysis Agent here!
=====================================

🌍 Market Overview:
[Your macro analysis in simple terms]

💡 Key Fundamentals:
- [Fundamental 1]
- [Fundamental 2]
- [Fundamental 3]

📈 Growth Opportunities:
1. [Long-term opportunity]
2. [Long-term opportunity]
3. [Long-term opportunity]

🔮 Future Outlook:
[Long-term market perspective]
"#;

pub struct FundamentalAgent {
    base: BaseAgent,
}

impl FundamentalAgent {
    pub async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        Ok(Self {
            base: BaseAgent::new(
                "Fundamental Agent".to_string(),
                model,
                FUNDAMENTAL_SYSTEM_PROMPT.to_string(),
                provider
            )
            .await?
            .with_temperature(0.7),
        })
    }
}

#[async_trait]
impl Agent for FundamentalAgent {
    fn name(&self) -> &str {
        &self.base.name
    }
    
    fn model(&self) -> &str {
        &self.base.model
    }
    
    async fn think(&mut self, market_data: &MarketData, previous_message: Option<String>) -> Result<String> {
        // Create context with both market data and technical analysis
        let mut context = serde_json::to_string_pretty(market_data)?;
        if let Some(ref tech_analysis) = previous_message {
            context.push_str("\n\nTechnical Analysis:\n");
            context.push_str(tech_analysis);
        }
        
        let prompt = "Analyze the current market conditions from a fundamental analysis perspective.";
        
        let response = self.base.generate_response(prompt, Some(&context)).await?;
        
        // Save to memory
        self.base.memory.conversations.push(Conversation {
            timestamp: Utc::now(),
            market_data: market_data.clone(),
            other_message: previous_message,
            response: response.clone(),
        });
        
        self.save_memory().await?;
        
        Ok(response)
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