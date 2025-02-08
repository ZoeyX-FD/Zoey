use async_trait::async_trait;
use anyhow::Result;
use std::path::PathBuf;

use crate::models::MarketData;
use super::{Agent, BaseAgent, ModelProvider};

const SYNOPSIS_SYSTEM_PROMPT: &str = r#"
You are the Round Synopsis Agent 📊
Your role is to create clear, concise summaries of market analysis.

Guidelines:
- Synthesize insights from technical, fundamental, and sentiment analysis
- Focus on actionable opportunities and risks
- Highlight key market trends and patterns
- Provide clear directional bias
- Note significant market observations

Format your response like this:

📊 Market Synopsis:
[Brief overview of current market conditions]

💡 Key Insights:
- [Technical insight]
- [Fundamental insight]
- [Sentiment insight]

🎯 Trading Opportunities:
- [Specific actionable opportunities]
- [Entry/exit levels if applicable]

⚠️ Risk Factors:
- [Key risks to monitor]
- [Potential market threats]

🔄 Next Steps:
[Clear, actionable next steps for traders]
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
        sentiment_response: Option<&str>,
    ) -> Result<String> {
        // Extract key points from all sources
        let mut key_points = String::new();
        
        // Add technical analysis
        if let Some(technical) = agent_one_response.split("Key Patterns:").nth(1) {
            if let Some(end) = technical.find("\n\n") {
                key_points.push_str("📈 Technical Analysis:\n");
                key_points.push_str(&technical[..end]);
                key_points.push_str("\n\n");
            }
        }
        
        // Add fundamental analysis
        if let Some(fundamental) = agent_two_response.split("Key Fundamentals:").nth(1) {
            if let Some(end) = fundamental.find("\n\n") {
                key_points.push_str("🌍 Fundamental Analysis:\n");
                key_points.push_str(&fundamental[..end]);
                key_points.push_str("\n\n");
            }
        }

        // Add sentiment analysis if available
        if let Some(sentiment) = sentiment_response {
            if let Some(sentiment_part) = sentiment.split("Social Media Pulse:").nth(1) {
                if let Some(end) = sentiment_part.find("\n\n") {
                    key_points.push_str("🎭 Sentiment Analysis:\n");
                    key_points.push_str(&sentiment_part[..end]);
                    key_points.push_str("\n\n");
                }
            }
        }

        // Extract opportunities and risks
        let mut opportunities = String::new();
        if let Some(opps) = agent_one_response.split("Opportunities:").nth(1) {
            if let Some(end) = opps.find("\n\n") {
                opportunities.push_str(&opps[..end]);
                opportunities.push_str("\n");
            }
        }
        if let Some(opps) = agent_two_response.split("Opportunities:").nth(1) {
            if let Some(end) = opps.find("\n\n") {
                opportunities.push_str(&opps[..end]);
            }
        }

        // Generate AI synopsis from the combined insights
        let context = format!(
            "Analyze these market insights and create a comprehensive synopsis:\n\n\
             {}\n\nPotential Opportunities:\n{}",
            key_points,
            opportunities
        );
        
        // Request a structured analysis
        self.base.generate_response(
            "Create a detailed market synopsis highlighting key insights, opportunities, and risks. \
             Focus on actionable items and clear directional bias.", 
            Some(&context)
        ).await
    }
    
    #[allow(dead_code)]
    fn extract_key_points(&self, text: &str) -> String {
        let mut key_points = String::new();
        
        // Extract patterns/trends
        if let Some(patterns) = text.split("Key Patterns:").nth(1) {
            if let Some(end) = patterns.find("\n\n") {
                key_points.push_str("Patterns:\n");
                key_points.push_str(&patterns[..end]);
                key_points.push_str("\n\n");
            }
        }
        
        // Extract opportunities
        if let Some(opportunities) = text.split("Opportunities:").nth(1) {
            if let Some(end) = opportunities.find("\n\n") {
                key_points.push_str("Opportunities:\n");
                key_points.push_str(&opportunities[..end]);
                key_points.push_str("\n\n");
            }
        }
        
        // Extract risks
        if let Some(risks) = text.split("Risks:").nth(1) {
            if let Some(end) = risks.find("\n\n") {
                key_points.push_str("Risks:\n");
                key_points.push_str(&risks[..end]);
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