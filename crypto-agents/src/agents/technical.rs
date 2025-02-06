use async_trait::async_trait;
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;

use crate::models::{MarketData, Conversation};
use super::{Agent, BaseAgent, ModelProvider};
use crate::api::coingecko::DetailedCoinData;
use crate::api::social_media::SocialMediaPost;

const TECHNICAL_SYSTEM_PROMPT: &str = r#"
You are Agent One - The Technical Analysis Expert 📊
Your role is to analyze charts, patterns, and market indicators to identify trading opportunities.

Focus on:
- Price action and chart patterns
- Technical indicators (RSI, MACD, stochastics,bollinger bands, etc.)
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

#[derive(Debug)]
pub struct TechnicalAnalysis {
    pub analysis: String,
    pub market_outlook: String,
    pub risk_level: String,
}

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

    pub async fn analyze_coin_data(
        &self,
        symbol: &str,
        coin_data: &DetailedCoinData,
        sentiment_data: Option<&Vec<SocialMediaPost>>,
    ) -> Result<TechnicalAnalysis> {
        let mut prompt = format!(
            "Analyze {} based on the following market data:\n\n", 
            symbol
        );

        // Add market data
        prompt.push_str(&format!(
            "Price: ${:.2}\n\
             24h Change: {:.2}%\n\
             Market Cap: ${:.2}M\n",
            coin_data.current_price,
            coin_data.price_change_24h.unwrap_or(0.0),
            coin_data.market_cap / 1e6,
        ));

        if let Some(posts) = sentiment_data {
            prompt.push_str("\nConsider this social sentiment data:\n");
            for post in posts.iter().take(5) {
                prompt.push_str(&format!("- {}\n", post.content));
            }
        }

        prompt.push_str("\nProvide a technical analysis with the following structure:\n");
        prompt.push_str("1. Technical Analysis: Detailed market analysis\n");
        prompt.push_str("2. Market Outlook: One word (Bullish/Bearish/Neutral)\n");
        prompt.push_str("3. Risk Level: One word (High/Medium/Low)\n");

        // Get analysis from model
        let response = self.base.generate_response(&prompt, None).await?;

        // Parse response sections
        let sections: Vec<&str> = response.split('\n').collect();
        let mut analysis = String::new();
        let mut market_outlook = String::from("Neutral");
        let mut risk_level = String::from("Medium");

        for section in sections {
            if section.starts_with("Technical Analysis:") {
                analysis = section.replace("Technical Analysis:", "").trim().to_string();
            } else if section.starts_with("Market Outlook:") {
                market_outlook = section.replace("Market Outlook:", "").trim().to_string();
            } else if section.starts_with("Risk Level:") {
                risk_level = section.replace("Risk Level:", "").trim().to_string();
            }
        }

        Ok(TechnicalAnalysis {
            analysis,
            market_outlook,
            risk_level,
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