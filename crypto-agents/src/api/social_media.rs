use anyhow::Result;
use serde::{Deserialize, Serialize};
use agent_twitter_client::scraper::Scraper;
use std::env;
use crate::models::MarketData;
use std::fs;
use chrono::{Utc, DateTime, Timelike};
use serde_json::{json, Value};
use csv::Writer;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialMediaPost {
    pub content: String,
    pub timestamp: String,
    pub engagement: i32,
    pub sentiment_score: Option<f64>,
}

pub struct SocialMediaClient {
    scraper: Scraper,
}

impl SocialMediaClient {
    pub async fn new() -> Result<Self> {
        // Create scraper with async initialization
        let scraper = Scraper::new().await?;
        let mut client = Self { scraper };
        
        // Handle authentication
        client.authenticate().await?;
        
        Ok(client)
    }

    async fn authenticate(&mut self) -> Result<()> {
        // Try to authenticate using cookies first
        if let Ok(cookie_string) = env::var("TWITTER_COOKIE_STRING") {
            println!("🍪 Authenticating with cookies...");
            self.scraper.set_from_cookie_string(&cookie_string).await?;
        } 
        // Fall back to username/password if cookies aren't available
        else if let (Ok(username), Ok(password)) = (
            env::var("TWITTER_USERNAME"),
            env::var("TWITTER_PASSWORD")
        ) {
            println!("🔑 Logging in with credentials...");
            self.scraper.login(
                username,
                password,
                None,
                None
            ).await?;
            
            // Save the cookies for future use
            if let Ok(new_cookies) = self.scraper.get_cookie_string().await {
                println!("💾 New cookies generated. Set TWITTER_COOKIE_STRING to:");
                println!("{}", new_cookies);
            }
        } else {
            println!("⚠️ No authentication method available - need either TWITTER_COOKIE_STRING or (TWITTER_USERNAME and TWITTER_PASSWORD)");
        }

        Ok(())
    }

