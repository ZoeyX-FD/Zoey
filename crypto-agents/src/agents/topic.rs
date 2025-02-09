use async_trait::async_trait;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;

use crate::models::{MarketData, Conversation};
use crate::api::{
    coingecko::DetailedCoinData,
    social_media::SocialMediaPost
};
use super::{Agent, BaseAgent, ModelProvider};
use crate::models::AgentError;

const TOPIC_SYSTEM_PROMPT: &str = r#"
You are a Market Topics Analysis AI specializing in cryptocurrency market analysis.

Your role is to analyze market data, technical analysis, and sentiment to identify key market themes and trends.

Format your response in the following structure:

🌍 Market Topics Analysis

📊 Key Market Themes:
- [List 3-5 dominant market narratives]
- [Focus on major price drivers]
- [Include sector-specific trends]

🚀 Growth Catalysts:
- [List 3-4 potential growth drivers]
- [Include upcoming events/developments]
- [Highlight positive market indicators]
- [Format: "Catalyst: Expected Impact"]

⚠️ Risk Factors:
- [List 3-4 key risks]
- [Include both macro and micro risks]
- [Highlight specific warning signs]
- [Format: "Risk: Potential Impact"]

💡 Trading Implications:
- [2-3 actionable insights]
- [Specific sectors to watch]
- [Risk management considerations]

Guidelines:
1. Be specific and data-driven
2. Avoid repetition across sections
3. Focus on actionable insights
4. Quantify impacts where possible
5. Highlight timeframes for catalysts/risks

Example format:
📊 Key Market Themes:
- Layer 1 Competition: SOL +15% weekly, outperforming BTC/ETH
- DeFi Revival: DEX volumes up 25% MoM
- AI Token Consolidation: Sector down 5% after recent rally

🚀 Growth Catalysts:
- ETH Dencun Upgrade (March): Potential 30-40% fee reduction
- BTC Halving (April): Historical bullish impact
- SOL Ecosystem Growth: Rising TVL, +40% monthly

⚠️ Risk Factors:
- Regulatory Uncertainty: SEC actions pending
- Market Correlation: 0.85 correlation with tech stocks
- Volume Decline: -20% weekly, suggesting weak conviction

💡 Trading Implications:
- Layer 1 rotation opportunity: Focus on SOL, AVAX
- Risk Management: Tight stops due to high correlation
- Timing: Wait for volume confirmation before large positions
"#;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopicAnalysis {
    pub timestamp: String,
    pub sector: String,
    pub sentiment: f64,
    pub key_projects: Vec<String>,
    pub catalysts: Vec<String>,
    pub risks: Vec<String>,
    pub trading_implications: Vec<String>,
}

pub struct TopicAgent {
    base: BaseAgent,
    analysis_history: HashMap<String, Vec<TopicAnalysis>>,
}

