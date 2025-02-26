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
use chrono::Timelike;
use crypto_agents::agents::{ModelProvider, BaseAgent};


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
struct UserInsights {
    influence_score: f64,
    engagement_rate: f64,
    posting_patterns: PostingPatterns,
    key_topics: Vec<String>,
    notable_connections: Vec<Connection>,
    information_value: InformationValue,
}

#[derive(Debug, Serialize, Deserialize)]
struct PostingPatterns {
    peak_hours: Vec<u32>,
    most_active_days: Vec<String>,
    tweet_frequency: f64,
    reply_rate: f64,
    retweet_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Connection {
    username: String,
    interaction_count: i32,
    relationship_type: String, // e.g., "frequently_mentioned", "regular_engagement", "potential_insider"
    last_interaction: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct InformationValue {
    alpha_score: f64,  // How often they share valuable "alpha"
    accuracy_rate: f64, // Historical accuracy of their calls/predictions
    insider_probability: f64, // Likelihood of having insider knowledge
    unique_insights: Vec<String>, // Notable unique insights shared
}

#[derive(Debug, Serialize, Deserialize)]
struct ContentAnalysis {
    top_hashtags: HashMap<String, i32>,
    top_mentions: HashMap<String, i32>,
    sentiment_scores: Vec<f64>,
    avg_sentiment: f64,
    key_topics: Vec<String>,
    common_phrases: HashMap<String, i32>,
    token_mentions: Vec<TokenMention>,
    ai_analysis: String,
    user_insights: UserInsights,
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
}

impl Default for ExtractSettings {
    fn default() -> Self {
        Self {
            max_tweets: 100,
            since_date: None,
            include_replies: true,
            include_retweets: true,
        }
    }
}

impl ExtractSettings {
    fn new(max_tweets: i32, days_ago: Option<i64>, _mode: Option<String>) -> Self {
        Self {
            max_tweets,
            since_date: days_ago.map(|days| Utc::now() - Duration::days(days)),
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

struct TwitterAnalyzer {
    base: BaseAgent,
}

impl TwitterAnalyzer {
    async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        let preamble = "You are an expert Twitter Analyst. Analyze user activity and provide insights about this user focusing on:
        1. Key topics and themes
        2. Market sentiment
        3. Notable projects mentioned
        4. secret information related to crypto projects
        5. Trading strategies discussed
        6. Example engagement tweets
        7. Overall engagement patterns";

        let base = BaseAgent::new(
            "Twitter Analyzer".to_string(),
            model,
            preamble.to_string(),
            provider
        ).await?;

        Ok(Self { base })
    }

    async fn analyze(&self, text: &str) -> Result<String> {
        self.base.generate_response(text, None).await
    }

    async fn extract_tokens(&self, text: &str) -> Result<Vec<String>> {
        let mut tokens = Vec::new();
        
        // Split text into words and analyze each word
        for word in text.split_whitespace() {
            let word = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '$' && c != '#');
            
            // Check for $SYMBOL pattern
            if word.starts_with('$') && word.len() > 1 {
                let symbol = word[1..].trim_matches(|c: char| !c.is_alphanumeric());
                if !symbol.is_empty() {
                    tokens.push(symbol.to_uppercase());
                }
            }
            // Check for #TOKEN pattern
            else if word.starts_with('#') && word.len() > 1 {
                let token = word[1..].trim_matches(|c: char| !c.is_alphanumeric());
                if !token.is_empty() {
                    tokens.push(token.to_uppercase());
                }
            }
            // Check for standalone token patterns (BTC, ETH, etc.)
            else if word.chars().all(|c| c.is_ascii_alphanumeric()) && word.len() >= 2 && word.len() <= 5 {
                let upper_word = word.to_uppercase();
                if ["BTC", "ETH", "SOL", "USDT", "BNB"].contains(&upper_word.as_str()) {
                    tokens.push(upper_word);
                }
            }
        }
        
        // Remove duplicates while preserving order
        let mut unique_tokens = Vec::new();
        for token in tokens {
            if !unique_tokens.contains(&token) {
                unique_tokens.push(token);
            }
        }
        
        Ok(unique_tokens)
    }

    async fn analyze_user_patterns(&self, tweets: &[TweetData]) -> PostingPatterns {
        let mut hour_counts = HashMap::new();
        let mut day_counts = HashMap::new();
        let mut reply_count = 0;
        let mut retweet_count = 0;

        for tweet in tweets {
            if let Some(date) = tweet.created_at.as_ref() {
                if let Ok(parsed_date) = DateTime::parse_from_rfc3339(date) {
                    let hour = parsed_date.hour();
                    let day = parsed_date.format("%A").to_string();
                    
                    *hour_counts.entry(hour).or_insert(0) += 1;
                    *day_counts.entry(day).or_insert(0) += 1;
                }
            }
            
            // Count replies and retweets
            if tweet.content.as_ref().map_or(false, |c| c.starts_with('@')) {
                reply_count += 1;
            }
            if tweet.content.as_ref().map_or(false, |c| c.to_lowercase().contains("rt @")) {
                retweet_count += 1;
            }
        }

        // Get peak hours (top 3)
        let mut peak_hours: Vec<_> = hour_counts.into_iter().collect();
        peak_hours.sort_by(|a, b| b.1.cmp(&a.1));
        let peak_hours: Vec<u32> = peak_hours.into_iter().take(3).map(|(h, _)| h).collect();

        // Get most active days
        let mut active_days: Vec<_> = day_counts.into_iter().collect();
        active_days.sort_by(|a, b| b.1.cmp(&a.1));
        let most_active_days: Vec<_> = active_days.into_iter().take(3).map(|(d, _)| d).collect();

        PostingPatterns {
            peak_hours,
            most_active_days,
            tweet_frequency: tweets.len() as f64 / 30.0, // tweets per day assuming 30 day period
            reply_rate: reply_count as f64 / tweets.len() as f64,
            retweet_rate: retweet_count as f64 / tweets.len() as f64,
        }
    }

    async fn analyze_connections(&self, tweets: &[TweetData]) -> Vec<Connection> {
        let mut interactions = HashMap::new();
        let mut last_interactions = HashMap::new();

        for tweet in tweets {
            if let Some(content) = &tweet.content {
                // Extract mentions
                let mentions: Vec<_> = content
                    .split_whitespace()
                    .filter(|word| word.starts_with('@'))
                    .map(|word| word.trim_matches(|c| !char::is_alphanumeric(c)))
                    .collect();

                for mention in mentions {
                    *interactions.entry(mention.to_string()).or_insert(0) += 1;
                    last_interactions.insert(mention.to_string(), tweet.created_at.clone().unwrap_or_default());
                }
            }
        }

        let mut connections: Vec<Connection> = interactions
            .into_iter()
            .map(|(username, count)| {
                let relationship_type = if count > 10 {
                    "frequently_mentioned"
                } else if count > 5 {
                    "regular_engagement"
                } else {
                    "occasional_interaction"
                };

                Connection {
                    username: username.clone(),
                    interaction_count: count,
                    relationship_type: relationship_type.to_string(),
                    last_interaction: last_interactions.get(&username).cloned().unwrap_or_default(),
                }
            })
            .collect();

        connections.sort_by(|a, b| b.interaction_count.cmp(&a.interaction_count));
        connections.into_iter().take(10).collect()
    }

    async fn calculate_information_value(&self, tweets: &[TweetData]) -> InformationValue {
        let mut alpha_count = 0;
        let mut prediction_count = 0;
        let mut unique_insights = Vec::new();

        for tweet in tweets {
            if let Some(content) = &tweet.content {
                let lower_content = content.to_lowercase();
                
                // Check for alpha indicators
                if lower_content.contains("alpha") || 
                   lower_content.contains("leaked") || 
                   lower_content.contains("insider") ||
                   lower_content.contains("exclusive") {
                    alpha_count += 1;
                }

                // Check for predictions and use the count
                if lower_content.contains("predict") || 
                   lower_content.contains("expect") ||
                   lower_content.contains("soon") ||
                   lower_content.contains("incoming") {
                    prediction_count += 1;
                }

                // Extract unique insights
                if content.len() > 100 && 
                   (lower_content.contains("thread") || 
                    lower_content.contains("1/") ||
                    lower_content.contains("analysis")) {
                    unique_insights.push(content.clone());
                }
            }
        }

        // Use prediction_count in calculating accuracy_rate
        let accuracy_rate = if prediction_count > 0 {
            // Simple heuristic: more predictions = potentially lower accuracy
            (1.0 / (prediction_count as f64)).min(0.8)
        } else {
            0.0
        };

        InformationValue {
            alpha_score: alpha_count as f64 / tweets.len() as f64,
            accuracy_rate,
            insider_probability: if alpha_count > 5 { 0.7 } else { 0.3 },
            unique_insights: unique_insights.into_iter().take(5).collect(),
        }
    }
}

async fn analyze_user_profile(tweets: &[TweetData], stats: &TweetStats, username: &str) -> Result<ContentAnalysis> {
    let analyzer = TwitterAnalyzer::new(
        "deepseek-r1:1.5b-qwen-distill-q8_0".to_string(),
        ModelProvider::Ollama
    ).await?;
 
    // Analyze user patterns and connections
    let posting_patterns = analyzer.analyze_user_patterns(tweets).await;
    let notable_connections = analyzer.analyze_connections(tweets).await;
    let information_value = analyzer.calculate_information_value(tweets).await;

    // Calculate influence score
    let influence_score = (stats.total_likes + stats.total_retweets * 2) as f64 / 
                         (tweets.len() as f64 * 100.0);
    
    // Calculate engagement rate
    let engagement_rate = if stats.total_views > 0 {
        (stats.total_likes + stats.total_retweets) as f64 / stats.total_views as f64 * 100.0
    } else {
        0.0
    };

    let user_insights = UserInsights {
        influence_score,
        engagement_rate,
        posting_patterns,
        key_topics: extract_key_topics(tweets),
        notable_connections,
        information_value,
    };

    // Create analysis prompt
    let prompt = format!(
        "You are an expert Twitter Analyst. Analyze this Twitter user's activity and provide insights.\n\n\
        User: @{}\n\
        Stats:\n\
        - Total tweets analyzed: {}\n\
        - Total engagement: {} likes, {} retweets\n\
        - Average views per tweet: {}\n\
        \nTop Hashtags: {}\n\
        Top Mentions: {}\n\
        \nTweet Sample:\n{}\n",
        username,
        stats.total_tweets,
        stats.total_likes,
        stats.total_retweets,
        if stats.total_tweets > 0 { stats.total_views / stats.total_tweets as i32 } else { 0 },
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
        tweets.iter()
            .filter_map(|t| t.content.clone())
            .take(50)
            .collect::<Vec<_>>()
            .join("\n\n")
    );

    // Get AI analysis
    let ai_analysis = analyzer.analyze(&prompt).await?;

    // Extract tokens from tweets
    let mut token_mentions = HashMap::new();
    let mut token_contexts = HashMap::new();

    for tweet in tweets {
        if let Some(content) = &tweet.content {
            if let Ok(tokens) = analyzer.extract_tokens(content).await {
                for token in tokens {
                    *token_mentions.entry(token.clone()).or_insert(0) += 1;
                    token_contexts.entry(token)
                        .or_insert_with(Vec::new)
                        .push(content.clone());
                }
            }
        }
    }

    // Process token mentions
    let mut token_mentions: Vec<TokenMention> = token_mentions.into_iter()
        .map(|(symbol, count)| {
            let contexts = token_contexts.get(&symbol)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .take(3)
                .collect();
            
            TokenMention {
                symbol: symbol.clone(),
                count,
                context: contexts,
            }
        })
        .collect();

    token_mentions.sort_by(|a, b| b.count.cmp(&a.count));
    let token_mentions = token_mentions.into_iter().take(10).collect();

    Ok(ContentAnalysis {
        top_hashtags: stats.top_hashtags.clone(),
        top_mentions: stats.top_mentions.clone(),
        sentiment_scores: calculate_sentiment_scores(tweets),
        avg_sentiment: calculate_avg_sentiment(tweets),
        key_topics: extract_key_topics(tweets),
        common_phrases: extract_common_phrases(tweets),
        token_mentions,
        ai_analysis,
        user_insights,
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

fn print_analysis_summary(user_data: &UserData) {
    println!("\n📊 Analysis for @{}:", user_data.username);
    println!("📅 Analysis Date: {}", user_data.extracted_at);
    
    println!("\n📈 Metrics:");
    println!("  • Total tweets: {}", user_data.metrics.total_tweets);
    println!("  • Total likes: {}", user_data.metrics.total_likes);
    println!("  • Total retweets: {}", user_data.metrics.total_retweets);
    println!("  • Total views: {}", user_data.metrics.total_views);
    println!("  • Engagement rate: {:.2}%", user_data.metrics.avg_engagement_rate);

    // Get date range of tweets
    let dates: Vec<DateTime<Utc>> = user_data.tweets.iter()
        .filter_map(|t| t.created_at.as_ref())
        .filter_map(|d| DateTime::parse_from_rfc3339(d).ok())
        .map(|d| d.with_timezone(&Utc))
        .collect();
    
    if let (Some(earliest), Some(latest)) = (dates.iter().min(), dates.iter().max()) {
        println!("\n📅 Tweet Date Range:");
        println!("  • Earliest: {}", earliest.format("%Y-%m-%d %H:%M UTC"));
        println!("  • Latest: {}", latest.format("%Y-%m-%d %H:%M UTC"));
    }

    println!("\n🔍 AI Analysis:");
    println!("{}", user_data.analysis.ai_analysis);

    println!("\n#️⃣ Top Hashtags:");
    for (tag, count) in user_data.analysis.top_hashtags.iter().take(5) {
        println!("  • #{}: {} times", tag, count);
    }

    println!("\n👥 Top Mentions:");
    for (user, count) in user_data.analysis.top_mentions.iter().take(5) {
        println!("  • @{}: {} times", user, count);
    }

    println!("\n📝 Key Topics:");
    for topic in &user_data.analysis.key_topics {
        println!("  • {}", topic);
    }

    println!("\n🎭 Sentiment Analysis:");
    println!("  • Average sentiment: {:.2}", user_data.analysis.avg_sentiment);

    println!("\n💰 Token Mentions:");
    for token in &user_data.analysis.token_mentions {
        println!("  • {} (mentioned {} times)", token.symbol, token.count);
        println!("    Example contexts with dates:");
        for (i, context) in token.context.iter().take(2).enumerate() {
            // Find the tweet that contains this context to get its date
            if let Some(tweet) = user_data.tweets.iter()
                .find(|t| t.content.as_ref().map_or(false, |c| c == context)) {
                if let Some(date) = tweet.created_at.as_ref() {
                    if let Ok(parsed_date) = DateTime::parse_from_rfc3339(date) {
                        println!("    {}. [{}] {}", 
                            i + 1,
                            parsed_date.with_timezone(&Utc).format("%Y-%m-%d %H:%M UTC"),
                            context
                        );
                    } else {
                        println!("    {}. {}", i + 1, context);
                    }
                } else {
                    println!("    {}. {}", i + 1, context);
                }
            } else {
                println!("    {}. {}", i + 1, context);
            }
        }
    }

    println!("\n🔍 User Insights:");
    println!("  • Influence Score: {:.2}", user_data.analysis.user_insights.influence_score);
    println!("  • Engagement Rate: {:.2}%", user_data.analysis.user_insights.engagement_rate);
    
    println!("\n⏰ Posting Patterns:");
    println!("  • Peak Hours: {:?}", user_data.analysis.user_insights.posting_patterns.peak_hours);
    println!("  • Most Active Days: {:?}", user_data.analysis.user_insights.posting_patterns.most_active_days);
    println!("  • Tweet Frequency: {:.1} tweets/day", user_data.analysis.user_insights.posting_patterns.tweet_frequency);
    println!("  • Reply Rate: {:.1}%", user_data.analysis.user_insights.posting_patterns.reply_rate * 100.0);
    println!("  • Retweet Rate: {:.1}%", user_data.analysis.user_insights.posting_patterns.retweet_rate * 100.0);

    println!("\n🤝 Notable Connections:");
    for connection in &user_data.analysis.user_insights.notable_connections {
        println!("  • {} ({} interactions) - {}", 
            connection.username,
            connection.interaction_count,
            connection.relationship_type
        );
    }

    println!("\n💎 Information Value:");
    println!("  • Alpha Score: {:.2}", user_data.analysis.user_insights.information_value.alpha_score);
    println!("  • Insider Probability: {:.2}", user_data.analysis.user_insights.information_value.insider_probability);
    println!("\n  Notable Insights:");
    for (i, insight) in user_data.analysis.user_insights.information_value.unique_insights.iter().enumerate() {
        println!("  {}. {}", i + 1, insight);
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

    // Get initial analysis with token extraction
    let analysis = match analyze_user_profile(&tweets, &stats, username).await {
        Ok(analysis) => analysis,
        Err(e) => {
            println!("⚠️ Error generating analysis: {}", e);
            ContentAnalysis {
                top_hashtags: stats_clone.top_hashtags,
                top_mentions: stats_clone.top_mentions,
                sentiment_scores: calculate_sentiment_scores(&tweets),
                avg_sentiment: calculate_avg_sentiment(&tweets),
                key_topics: extract_key_topics(&tweets),
                common_phrases: extract_common_phrases(&tweets),
                token_mentions: Vec::new(),
                ai_analysis: "Analysis unavailable".to_string(),
                user_insights: UserInsights {
                    influence_score: 0.0,
                    engagement_rate: 0.0,
                    posting_patterns: PostingPatterns {
                        peak_hours: Vec::new(),
                        most_active_days: Vec::new(),
                        tweet_frequency: 0.0,
                        reply_rate: 0.0,
                        retweet_rate: 0.0,
                    },
                    key_topics: Vec::new(),
                    notable_connections: Vec::new(),
                    information_value: InformationValue {
                        alpha_score: 0.0,
                        accuracy_rate: 0.0,
                        insider_probability: 0.0,
                        unique_insights: Vec::new(),
                    },
                },
            }
        }
    };

    // Create user data with proper String conversion
    let user_data = UserData {
        username: username.to_string(),
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
    print_analysis_summary(&user_data);

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
