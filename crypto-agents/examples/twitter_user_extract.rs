use anyhow::Result;
use agent_twitter_client::scraper::Scraper;
use dotenv::dotenv;
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;
use chrono::Utc;
use std::collections::HashMap;
use chrono::DateTime;
use chrono::Duration;
use crypto_agents::agents::{ModelProvider, TopicAgent, ExtractorAgent};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TweetData {
    id: Option<String>,
    content: Option<String>,
    created_at: Option<String>,
    likes: Option<i32>,
    retweets: Option<i32>,
    views: Option<i32>,
    hashtags: Vec<String>,
    mentions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserData {
    username: String,
    extracted_at: String,
    tweets: Vec<TweetData>,
    metrics: UserMetrics,
    analysis: ContentAnalysis,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TweetStats {
    total_tweets: usize,
    total_likes: i32,
    total_retweets: i32,
    total_views: i32,
    top_hashtags: HashMap<String, i32>,
    top_mentions: HashMap<String, i32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserMetrics {
    total_tweets: usize,
    total_likes: i32,
    total_retweets: i32,
    total_views: i32,
    avg_engagement_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContentAnalysis {
    top_hashtags: HashMap<String, i32>,
    top_mentions: HashMap<String, i32>,
    sentiment_scores: Vec<f64>,
    avg_sentiment: f64,
    key_topics: Vec<String>,
    common_phrases: HashMap<String, i32>,
    ai_summary: Option<AISummary>,  // Optional because it's only present in detailed mode
}

#[derive(Debug, Serialize, Deserialize)]
struct AISummary {
    topics: Vec<String>,
    catalysts: Vec<String>,
    risks: Vec<String>,
    sentiment: String,
    sector: String,
    token_mentions: Vec<TokenMention>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenMention {
    symbol: String,
    count: i32,
    context: Vec<String>,
}

#[derive(Debug, Clone)]
struct ExtractSettings {
    max_tweets: i32,
    since_date: Option<DateTime<Utc>>,
    include_replies: bool,
    include_retweets: bool,
    analysis_mode: AnalysisMode,
}

#[derive(Debug, Clone)]
enum AnalysisMode {
    Quick,
    Detailed,
}

impl Default for ExtractSettings {
    fn default() -> Self {
        Self {
            max_tweets: 100,
            since_date: None,
            include_replies: true,
            include_retweets: true,
            analysis_mode: AnalysisMode::Quick,
        }
    }
}

impl ExtractSettings {
    fn new(max_tweets: i32, days_ago: Option<i64>, mode: Option<String>) -> Self {
        Self {
            max_tweets,
            since_date: days_ago.map(|days| Utc::now() - Duration::days(days)),
            analysis_mode: match mode.as_deref() {
                Some("ai") => AnalysisMode::Detailed,
                _ => AnalysisMode::Quick,
            },
            ..Default::default()
        }
    }
}

fn calculate_stats(tweets: &[TweetData]) -> TweetStats {
    let mut hashtag_counts = HashMap::new();
    let mut mention_counts = HashMap::new();
    let mut total_likes = 0;
    let mut total_retweets = 0;
    let mut total_views = 0;

    for tweet in tweets {
        total_likes += tweet.likes.unwrap_or(0);
        total_retweets += tweet.retweets.unwrap_or(0);
        total_views += tweet.views.unwrap_or(0);

        // Count hashtags
        for tag in &tweet.hashtags {
            *hashtag_counts.entry(tag.clone()).or_insert(0) += 1;
        }

        // Count mentions
        for mention in &tweet.mentions {
            *mention_counts.entry(mention.clone()).or_insert(0) += 1;
        }
    }

    TweetStats {
        total_tweets: tweets.len(),
        total_likes,
        total_retweets,
        total_views,
        top_hashtags: hashtag_counts,
        top_mentions: mention_counts,
    }
}

async fn analyze_user_profile(tweets: &[TweetData], stats: &TweetStats, username: &str) -> Result<AISummary> {
    let topic_agent = TopicAgent::new(
        "google/gemini-2.0-flash-001".to_string(),
        ModelProvider::OpenRouter
    ).await?;

    let extractor_agent = ExtractorAgent::new(
        "mistral-small-latest".to_string(),
        ModelProvider::Mistral
    ).await?;

    // Convert tweets to string
    let tweet_texts: Vec<String> = tweets.iter()
        .filter_map(|t| t.content.clone())
        .take(50)
        .collect();
    
    let tweet_sample = tweet_texts.join("\n\n");

    // Extract tokens from all tweets
    let mut token_mentions = HashMap::new();
    let mut token_contexts = HashMap::new();

    for tweet in tweets {
        if let Some(content) = &tweet.content {
            if let Ok(tokens) = extractor_agent.extract_tokens(content).await {
                for token in tokens {
                    *token_mentions.entry(token.clone()).or_insert(0) += 1;
                    token_contexts.entry(token)
                        .or_insert_with(Vec::new)
                        .push(content.clone());
                }
            }
        }
    }

    // Convert to sorted vec of TokenMention
    let mut token_mentions: Vec<TokenMention> = token_mentions.into_iter()
        .map(|(symbol, count)| {
            let contexts = token_contexts.get(&symbol)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .take(3)  // Keep only 3 example contexts per token
                .collect();
                
            TokenMention {
                symbol: symbol.clone(),  // Clone here before moving into struct
                count,
                context: contexts,
            }
        })
        .collect();

    token_mentions.sort_by(|a, b| b.count.cmp(&a.count));
    let token_mentions: Vec<TokenMention> = token_mentions.into_iter().take(10).collect();

    // Add token information to the prompt
    let prompt = format!(
        "Analyze this Twitter user @{} based on their recent activity:\n\
         \nTweet Stats:\
         \n- Total tweets: {}\
         \n- Total likes: {}\
         \n- Total retweets: {}\
         \n- Total views: {}\
         \nTop Hashtags: {}\
         \nTop Mentions: {}\
         \nToken Mentions: {}\
         \n\nRecent Tweets Sample:\n{}\
         \n\nProvide analysis focusing on:\
         \n1. Key topics and themes\
         \n2. Market catalysts and opportunities\
         \n3. Potential risks and challenges\
         \n4. Overall sentiment and tone\
         \n5. Main sector/area of expertise\
         \n6. Token analysis and trading activity",
        username,
        stats.total_tweets,
        stats.total_likes,
        stats.total_retweets,
        stats.total_views,
        stats.top_hashtags.iter()
            .take(5)
            .map(|(tag, count)| format!("#{} ({})", tag, count))
            .collect::<Vec<_>>()
            .join(", "),
        stats.top_mentions.iter()
            .take(5)
            .map(|(user, count)| format!("@{} ({})", user, count))
            .collect::<Vec<_>>()
            .join(", "),
        token_mentions.iter()
            .map(|t| format!("{} ({} mentions)", t.symbol, t.count))
            .collect::<Vec<_>>()
            .join(", "),
        tweet_sample
    );

    let analysis = topic_agent.analyze_topic(&prompt).await?;

    Ok(AISummary {
        topics: analysis.key_projects,
        catalysts: analysis.catalysts,
        risks: analysis.risks,
        sentiment: format!("{:.2}", analysis.sentiment),
        sector: analysis.sector,
        token_mentions,
    })
}

fn calculate_sentiment_scores(tweets: &[TweetData]) -> Vec<f64> {
    tweets.iter()
        .filter_map(|t| t.content.as_ref())
        .map(|content| {
            let text = content.to_lowercase();
            let positive_words = ["bullish", "moon", "up", "gain", "profit", "buy", "good"];
            let negative_words = ["bearish", "down", "loss", "sell", "bad", "crash", "dump"];

            let positive_count = positive_words.iter()
                .filter(|&word| text.contains(word))
                .count() as f64;
            let negative_count = negative_words.iter()
                .filter(|&word| text.contains(word))
                .count() as f64;

            if positive_count + negative_count > 0.0 {
                (positive_count - negative_count) / (positive_count + negative_count)
            } else {
                0.0
            }
        })
        .collect()
}

fn calculate_avg_sentiment(tweets: &[TweetData]) -> f64 {
    let scores = calculate_sentiment_scores(tweets);
    if scores.is_empty() {
        0.0
    } else {
        scores.iter().sum::<f64>() / scores.len() as f64
    }
}

fn extract_key_topics(tweets: &[TweetData]) -> Vec<String> {
    let mut topic_counts = HashMap::new();
    let keywords = [
        "bitcoin", "ethereum", "defi", "nft", "layer2", "dao",
        "web3", "crypto", "blockchain", "token", "protocol", "ai"
    ];

    for tweet in tweets {
        if let Some(content) = &tweet.content {
            let text = content.to_lowercase();
            for keyword in keywords.iter() {
                if text.contains(keyword) {
                    *topic_counts.entry(keyword.to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    let mut topics: Vec<_> = topic_counts.into_iter().collect();
    topics.sort_by(|a, b| b.1.cmp(&a.1));
    topics.into_iter()
        .take(5)
        .map(|(topic, _)| topic)
        .collect()
}

fn extract_common_phrases(tweets: &[TweetData]) -> HashMap<String, i32> {
    let mut phrase_counts = HashMap::new();
    
    for tweet in tweets {
        if let Some(content) = &tweet.content {
            let words: Vec<&str> = content.split_whitespace().collect();
            for window in words.windows(3) {
                if window.len() == 3 {
                    let phrase = window.join(" ").to_lowercase();
                    *phrase_counts.entry(phrase).or_insert(0) += 1;
                }
            }
        }
    }

    // Keep only phrases that appear more than once
    phrase_counts.into_iter()
        .filter(|(_, count)| *count > 1)
        .collect()
}

fn print_analysis_summary(user_data: &UserData, _mode: &AnalysisMode) {
    println!("\n📊 Analysis for @{}:", user_data.username);
    println!("\nMetrics:");
    println!("  • Total tweets: {}", user_data.metrics.total_tweets);
    println!("  • Total likes: {}", user_data.metrics.total_likes);
    println!("  • Total retweets: {}", user_data.metrics.total_retweets);
    println!("  • Total views: {}", user_data.metrics.total_views);
    println!("  • Engagement rate: {:.2}%", user_data.metrics.avg_engagement_rate);

    println!("\nTop Hashtags:");
    for (tag, count) in user_data.analysis.top_hashtags.iter().take(5) {
        println!("  • #{}: {} times", tag, count);
    }

    println!("\nTop Mentions:");
    for (user, count) in user_data.analysis.top_mentions.iter().take(5) {
        println!("  • @{}: {} times", user, count);
    }

    println!("\nKey Topics:");
    for topic in &user_data.analysis.key_topics {
        println!("  • {}", topic);
    }

    println!("\nSentiment Analysis:");
    println!("  • Average sentiment: {:.2}", user_data.analysis.avg_sentiment);

    if let Some(ai) = &user_data.analysis.ai_summary {
        println!("\n🤖 AI Analysis:");
        println!("\nTopics:");
        for topic in &ai.topics {
            println!("  • {}", topic);
        }

        println!("\nCatalysts:");
        for catalyst in &ai.catalysts {
            println!("  • {}", catalyst);
        }

        println!("\nRisks:");
        for risk in &ai.risks {
            println!("  • {}", risk);
        }

        println!("\nSector: {}", ai.sector);
        println!("Sentiment: {}", ai.sentiment);

        println!("\n💰 Token Mentions:");
        for token in &ai.token_mentions {
            println!("  • {} (mentioned {} times)", token.symbol, token.count);
            println!("    Example contexts:");
            for context in token.context.iter().take(2) {
                println!("    - {}", context);
            }
        }
    }
}

// Add this new function to process a single user
async fn process_user(username: &str, settings: &ExtractSettings, scraper: &mut Scraper) -> Result<()> {
    println!("\n📍 Processing @{}", username);
    
    // 1. Follow the user
    if std::env::var("AUTO_FOLLOW").unwrap_or_default() == "true" {
        match scraper.follow_user(&username).await {
            Ok(_) => println!("✅ Successfully followed @{}", username),
            Err(e) => println!("⚠️ Follow error: {}", e),
        }
    }

    // 2. Get tweets with pagination
    let mut all_tweets = Vec::new();
    let mut cursor: Option<String> = None;
    let mut total_fetched = 0;

    println!("📥 Fetching tweets...");
    
    loop {
        match scraper.fetch_tweets_and_replies(&username, 100, cursor.as_deref()).await {
            Ok(response) => {
                let batch_size = response.tweets.len();
                if batch_size == 0 {
                    break;
                }

                total_fetched += batch_size;
                println!("  • Fetched {} tweets (Total: {})", batch_size, total_fetched);

                // Just clone the next cursor
                cursor = response.next.clone();
                
                // Convert and filter tweets
                let tweet_batch: Vec<TweetData> = response.tweets.iter()
                    .filter(|tweet| {
                        // Filter by date if specified
                        if let Some(since_date) = settings.since_date {
                            if let Some(tweet_date) = tweet.created_at.as_ref()
                                .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
                                .map(|d| d.with_timezone(&Utc)) {
                                if tweet_date < since_date {
                                    return false;
                                }
                            }
                        }

                        // Filter replies and retweets if needed
                        if !settings.include_replies && tweet.in_reply_to_status.is_some() {
                            return false;
                        }
                        if !settings.include_retweets && tweet.retweeted_status.is_some() {
                            return false;
                        }

                        true
                    })
                    .map(|tweet| TweetData {
                        id: tweet.id.clone(),
                        content: tweet.text.clone(),
                        created_at: tweet.created_at.clone(),
                        likes: tweet.likes,
                        retweets: tweet.retweets,
                        views: tweet.views,
                        hashtags: tweet.hashtags.clone(),
                        mentions: tweet.mentions.iter()
                            .filter_map(|m| m.username.clone())
                            .collect(),
                    })
                    .collect();

                all_tweets.extend(tweet_batch);

                // Check if we've reached the max tweets limit
                if settings.max_tweets > 0 && total_fetched >= settings.max_tweets as usize {
                    break;
                }

                // Check if there's no next page
                if cursor.is_none() {
                    break;
                }

                // Add a small delay between requests
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            },
            Err(e) => {
                println!("⚠️ Error fetching tweets: {}", e);
                break;
            }
        }
    }

    println!("✅ Fetched {} tweets in total", all_tweets.len());

    if all_tweets.is_empty() {
        println!("❌ No tweets found for @{}", username);
        return Ok(());
    }

    // Use all_tweets instead of tweets for the rest of the processing
    let tweets = all_tweets;
    println!("📝 Found {} tweets matching criteria", tweets.len());
            
    // Calculate stats
    let stats = calculate_stats(&tweets);
    let stats_clone = stats.clone();

    // Get AI analysis if in detailed mode
    let ai_summary = match settings.analysis_mode {
        AnalysisMode::Detailed => {
            match analyze_user_profile(&tweets, &stats, username).await {
                Ok(analysis) => Some(analysis),
                Err(e) => {
                    println!("⚠️ Error generating AI analysis: {}", e);
                    None
                }
            }
        },
        AnalysisMode::Quick => None,
    };

    // Create content analysis
    let analysis = ContentAnalysis {
        top_hashtags: stats_clone.top_hashtags,
        top_mentions: stats_clone.top_mentions,
        sentiment_scores: calculate_sentiment_scores(&tweets),
        avg_sentiment: calculate_avg_sentiment(&tweets),
        key_topics: extract_key_topics(&tweets),
        common_phrases: extract_common_phrases(&tweets),
        ai_summary,
    };

    // Create user data with proper String conversion
    let user_data = UserData {
        username: username.to_string(),  // Convert &str to String
        extracted_at: Utc::now().to_rfc3339(),
        tweets: tweets.clone(),
        metrics: UserMetrics {
            total_tweets: stats_clone.total_tweets,
            total_likes: stats_clone.total_likes,
            total_retweets: stats_clone.total_retweets,
            total_views: stats_clone.total_views,
            avg_engagement_rate: if stats_clone.total_views > 0 {
                (stats_clone.total_likes + stats_clone.total_retweets) as f64 
                    / stats_clone.total_views as f64 * 100.0
            } else {
                0.0
            },
        },
        analysis,
    };

    // Save everything to one file
    let filename = format!(
        "data/twitter_data/{}_data_{}.json",
        username.to_lowercase(),
        Utc::now().format("%Y%m%d_%H%M%S")
    );
    
    fs::write(&filename, serde_json::to_string_pretty(&user_data)?)?;
    println!("💾 Saved complete analysis to {}", filename);

    // Print summary
    print_analysis_summary(&user_data, &settings.analysis_mode);

    Ok(())
}

// Modify main function to handle multiple users
#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    println!("🤖 Twitter Follow & Extract");
    println!("==========================");

    // Get command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage:");
        println!("Single user:  cargo run --example twitter_follow_extract <username> [max_tweets] [days_ago] [mode]");
        println!("Multiple users from file: cargo run --example twitter_follow_extract --file users.txt [max_tweets] [days_ago] [mode]");
        println!("\nExamples:");
        println!("  Quick stats:  cargo run --example twitter_follow_extract aixbt_agent 200 7");
        println!("  AI analysis:  cargo run --example twitter_follow_extract aixbt_agent 200 7 ai");
        println!("  From file:    cargo run --example twitter_follow_extract --file users.txt 200 7 ai");
        return Ok(());
    }

    // Parse settings
    let (usernames, settings) = if args[1] == "--file" {
        // Read usernames from file
        let filename = args.get(2).expect("Please provide a filename");
        let content = fs::read_to_string(filename)?;
        let usernames: Vec<String> = content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();

        // Get other settings from remaining args
        let max_tweets = args.get(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);
        let days_ago = args.get(4)
            .and_then(|s| s.parse().ok());
        let mode = args.get(5).cloned();

        (usernames, ExtractSettings::new(max_tweets, days_ago, mode))
    } else {
        // Single username
        let username = args[1].clone();
        let max_tweets = args.get(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);
        let days_ago = args.get(3)
            .and_then(|s| s.parse().ok());
        let mode = args.get(4).cloned();

        (vec![username], ExtractSettings::new(max_tweets, days_ago, mode))
    };

    println!("Settings:");
    println!("- Users to process: {}", usernames.len());
    println!("- Max tweets per user: {}", settings.max_tweets);
    if let Some(since) = settings.since_date {
        println!("- Since date: {}", since.format("%Y-%m-%d"));
    }
    println!("- Mode: {}", match settings.analysis_mode {
        AnalysisMode::Quick => "Quick stats",
        AnalysisMode::Detailed => "Detailed AI analysis",
    });

    // Create data directory
    let data_dir = Path::new("data/twitter_data");
    fs::create_dir_all(data_dir)?;

    // Initialize scraper
    let mut scraper = Scraper::new().await?;
    
    // Authenticate
    let cookie_string = std::env::var("TWITTER_COOKIE_STRING")
        .expect("TWITTER_COOKIE_STRING must be set in .env");
    scraper.set_from_cookie_string(&cookie_string).await?;

    // Process each user
    for (i, username) in usernames.iter().enumerate() {
        println!("\n[{}/{}] Processing user", i + 1, usernames.len());
        if let Err(e) = process_user(username, &settings, &mut scraper).await {
            println!("⚠️ Error processing @{}: {}", username, e);
        }
        
        // Add delay between users
        if i < usernames.len() - 1 {
            println!("Waiting 2 seconds before next user...");
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    }

    println!("\n✅ Finished processing {} users", usernames.len());
    Ok(())
} 