impl TopicAgent {
    pub async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        Ok(Self {
            base: BaseAgent::new(
                "Topic Analysis Agent".to_string(),
                model,
                TOPIC_SYSTEM_PROMPT.to_string(),
                provider
            ).await?.with_temperature(0.7),
            analysis_history: HashMap::new(),
        })
    }

    pub async fn analyze_sector(&mut self, sector: &str, market_data: &MarketData) -> Result<TopicAnalysis> {
        let prompt = format!(
            "Analyze the {} sector in detail. Focus on current trends, key projects, and upcoming catalysts.",
            sector
        );

        let context = serde_json::to_string_pretty(market_data)?;
        let response = self.base.generate_response(&prompt, Some(&context)).await?;

        // Parse the response to extract structured data
        let analysis = TopicAnalysis {
            timestamp: Utc::now().to_rfc3339(),
            sector: sector.to_string(),
            sentiment: self.calculate_sector_sentiment(&response),
            key_projects: self.extract_key_projects(&response),
            catalysts: self.extract_catalysts(&response),
            risks: self.extract_risks(&response),
            trading_implications: self.extract_trading_implications(&response),
        };

        // Store in history
        self.analysis_history
            .entry(sector.to_string())
            .or_insert_with(Vec::new)
            .push(analysis.clone());

        Ok(analysis)
    }

    pub async fn analyze_coin_with_sentiment(
        &mut self,
        symbol: &str,
        coin_data: &DetailedCoinData,
        sentiment_data: &[SocialMediaPost]
    ) -> Result<TopicAnalysis> {
        let prompt = format!(
            "Analyze {} ({}) with the following data:\n\
            Current Price: ${:.4}\n\
            1h Change: {:.2}%\n\
            24h Change: {:.2}%\n\
            7d Change: {:.2}%\n\
            Market Cap: ${:.2}M\n\
            24h Volume: ${:.2}M\n\
            Social Media Posts: {}\n\
            Average Engagement: {:.1}\n\n\
            Focus on market trends, social sentiment, and upcoming catalysts.",
            coin_data.name,
            symbol,
            coin_data.current_price,
            coin_data.price_change_1h.unwrap_or_default(),
            coin_data.price_change_24h.unwrap_or_default(),
            coin_data.price_change_7d.unwrap_or_default(),
            coin_data.market_cap / 1_000_000.0,
            coin_data.volume_24h / 1_000_000.0,
            sentiment_data.len(),
            if !sentiment_data.is_empty() {
                sentiment_data.iter().map(|p| p.engagement).sum::<i32>() as f64 / sentiment_data.len() as f64
            } else {
                0.0
            }
        );

        // Add sample tweets for context
        let mut context = String::new();
        for (i, post) in sentiment_data.iter().take(5).enumerate() {
            context.push_str(&format!(
                "\nTweet {}: {} (Engagement: {})",
                i + 1,
                post.content,
                post.engagement
            ));
        }

        let response = self.base.generate_response(&prompt, Some(&context)).await?;

        let analysis = TopicAnalysis {
            timestamp: Utc::now().to_rfc3339(),
            sector: format!("{} Analysis", symbol),
            sentiment: self.calculate_combined_sentiment(&response, sentiment_data),
            key_projects: self.extract_key_points(&response),
            catalysts: self.extract_catalysts(&response),
            risks: self.extract_risks(&response),
            trading_implications: self.extract_trading_implications(&response),
        };

        // Store in history
        self.analysis_history
            .entry(symbol.to_string())
            .or_insert_with(Vec::new)
            .push(analysis.clone());

        Ok(analysis)
    }

    fn calculate_sector_sentiment(&self, text: &str) -> f64 {
        let positive_words = ["bullish", "growth", "positive", "opportunity", "strong", "success"];
        let negative_words = ["bearish", "risk", "negative", "concern", "weak", "failure"];
        
        let text_lower = text.to_lowercase();
        let positive_count = positive_words.iter()
            .map(|word| text_lower.matches(*word).count())
            .sum::<usize>();
        let negative_count = negative_words.iter()
            .map(|word| text_lower.matches(*word).count())
            .sum::<usize>();
            
        let total = positive_count + negative_count;
        if total == 0 {
            return 0.0;
        }
        (positive_count as f64 - negative_count as f64) / total as f64
    }

    fn calculate_combined_sentiment(&self, text: &str, sentiment_data: &[SocialMediaPost]) -> f64 {
        // Combine AI analysis with social sentiment
        let ai_sentiment = self.calculate_sector_sentiment(text);
        
        // Calculate social sentiment
        let social_sentiment = sentiment_data.iter()
            .filter_map(|post| post.sentiment_score)
            .sum::<f64>() / sentiment_data.len() as f64;
        
        // Weight: 40% AI analysis, 60% social sentiment
        (ai_sentiment * 0.4) + (social_sentiment * 0.6)
    }

    fn extract_key_projects(&self, text: &str) -> Vec<String> {
        let mut projects = Vec::new();
        if let Some(section) = text.split("Key Projects:").nth(1) {
            if let Some(end) = section.find("\n\n") {
                let lines = section[..end].lines();
                for line in lines {
                    if line.starts_with('-') {
                        projects.push(line.trim_start_matches('-').trim().to_string());
                    }
                }
            }
        }
        projects
    }

    fn extract_catalysts(&self, text: &str) -> Vec<String> {
        let mut catalysts = Vec::new();
        if let Some(section) = text.split("Growth Catalysts:").nth(1) {
            if let Some(end) = section.find("\n\n") {
                let lines = section[..end].lines();
                for line in lines {
                    if !line.trim().is_empty() {
                        catalysts.push(line.trim().to_string());
                    }
                }
            }
        }
        catalysts
    }

    fn extract_risks(&self, text: &str) -> Vec<String> {
        let mut risks = Vec::new();
        if let Some(section) = text.split("Risk Assessment:").nth(1) {
            let lines = section.lines();
            for line in lines {
                if !line.trim().is_empty() {
                    risks.push(line.trim().to_string());
                }
            }
        }
        risks
    }

    fn extract_key_points(&self, text: &str) -> Vec<String> {
        let mut points = Vec::new();
        if let Some(section) = text.split("💡 Key Points:").nth(1) {
            if let Some(end) = section.find("\n\n") {
                let lines = section[..end].lines();
                for line in lines {
                    if !line.trim().is_empty() {
                        points.push(line.trim().to_string());
                    }
                }
            }
        }
        points
    }

    fn extract_trading_implications(&self, text: &str) -> Vec<String> {
        let mut implications = Vec::new();
        if let Some(section) = text.split("💡 Trading Implications:").nth(1) {
            if let Some(end) = section.find("\n\n") {
                let lines = section[..end].lines();
                for line in lines {
                    if !line.trim().is_empty() {
                        implications.push(line.trim().to_string());
                    }
                }
            }
        }
        implications
    }

    pub fn format_market_data(&self, coin_data: &DetailedCoinData) -> String {
        let mut output = String::new();
        
        // Format price with appropriate precision
        let price = if coin_data.current_price < 0.01 {
            format!("${:.8}", coin_data.current_price)
        } else {
            format!("${:.4}", coin_data.current_price)
        };
        
        output.push_str(&format!("Current Price: {}\n", price));
        
        // Format price changes
        if let Some(change) = coin_data.price_change_1h {
            output.push_str(&format!("1h Change: {:.2}%\n", change));
        }
        if let Some(change) = coin_data.price_change_24h {
            output.push_str(&format!("24h Change: {:.2}%\n", change));
        }
        if let Some(change) = coin_data.price_change_7d {
            output.push_str(&format!("7d Change: {:.2}%\n", change));
        }
        
        // Format market cap
        if coin_data.market_cap > 0.0 {
            let market_cap_m = coin_data.market_cap / 1_000_000.0;
            output.push_str(&format!("Market Cap: ${:.2}M\n", market_cap_m));
        }
        
        // Format volume
        if coin_data.volume_24h > 0.0 {
            let volume_m = coin_data.volume_24h / 1_000_000.0;
            output.push_str(&format!("24h Volume: ${:.2}M", volume_m));
        }
        
        output
    }

    pub async fn analyze_topic(&self, context: &str) -> Result<TopicAnalysis> {
        let prompt = format!(
            r#"You are a Crypto Market Intelligence AI analyzing social media data and market trends.

Analyze the following topic data and provide detailed insights:

{context}

Please provide a comprehensive analysis with at least 3 points in each section, even if the data is limited.
Focus on:
1. Current state and developments
2. Market sentiment and community reaction
3. Technical developments and innovations
4. Future potential and opportunities
5. Possible risks and challenges

Format your response EXACTLY as follows:

KEY INSIGHTS:
- [Key insight about current state]
- [Key insight about market dynamics]
- [Key insight about notable developments]

CURRENT TRENDS:
- [Trend with supporting evidence]
- [Trend with market impact]
- [Trend with future implications]

RISKS AND CHALLENGES:
- [Risk with potential impact]
- [Challenge facing adoption]
- [Market-related concern]

Keep insights specific, data-driven, and actionable. If data is limited, provide analysis based on broader market context and similar projects."#
        );

        let response = self.base.generate_response(&prompt, None).await?;
        
        // Parse sections
        let key_projects = self.extract_section(&response, "KEY INSIGHTS:");
        let catalysts = self.extract_section(&response, "CURRENT TRENDS:");
        let risks = self.extract_section(&response, "RISKS AND CHALLENGES:");

        // Ensure we have at least some content in each section
        if key_projects.is_empty() || catalysts.is_empty() || risks.is_empty() {
            return Err(AgentError::InvalidData("Incomplete analysis generated".to_string()).into());
        }

        Ok(TopicAnalysis {
            timestamp: Utc::now().to_rfc3339(),
            sector: "Custom Topic".to_string(),
            sentiment: self.calculate_sector_sentiment(&response),
            key_projects,
            catalysts,
            risks,
            trading_implications: self.extract_trading_implications(&response),
        })
    }

    // Helper method to extract sections
    fn extract_section(&self, text: &str, section_header: &str) -> Vec<String> {
        if let Some(section_start) = text.find(section_header) {
            let section_text = &text[section_start..];
            if let Some(section_end) = section_text.find('\n') {
                return section_text[section_end..]
                    .lines()
                    .map(|line| line.trim())
                    .filter(|line| line.starts_with('-'))
                    .map(|line| line.trim_start_matches('-').trim().to_string())
                    .filter(|line| !line.is_empty())
                    .collect();
            }
        }
        Vec::new()
    }

    pub async fn analyze_market_topics(
        &self,
        market_data: &MarketData,
        technical_analysis: &str,
        sentiment_analysis: &str
    ) -> Result<String> {
        println!("🔄 Processing market topics...");

        // Add shorter timeout and progress indicator
        let timeout_duration = std::time::Duration::from_secs(15); // Reduced from 30s to 15s

        // 1. Limit data size more aggressively
        let market_summary = self.summarize_market_data(market_data);
        // Take only first 500 chars of each analysis
        let tech_summary = technical_analysis.chars().take(500).collect::<String>();
        let sentiment_summary = sentiment_analysis.chars().take(500).collect::<String>();

        let context = format!(
            "Provide brief market analysis (max 3 points per section):\n\n\
             Market Summary:\n{}\n\n\
             Technical Summary:\n{}\n\n\
             Sentiment Summary:\n{}\n\n",
            market_summary,
            tech_summary,
            sentiment_summary
        );

        println!("⏳ Analyzing market context...");
        println!("(Timeout set to 15 seconds)");

        // Add timeout with fallback
        let analysis = match tokio::time::timeout(
            timeout_duration,
            self.analyze_topic(&context)
        ).await {
            Ok(result) => result?,
            Err(_) => {
                println!("⚠️ Analysis timed out, providing simplified response...");
                return Ok(format!(
                    "🌍 Market Overview\n\n\
                     📊 Current State:\n• Market Cap: ${:.2}B\n• 24h Volume: ${:.2}B\n\n\
                     ⚠️ Note: Detailed analysis timed out, showing basic metrics only.",
                    market_data.overview.total_market_cap / 1e9,
                    market_data.overview.total_volume / 1e9
                ));
            }
        };

        // Simplified response format
        Ok(format!(
            "🌍 Quick Market Topics Analysis\n\n\
             📊 Key Themes:\n{}\n\n\
             🚀 Catalysts:\n{}\n\n\
             ⚠️ Risks:\n{}\n",
            analysis.key_projects.iter().take(3).map(|s| format!("• {}", s)).collect::<Vec<_>>().join("\n"),
            analysis.catalysts.iter().take(3).map(|s| format!("• {}", s)).collect::<Vec<_>>().join("\n"),
            analysis.risks.iter().take(3).map(|s| format!("• {}", s)).collect::<Vec<_>>().join("\n")
        ))
    }

    // Helper to summarize market data
    fn summarize_market_data(&self, data: &MarketData) -> String {
        let overview = &data.overview;
        let trending_coins = &data.trending;

        // Format trending coins with proper string joining
        let trending_list = trending_coins
            .iter()
            .take(3)
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "Market Cap: ${:.2}B\n\
             24h Volume: ${:.2}B\n\
             24h Change: {:.2}%\n\
             Top Trending: {}\n\
             Market Sentiment: {}",
            overview.total_market_cap / 1e9,
            overview.total_volume / 1e9,
            overview.market_cap_change_percentage_24h,
            trending_list,
            if overview.market_cap_change_percentage_24h > 0.0 { "Bullish 📈" } else { "Bearish 📉" }
        )
    }
}

