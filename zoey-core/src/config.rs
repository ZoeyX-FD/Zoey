use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineSearch {
    pub query: String,
    pub max_results: i64,
    pub interval_minutes: u64,  // How often to search
    pub enable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicTimeline {
    pub topic_id: String,
    pub topic_name: String,
    pub max_results: i64,
    pub interval_minutes: u64,
    pub enable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitterConfig {
    pub enabled: bool,  // If true, use custom config; if false, use defaults
    
    // Timeline settings
    pub max_tweet_length: usize,
    pub max_history_tweets: i64,
    pub home_timeline_fetch_count: i64,
    pub mentions_fetch_count: i64,
    
    // Timeline and Search settings
    pub search_queries: Vec<TimelineSearch>,
    pub topic_timelines: Vec<TopicTimeline>,
    pub search_languages: Vec<String>,  // Filter by language
    pub exclude_retweets: bool,
    pub exclude_replies: bool,
    pub min_likes: Option<i32>,        // Filter by minimum likes
    pub min_retweets: Option<i32>,     // Filter by minimum retweets
    
    // Time intervals (in seconds)
    pub min_action_interval: u64,  // Minimum time between actions
    pub max_action_interval: u64,  // Maximum time between actions
    pub min_task_interval: u64,    // Minimum time between tasks
    pub max_task_interval: u64,    // Maximum time between tasks
    
    // Interaction settings
    pub enable_likes: bool,
    pub enable_retweets: bool,
    pub enable_quotes: bool,
    pub enable_replies: bool,
    
    // Rate limiting
    pub max_tweets_per_hour: u32,
    pub max_likes_per_hour: u32,
    pub max_retweets_per_hour: u32,
}

impl Default for TwitterConfig {
    fn default() -> Self {
        Self {
            enabled: false,  // By default, use default settings
            
            // Default conservative settings
            max_tweet_length: 280,
            max_history_tweets: 10,
            home_timeline_fetch_count: 1,
            mentions_fetch_count: 5,
            
            // Default empty search settings
            search_queries: vec![],  // No default searches
            topic_timelines: vec![], // No default topics
            search_languages: vec!["en".to_string()],
            exclude_retweets: false,
            exclude_replies: false,
            min_likes: None,        // No minimum likes by default
            min_retweets: None,     // No minimum retweets by default
            
            // Default intervals
            min_action_interval: 60,   // 1 minute
            max_action_interval: 180,  // 3 minutes
            min_task_interval: 900,    // 15 minutes
            max_task_interval: 3600,   // 1 hour
            
            // Default features all enabled
            enable_likes: true,
            enable_retweets: true,
            enable_quotes: true,
            enable_replies: true,
            
            // Default rate limits
            max_tweets_per_hour: 5,
            max_likes_per_hour: 20,
            max_retweets_per_hour: 10,
        }
    }
} 