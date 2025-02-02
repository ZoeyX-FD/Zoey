use async_trait::async_trait;
use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::path::PathBuf;

use crate::models::{MarketData, Conversation, AgentError};
use super::{Agent, BaseAgent, ModelProvider};

const TECHNICAL_SYSTEM_PROMPT: &str = r#"
You are Agent One - The Technical Analysis Expert 📊
Your role is to analyze charts, patterns, and market indicators to identify trading opportunities.

Focus on:
- Price action and chart patterns
- Technical indicators (RSI, MACD, etc.)
- Volume analysis
- Support/resistance levels
- Short to medium-term opportunities

Remember to be specific about entry/exit points and always consider risk management rules! 🎯

Format your response like this:

🤖 Hey! Technical Analysis Agent here!
=================================

📊 Market Analysis:
[Your technical analysis in simple terms]

💡 Key Patterns:
- [Pattern 1]
- [Pattern 2]
- [Pattern 3]

🎯 Trading Opportunities:
1. [Clear opportunity with entry/exit]
2. [Clear opportunity with entry/exit]
3. [Clear opportunity with entry/exit]

⚠️ Risk Management:
[Risk management considerations]
"#;

pub struct TechnicalAgent {
    base: BaseAgent,
}

impl TechnicalAgent {
    pub async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        Ok(Self {
            base: BaseAgent::new(
                "Technical Agent".to_string(),
                model,
                TECHNICAL_SYSTEM_PROMPT.to_string(),
                provider
            )
            .await?
            .with_temperature(0.7),
        })
    }
}

#[async_trait]
impl Agent for TechnicalAgent {
    fn name(&self) -> &str {
        &self.base.name
    }
    
    fn model(&self) -> &str {
        &self.base.model
    }
    
    async fn think(&mut self, market_data: &MarketData, previous_message: Option<String>) -> Result<String> {
        // Create context from market data
        let context = serde_json::to_string_pretty(market_data)?;
        
        let prompt = "Analyze the current market conditions from a technical analysis perspective.";
        
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