#[async_trait]
impl Agent for TopicAgent {
    fn name(&self) -> &str {
        &self.base.name
    }
    
    fn model(&self) -> &str {
        &self.base.model
    }
    
    async fn think(&mut self, market_data: &MarketData, previous_message: Option<String>) -> Result<String> {
        // Analyze each sector
        let sectors = vec![
            "AI & ML Tokens",
            "Layer 1",
            "layer 2"
           
        ];

        let mut full_analysis = String::new();
        
        for sector in sectors {
            match self.analyze_sector(sector, market_data).await {
                Ok(analysis) => {
                    full_analysis.push_str(&format!("\n\n🔍 {} Analysis:\n", sector));
                    full_analysis.push_str(&format!("Sentiment: {:.2}\n", analysis.sentiment));
                    full_analysis.push_str("Key Projects:\n");
                    for project in analysis.key_projects {
                        full_analysis.push_str(&format!("- {}\n", project));
                    }
                    full_analysis.push_str("\nUpcoming Catalysts:\n");
                    for catalyst in analysis.catalysts {
                        full_analysis.push_str(&format!("- {}\n", catalyst));
                    }
                }
                Err(e) => {
                    full_analysis.push_str(&format!("\n⚠️ Error analyzing {}: {}\n", sector, e));
                }
            }
        }

        // Save to memory
        self.base.memory.conversations.push(Conversation {
            timestamp: Utc::now(),
            market_data: market_data.clone(),
            technical_data: None,
            other_message: previous_message,
            response: full_analysis.clone(),
        });

        self.save_memory().await?;
        
        Ok(full_analysis)
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