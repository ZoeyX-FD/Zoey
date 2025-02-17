use async_trait::async_trait;
use anyhow::Result;
use chrono::{Utc, Duration};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;

use crate::models::{MarketData, Conversation};
use crate::api::{
    coingecko::DetailedCoinData,
    social_media::SocialMediaPost,
};
use super::{Agent, BaseAgent, ModelProvider};
use common::exa::{ExaClient, ExaSearchParams, Contents, Highlights, Summary};

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
    exa_client: Option<ExaClient>,
}

impl TopicAgent {
    pub async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        // Initialize Exa client if API key is available
        let exa_client = std::env::var("EXA_API_KEY").ok().map(|key| ExaClient::new(&key));
        
        Ok(Self {
            base: BaseAgent::new(
                "Topic Analysis Agent".to_string(),
                model,
                TOPIC_SYSTEM_PROMPT.to_string(),
                provider
            ).await?.with_temperature(0.7),
            analysis_history: HashMap::new(),
            exa_client,
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
            key_projects: self.extract_key_projects(&response),
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
        if let Some(section) = text.split("Key Market Themes:").nth(1) {
            if let Some(end) = section.find("\n\n") {
                let lines = section[..end].lines();
                for line in lines {
                    let line = line.trim();
                    if line.starts_with('-') || line.starts_with('•') {
                        projects.push(line.trim_start_matches(['-', '•', ' ']).to_string());
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
                    let line = line.trim();
                    if line.starts_with('-') || line.starts_with('•') {
                        catalysts.push(line.trim_start_matches(['-', '•', ' ']).to_string());
                    }
                }
            }
        }
        catalysts
    }

    fn extract_risks(&self, text: &str) -> Vec<String> {
        let mut risks = Vec::new();
        if let Some(section) = text.split("Risk Factors:").nth(1) {
            if let Some(end) = section.find("\n\n") {
                let lines = section[..end].lines();
                for line in lines {
                    let line = line.trim();
                    if line.starts_with('-') || line.starts_with('•') {
                        risks.push(line.trim_start_matches(['-', '•', ' ']).to_string());
                    }
                }
            }
        }
        risks
    }

    fn extract_trading_implications(&self, text: &str) -> Vec<String> {
        let mut implications = Vec::new();
        if let Some(section) = text.split("Trading Implications:").nth(1) {
            if let Some(end) = section.find("\n\n") {
                let lines = section[..end].lines();
                for line in lines {
                    let line = line.trim();
                    if line.starts_with('-') || line.starts_with('•') {
                        implications.push(line.trim_start_matches(['-', '•', ' ']).to_string());
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

    pub async fn analyze_market_topics(
        &self,
        market_data: &MarketData,
        _technical_analysis: &str,
    ) -> Result<String> {
        let mut analysis = String::new();
        
        // Add market overview header
        analysis.push_str(&format!(
            "🌍 Market Topics Analysis ({})\n\n\
             📊 Market Overview:\n\
             • Total Market Cap: ${:.2}B\n\
             • 24h Volume: ${:.2}B\n\
             • Market Cap Change 24h: {:.2}%\n\n",
            Utc::now().format("%Y-%m-%d %H:%M UTC"),
            market_data.overview.total_market_cap / 1_000_000_000.0,
            market_data.overview.total_volume / 1_000_000_000.0,
            market_data.overview.market_cap_change_percentage_24h
        ));

        if let Some(client) = &self.exa_client {
            let contents = Contents {
                text: true,
                highlights: Some(Highlights {
                    num_sentences: 3,
                    highlights_per_result: 3,
                }),
                summary: Some(Summary {
                    max_sentences: 5,
                }),
            };

            let search_params = ExaSearchParams {
                query: "cryptocurrency market latest news trends developments price analysis".to_string(),
                num_results: 10,
                include_domains: vec![
                    "beincrypto.com".to_string(),
                ],
                start_date: Some(Utc::now() - Duration::hours(24)),
                end_date: None,
                contents: Some(contents),
            };

            match client.search_crypto(search_params).await {
                Ok(results) => {
                    if !results.is_empty() {
                        let mut content_for_analysis = String::new();
                        let mut sources = Vec::new();
                        
                        for result in &results {
                            // Store source information
                            sources.push(format!(
                                "• {} ({})\n  🔗 {}", 
                                result.title,
                                result.published_date
                                    .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                                    .unwrap_or_else(|| "Recent".to_string()),
                                result.url
                            ));
                            
                            // Prepare content for analysis
                            content_for_analysis.push_str(&format!(
                                "\n📰 {}\n",
                                result.title
                            ));
                            
                            if let Some(summary) = &result.summary {
                                content_for_analysis.push_str(&format!("Summary: {}\n", summary));
                            }
                            
                            if !result.highlights.is_empty() {
                                content_for_analysis.push_str("Key Points:\n");
                                for highlight in &result.highlights {
                                    content_for_analysis.push_str(&format!("• {}\n", highlight));
                                }
                            }
                        }

                        let prompt = format!(
                            "Analyze these recent market developments and provide a concise summary:\n\n{}\n\n\
                             Format the analysis as:\n\
                             1. Key Market Trends (3-4 points)\n\
                             2. Important Developments (2-3 points)\n\
                             3. Potential Market Impacts (2-3 points)",
                            content_for_analysis
                        );

                        if let Ok(ai_analysis) = self.base.generate_response(&prompt, None).await {
                            analysis.push_str(&ai_analysis);
                            
                            // Add sources section at the end
                            analysis.push_str("\n\n📚 Sources:\n");
                            for source in sources {
                                analysis.push_str(&format!("{}\n", source));
                            }
                        }
                    } else {
                        analysis.push_str("No recent market developments found.\n");
                    }
                }
                Err(e) => {
                    println!("⚠️ Error fetching market news: {}", e);
                    analysis.push_str("Error fetching market developments.\n");
                }
            }
        } else {
            analysis.push_str("Exa client not available for market analysis.\n");
        }

        Ok(analysis)
    }

    // Dynamic prompt generation based on analysis type
    fn generate_search_prompt(&self, analysis_type: &str, context: &str) -> ExaSearchParams {
        let base_contents = Contents {
            text: true,
            highlights: Some(Highlights {
                num_sentences: 3,
                highlights_per_result: 3,
            }),
            summary: Some(Summary {
                max_sentences: 5,
            }),
        };

        match analysis_type {
            "news" => ExaSearchParams {
                query: format!("cryptocurrency {} latest news developments", context),
                num_results: 5,
                include_domains: vec![
                    "cointelegraph.com".to_string(),
                    "coindesk.com".to_string(),
                    "theblock.co".to_string(),
                ],
                start_date: Some(Utc::now() - Duration::days(7)),
                end_date: None,
                contents: Some(base_contents.clone()),
            },
            "technical" => ExaSearchParams {
                query: format!("{} price analysis market prediction technical indicators", context),
                num_results: 5,
                include_domains: vec![
                    "tradingview.com".to_string(),
                    "investing.com".to_string(),
                ],
                start_date: Some(Utc::now() - Duration::days(2)),
                end_date: None,
                contents: Some(base_contents.clone()),
            },
            "sentiment" => ExaSearchParams {
                query: format!("{} market sentiment social media community reaction", context),
                num_results: 5,
                include_domains: vec![
                    "cointelegraph.com".to_string(),
                    "coindesk.com".to_string(),
                ],
                start_date: Some(Utc::now() - Duration::days(1)),
                end_date: None,
                contents: Some(base_contents.clone()),
            },
            "development" => ExaSearchParams {
                query: format!("{} development github updates progress technical", context),
                num_results: 5,
                include_domains: vec![
                    "github.com".to_string(),
                    "medium.com".to_string(),
                ],
                start_date: Some(Utc::now() - Duration::days(30)),
                end_date: None,
                contents: Some(base_contents.clone()),
            },
            _ => ExaSearchParams {
                query: format!("cryptocurrency {} analysis", context),
                num_results: 5,
                include_domains: vec![
                    "cointelegraph.com".to_string(),
                    "coindesk.com".to_string(),
                ],
                start_date: Some(Utc::now() - Duration::days(7)),
                end_date: None,
                contents: Some(base_contents),
            },
        }
    }

    async fn get_comprehensive_analysis(&self, symbol: &str) -> Result<String> {
        if let Some(client) = &self.exa_client {
            let analysis_types = vec!["news", "technical", "sentiment", "development"];
            let mut all_results = Vec::new();

            for analysis_type in analysis_types {
                let params = self.generate_search_prompt(analysis_type, symbol);
                match client.search_crypto(params).await {
                    Ok(results) => {
                        all_results.push((analysis_type, results));
                    },
                    Err(e) => {
                        println!("⚠️ Error fetching {} data: {}", analysis_type, e);
                    }
                }
            }

            let mut context = String::new();
            for (analysis_type, results) in all_results {
                context.push_str(&format!("\n📊 {} Analysis:\n", analysis_type.to_uppercase()));
                for result in results.iter().take(2) {
                    let content = result.text.as_ref()
                        .or(result.summary.as_ref())
                        .map(|s| s.chars().take(200).collect::<String>())
                        .unwrap_or_else(|| "No content available".to_string());
                    
                    context.push_str(&format!("• {}\n  {}\n  {}\n",
                        result.title,
                        result.published_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default(),
                        content
                    ));
                }
            }

            Ok(context)
        } else {
            Ok("No data available - Exa client not initialized".to_string())
        }
    }

    async fn analyze_with_news(&self, symbol: &str) -> Result<TopicAnalysis> {
        // Get comprehensive analysis
        let analysis_context = self.get_comprehensive_analysis(symbol).await?;

        let mut context = String::new();
        context.push_str(&analysis_context);

        // Use this enriched context in your analysis
        let prompt = format!(
            "Analyze {} with the following market data and news:\n\n{}",
            symbol, context
        );

        // Generate analysis using the enriched context
        let response = self.base.generate_response(&prompt, None).await?;
        
        // Parse the response into TopicAnalysis
        Ok(TopicAnalysis {
            timestamp: Utc::now().to_rfc3339(),
            sector: format!("{} Analysis", symbol),
            sentiment: self.calculate_sector_sentiment(&response),
            key_projects: self.extract_key_projects(&response),
            catalysts: self.extract_catalysts(&response),
            risks: self.extract_risks(&response),
            trading_implications: self.extract_trading_implications(&response),
        })
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
        
        println!("🔍 Starting sector analysis with news integration...");
        
        for sector in sectors {
            match self.analyze_with_news(sector).await {
                Ok(analysis) => {
                    full_analysis.push_str(&format!("\n\n🔍 {} Analysis:\n", sector));
                    full_analysis.push_str(&format!("Sentiment: {:.2}\n", analysis.sentiment));
                    
                    // Get comprehensive news and market data
                    if let Some(news_data) = self.get_comprehensive_analysis(sector).await.ok() {
                        full_analysis.push_str("\n📰 Latest Market Context:\n");
                        full_analysis.push_str(&news_data);
                    }
                    
                    full_analysis.push_str("\nKey Projects:\n");
                    for project in analysis.key_projects {
                        full_analysis.push_str(&format!("- {}\n", project));
                    }
                    
                    full_analysis.push_str("\nUpcoming Catalysts:\n");
                    for catalyst in analysis.catalysts {
                        full_analysis.push_str(&format!("- {}\n", catalyst));
                    }
                    
                    full_analysis.push_str("\n💡 Trading Implications:\n");
                    for implication in analysis.trading_implications {
                        full_analysis.push_str(&format!("- {}\n", implication));
                    }
                }
                Err(e) => {
                    full_analysis.push_str(&format!("\n⚠️ Error analyzing {}: {}\n", sector, e));
                }
            }
            
            // Add delay between sector analyses
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
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