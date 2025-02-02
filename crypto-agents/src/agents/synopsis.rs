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
    ) -> Result<String> {
        // Create a more focused prompt with key points only
        let context = format!(
            "Technical Analysis Key Points:\n{}\n\nFundamental Analysis Key Points:\n{}\n\nCreate a brief, focused synopsis highlighting only the most important points and actionable items.",
            self.extract_key_points(agent_one_response),
            self.extract_key_points(agent_two_response)
        );
        
        self.base.generate_response("Create a brief synopsis focusing on key decisions and actions.", Some(&context)).await
    }
    
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