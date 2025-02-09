use async_trait::async_trait;
use anyhow::Result;
use chrono::Utc;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;

use crate::models::{MarketData, Conversation};
use super::{Agent, BaseAgent, ModelProvider};
use crate::api::social_media::{SocialMediaClient, SocialMediaPost};

const SENTIMENT_SYSTEM_PROMPT: &str = r#"
You are the Sentiment Analysis Expert 📊
Your role is to analyze social media sentiment, news, and market psychology.

Focus on:
- Social media trends and discussions
- News sentiment analysis
- Market psychology and emotions
- Community engagement metrics
- Influencer impact analysis

Format your response like this:

🤖 Sentiment Analysis Report
==========================

📱 Social Media Pulse:
[Overall social sentiment summary]

📰 News Analysis:
- [Key news item 1 with sentiment]
- [Key news item 2 with sentiment]
- [Key news item 3 with sentiment]

🎭 Market Psychology:
[Current market psychology state]

📊 Sentiment Metrics:
- Twitter: [Positive/Negative/Neutral]
- Reddit: [Positive/Negative/Neutral]
- News: [Positive/Negative/Neutral]

⚠️ Risk Factors:
[Sentiment-based risks]

🔮 Sentiment Forecast:
[Short-term sentiment prediction]
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentMetrics {
    pub positive_ratio: f64,
    pub negative_ratio: f64,
    pub neutral_ratio: f64,
    pub volume_24h: i32,
}

pub struct SentimentAgent {
    base: BaseAgent,
    current_metrics: Vec<SentimentMetrics>,
    social_client: SocialMediaClient,
}

impl SentimentAgent {
    pub async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        Ok(Self {
            base: BaseAgent::new(
                "Sentiment Agent".to_string(),
                model,
                SENTIMENT_SYSTEM_PROMPT.to_string(),
                provider
            )
            .await?
            .with_temperature(0.7),
            current_metrics: Vec::new(),
            social_client: SocialMediaClient::new().await?,
        })
    }

    pub async fn update_metrics(&mut self, metrics: Vec<SentimentMetrics>) {
        self.current_metrics = metrics;
    }

    async fn gather_social_data(&self, market_data: &MarketData) -> Result<Vec<SocialMediaPost>> {
        let mut all_posts = Vec::new();
        
        // Get sentiment for BTC and ETH
        for symbol in &["BTC", "ETH"] {
            let mut posts = self.social_client.get_twitter_sentiment(symbol).await?;
            all_posts.append(&mut posts);
        }

        // Get sentiment for trending coins
        for coin in &market_data.trending {
            let mut posts = self.social_client.get_twitter_sentiment(&coin.symbol).await?;
            all_posts.append(&mut posts);
        }

        Ok(all_posts)
    }

    async fn analyze_sentiment_data(&self, market_data: &MarketData) -> Result<String> {
        let mut context = String::new();

        // Add market overview
        context.push_str(&format!("\nMarket Overview:\n"));
        context.push_str(&format!("Total Market Cap: ${:.2}B\n", market_data.overview.total_market_cap / 1_000_000_000.0));
        context.push_str(&format!("24h Volume: ${:.2}B\n", market_data.overview.total_volume / 1_000_000_000.0));

        // Gather Twitter data
        println!("📱 Gathering Twitter data...");
        let social_posts = self.gather_social_data(market_data).await?;

        // Calculate sentiment metrics
        let mut metrics = SentimentMetrics {
            positive_ratio: 0.0,
            negative_ratio: 0.0,
            neutral_ratio: 0.0,
            volume_24h: 0,
        };

        for post in &social_posts {
            metrics.volume_24h += 1;

            if let Some(sentiment) = post.sentiment_score {
                if sentiment > 0.3 {
                    metrics.positive_ratio += 1.0;
                } else if sentiment < -0.3 {
                    metrics.negative_ratio += 1.0;
                } else {
                    metrics.neutral_ratio += 1.0;
                }
            }
        }

        // Normalize metrics
        let total = metrics.volume_24h as f64;
        if total > 0.0 {
            metrics.positive_ratio /= total;
            metrics.negative_ratio /= total;
            metrics.neutral_ratio /= total;
        }

        // Add Twitter metrics to context
        context.push_str("\nTwitter Sentiment Analysis:\n");
        context.push_str(&format!(
            "Total Tweets: {}\nPositive: {:.1}%\nNegative: {:.1}%\nNeutral: {:.1}%\n",
            metrics.volume_24h,
            metrics.positive_ratio * 100.0,
            metrics.negative_ratio * 100.0,
            metrics.neutral_ratio * 100.0
        ));

        // Add sample tweets
        context.push_str("\nSample Tweets:\n");
        for post in social_posts.iter().take(5) {
            let sentiment = match post.sentiment_score {
                Some(s) if s > 0.3 => "positive",
                Some(s) if s < -0.3 => "negative",
                _ => "neutral"
            };
            context.push_str(&format!(
                "- {} ({})\n  Engagement: {} likes\n",
                post.content,
                sentiment,
                post.engagement
            ));
        }

        // Generate analysis using the AI model
        self.base.generate_response(
            "Analyze the current market sentiment on Twitter and provide a detailed report.",
            Some(&context)
        ).await
    }
}

#[async_trait]
impl Agent for SentimentAgent {
    fn name(&self) -> &str {
        &self.base.name
    }
    
    fn model(&self) -> &str {
        &self.base.model
    }
    
    async fn think(&mut self, market_data: &MarketData, previous_message: Option<String>) -> Result<String> {
        // Analyze sentiment and generate response
        let sentiment_analysis = self.analyze_sentiment_data(market_data).await?;
        
        // Save to memory
        self.base.memory.conversations.push(Conversation {
            timestamp: Utc::now(),
            market_data: market_data.clone(),
            technical_data: None,
            other_message: previous_message,
            response: sentiment_analysis.clone(),
        });
        
        self.save_memory().await?;
        
        Ok(sentiment_analysis)
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