    pub async fn get_twitter_sentiment(&self, symbol: &str) -> Result<Vec<SocialMediaPost>> {
        const MAX_RETRIES: u32 = 3;
        const RETRY_DELAY: Duration = Duration::from_secs(2);
        
        for attempt in 0..MAX_RETRIES {
            match self.try_get_twitter_sentiment(symbol).await {
                Ok(posts) => return Ok(posts),
                Err(e) => {
                    if attempt < MAX_RETRIES - 1 {
                        println!("⚠️ Error fetching tweets for {}: {}. Retrying...", symbol, e);
                        sleep(RETRY_DELAY).await;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!("Max retries exceeded for {}", symbol))
    }

    // Enhanced sentiment analysis
    fn analyze_sentiment(&self, text: &str) -> f64 {
        let positive_words = [
            "bullish", "moon", "up", "gain", "profit", "buy", "good", "great",
            "excellent", "positive", "success", "winning", "confident", "secure",
            "long", "strong", "rise", "rising", "higher", "support", "breakthrough",
            "breakout", "momentum", "rally", "growth", "accumulate", "hold", "hodl"
        ];
        
        let negative_words = [
            "bearish", "down", "loss", "sell", "bad", "worse", "negative",
            "fail", "risk", "crash", "dump", "poor", "weak", "uncertain",
            "short", "resistance", "lower", "falling", "dip", "bear", "correction",
            "decline", "drop", "fear", "panic", "capitulate", "liquidate"
        ];

        let lowercase_text = text.to_lowercase();
        let words: Vec<&str> = lowercase_text.split_whitespace().collect();
        let mut score: f64 = 0.0;

        for word in words {
            if positive_words.contains(&word) {
                score += 1.0;
            }
            if negative_words.contains(&word) {
                score -= 1.0;
            }
        }

        // Normalize score between -1 and 1
        if score != 0.0 {
            score / score.abs().max(1.0)
        } else {
            0.0
        }
    }

    // Add new function to save data
    async fn save_sentiment_data(&self, symbol: &str, posts: &[SocialMediaPost], format: &str) -> Result<()> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let data_dir = "data/sentiment_logs";
        
        // Create directories if they don't exist
        fs::create_dir_all(format!("{}/{}", data_dir, symbol))?;
        
        // Calculate additional metrics
        let metrics = calculate_metrics(posts);
        
        match format.to_lowercase().as_str() {
            "json" => {
                let file_path = format!("{}/{}/{}_sentiment.json", data_dir, symbol, timestamp);
                let json_data = json!({
                    "symbol": symbol,
                    "timestamp": timestamp,
                    "total_posts": posts.len(),
                    "posts": posts,
                    "metrics": {
                        "average_engagement": metrics.avg_engagement,
                        "max_engagement": metrics.max_engagement,
                        "sentiment_distribution": metrics.sentiment_dist,
                        "hourly_distribution": metrics.hourly_dist,
                        "engagement_percentiles": metrics.engagement_percentiles,
                    }
                });
                fs::write(file_path, serde_json::to_string_pretty(&json_data)?)?;
            },
            "csv" => {
                let file_path = format!("{}/{}/{}_sentiment.csv", data_dir, symbol, timestamp);
                let mut wtr = Writer::from_path(file_path)?;
                
                // Write header
                wtr.write_record(&["timestamp", "content", "engagement", "sentiment_score"])?;
                
                // Write data
                for post in posts {
                    wtr.write_record(&[
                        &post.timestamp,
                        &post.content,
                        &post.engagement.to_string(),
                        &post.sentiment_score.unwrap_or(0.0).to_string()
                    ])?;
                }
                wtr.flush()?;
            },
            _ => println!("⚠️ Unsupported format: {}", format)
        }
        
        Ok(())
    }
}

// Move Metrics struct outside of impl block
#[derive(Debug)]
struct Metrics {
    avg_engagement: f64,
    max_engagement: i32,
    sentiment_dist: HashMap<String, i32>,
    hourly_dist: HashMap<i32, i32>,
    engagement_percentiles: Vec<i32>,
}

// Move calculate_metrics function outside of impl block
fn calculate_metrics(posts: &[SocialMediaPost]) -> Metrics {
    let mut metrics = Metrics {
        avg_engagement: 0.0,
        max_engagement: 0,
        sentiment_dist: HashMap::new(),
        hourly_dist: HashMap::new(),
        engagement_percentiles: Vec::new(),
    };
    
    if posts.is_empty() {
        return metrics;
    }
    
    // Calculate metrics
    for post in posts {
        metrics.max_engagement = metrics.max_engagement.max(post.engagement);
        
        // Categorize sentiment
        let sentiment = match post.sentiment_score {
            Some(s) if s > 0.3 => "positive",
            Some(s) if s < -0.3 => "negative",
            _ => "neutral",
        };
        *metrics.sentiment_dist.entry(sentiment.to_string()).or_insert(0) += 1;
        
        // Add hourly distribution
        if let Ok(dt) = DateTime::parse_from_rfc3339(&post.timestamp) {
            *metrics.hourly_dist.entry(dt.hour() as i32).or_insert(0) += 1;
        }
    }
    
    // Calculate average
    metrics.avg_engagement = posts.iter().map(|p| p.engagement as f64).sum::<f64>() / posts.len() as f64;
    
    // Calculate percentiles
    let mut engagements: Vec<i32> = posts.iter().map(|p| p.engagement).collect();
    engagements.sort_unstable();
    
    if engagements.len() >= 4 {
        metrics.engagement_percentiles = vec![
            engagements[engagements.len() * 25 / 100],  // 25th percentile
            engagements[engagements.len() * 50 / 100],  // median
            engagements[engagements.len() * 75 / 100],  // 75th percentile
        ];
    }
    
    metrics
}

impl SocialMediaClient {
    async fn try_get_twitter_sentiment(&self, symbol: &str) -> Result<Vec<SocialMediaPost>> {
        println!("🔄 Searching tweets for {}", symbol);
        
        // Get configuration from environment
        let languages = env::var("SENTIMENT_LANGUAGES")
            .unwrap_or_else(|_| "en".to_string());
        
        let include_retweets = env::var("SENTIMENT_INCLUDE_RETWEETS")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true";
        
        let min_engagement = env::var("SENTIMENT_MIN_ENGAGEMENT")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<i32>()
            .unwrap_or(10);

        // Build query with configuration
        let mut query = format!("#{} OR ${}", symbol, symbol);
        
        // Add language filter
        query.push_str(&format!(" lang:{}", languages.replace(',', " OR lang:")));
        
        // Exclude retweets if configured
        if !include_retweets {
            query.push_str(" -is:retweet");
        }

        let search_results = self.scraper.search_tweets(
            &query,
            20, // Increased count
            agent_twitter_client::search::SearchMode::Top,
            None
        ).await?;

        let mut posts = Vec::new();

        for tweet in search_results.tweets {
            if let (Some(text), Some(created_at)) = (tweet.text.clone(), tweet.created_at.clone()) {
                let mut post = SocialMediaPost {
                    content: text,
                    timestamp: created_at,
                    engagement: tweet.likes.unwrap_or(0) + 
                              tweet.retweets.unwrap_or(0) + 
                              tweet.replies.unwrap_or(0) +
                              tweet.quote_count.unwrap_or(0),
                    sentiment_score: None,
                };

                // Skip if post doesn't pass validation
                if !Self::clean_and_validate_post(&mut post) {
                    continue;
                }

                // Skip posts with low engagement
                if post.engagement < min_engagement {
                    continue;
                }

                post.sentiment_score = Some(self.analyze_sentiment(&post.content));
                posts.push(post);
            }
        }

        // Sort by engagement
        posts.sort_by(|a, b| b.engagement.cmp(&a.engagement));

        // Save the data in both formats
        self.save_sentiment_data(symbol, &posts, "json").await?;
        self.save_sentiment_data(symbol, &posts, "csv").await?;

        Ok(posts)
    }

    pub async fn gather_social_data(&self, market_data: &MarketData) -> Result<Vec<SocialMediaPost>> {
        let mut all_posts = Vec::new();
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        
        // Use HashSet to prevent duplicate symbols
        let mut processed_symbols = std::collections::HashSet::new();
        
        // Create summary file for this gathering session
        let summary_path = format!("data/sentiment_logs/summary_{}.json", timestamp);
        let mut summary = json!({
            "timestamp": timestamp,
            "symbols": {},
            "total_posts": 0,
            "analysis_start_time": Utc::now().to_rfc3339(),
        });
        
        // Get default targets from environment or use defaults
        let default_targets = env::var("SENTIMENT_DEFAULT_TARGETS")
            .unwrap_or_else(|_| "BTC,ETH".to_string());
        
        // Split by comma and trim whitespace
        let default_symbols: Vec<String> = default_targets
            .split(',')
            .map(|s| s.trim().to_uppercase())
            .collect();

        let total_symbols = default_symbols.len() + market_data.trending.len();
        let mut processed = 0;

        // Process symbols only if not already processed
        for symbol in default_symbols {
            processed += 1;
            println!("🔄 Progress: [{}/{}] Analyzing {}", processed, total_symbols, symbol);
            if processed_symbols.insert(symbol.clone()) {
                println!("🔍 Analyzing sentiment for {}", symbol);
                let mut posts = self.get_twitter_sentiment(&symbol).await?;
                
                // Update summary
                if let Value::Object(ref mut map) = summary["symbols"] {
                    map.insert(symbol.clone(), json!({
                        "posts_count": posts.len(),
                        "average_engagement": posts.iter().map(|p| p.engagement).sum::<i32>() as f64 / posts.len() as f64,
                        "average_sentiment": posts.iter().filter_map(|p| p.sentiment_score).sum::<f64>() / posts.len() as f64
                    }));
                }
                
                all_posts.append(&mut posts);
            }
        }

        // Get sentiment for trending coins if enabled
        if env::var("SENTIMENT_INCLUDE_TRENDING")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase() == "true" 
        {
            for coin in &market_data.trending {
                processed += 1;
                println!("🔄 Progress: [{}/{}] Analyzing {}", processed, total_symbols, coin.symbol);
                println!("🔥 Analyzing trending coin: {}", coin.symbol);
                let mut posts = self.get_twitter_sentiment(&coin.symbol).await?;
                all_posts.append(&mut posts);
            }
        }

        // Sort all posts by engagement
        all_posts.sort_by(|a, b| b.engagement.cmp(&a.engagement));

        // Limit total posts if specified
        if let Ok(limit) = env::var("SENTIMENT_MAX_POSTS")
            .and_then(|s| s.parse::<usize>().map_err(|_| std::env::VarError::NotPresent)) {
            all_posts.truncate(limit);
        }

        // Update total posts in summary
        summary["total_posts"] = json!(all_posts.len());
        
        // Save summary
        fs::write(summary_path, serde_json::to_string_pretty(&summary)?)?;

        // Get additional targets if specified
        if let Ok(additional_targets) = env::var("SENTIMENT_ADDITIONAL_TARGETS") {
            let additional_symbols: Vec<String> = additional_targets
                .split(',')
                .map(|s| s.trim().to_uppercase())
                .collect();

            for symbol in additional_symbols {
                processed += 1;
                println!("🔄 Progress: [{}/{}] Analyzing {}", processed, total_symbols, symbol);
                println!("🔍 Analyzing additional target: {}", symbol);
                let mut posts = self.get_twitter_sentiment(&symbol).await?;
                all_posts.append(&mut posts);
            }
        }

        // Sort all posts by engagement
        all_posts.sort_by(|a, b| b.engagement.cmp(&a.engagement));

        // Limit total posts if specified
        if let Ok(limit) = env::var("SENTIMENT_MAX_POSTS")
            .and_then(|s| s.parse::<usize>().map_err(|_| std::env::VarError::NotPresent)) {
            all_posts.truncate(limit);
        }

        Ok(all_posts)
    }

    fn clean_and_validate_post(post: &mut SocialMediaPost) -> bool {
        // Remove spam/bot indicators
        if post.content.contains("🤖") || post.content.matches("http").count() > 3 {
            return false;
        }
        
        // Clean text
        post.content = post.content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        
        // Validate engagement
        if post.engagement < 0 {
            post.engagement = 0;
        }
        
        true
    }
} 