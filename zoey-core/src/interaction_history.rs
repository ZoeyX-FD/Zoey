use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_rusqlite::Connection;
use tracing::{debug, info};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionMetrics {
    pub tweet_id: String,
    pub timestamp: DateTime<Utc>,
    pub likes: i32,
    pub retweets: i32,
    pub quotes: i32,
    pub replies: i32,
    pub content: String,
    pub engagement_score: f32,
}

impl InteractionMetrics {
    pub fn new(tweet_id: String, content: String) -> Self {
        Self {
            tweet_id,
            content,
            timestamp: Utc::now(),
            likes: 0,
            retweets: 0,
            quotes: 0,
            replies: 0,
            engagement_score: 0.0,
        }
    }

    pub fn calculate_engagement_score(&mut self) {
        // Weighted scoring based on different engagement types
        self.engagement_score = 
            (self.likes as f32 * 1.0) +
            (self.retweets as f32 * 2.0) +
            (self.quotes as f32 * 2.5) +
            (self.replies as f32 * 1.5);
    }
}

#[derive(Clone)]
pub struct InteractionHistory {
    conn: Arc<Connection>,
}

impl InteractionHistory {
    pub async fn new(conn: Connection) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing InteractionHistory storage");
        let history = Self { 
            conn: Arc::new(conn)
        };
        history.init_db().await?;
        info!("InteractionHistory storage initialized successfully");
        Ok(history)
    }

    async fn init_db(&self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Creating/verifying interaction_history tables");
        self.conn.call(|conn| {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS interaction_history (
                    tweet_id TEXT PRIMARY KEY,
                    content TEXT NOT NULL,
                    timestamp TEXT NOT NULL,
                    likes INTEGER DEFAULT 0,
                    retweets INTEGER DEFAULT 0,
                    quotes INTEGER DEFAULT 0,
                    replies INTEGER DEFAULT 0,
                    engagement_score REAL DEFAULT 0.0
                )",
                (),
            )?;

            conn.execute(
                "CREATE TABLE IF NOT EXISTS tweet_interactions (
                    tweet_id TEXT PRIMARY KEY,
                    interaction_type TEXT NOT NULL,
                    timestamp TEXT NOT NULL,
                    UNIQUE(tweet_id, interaction_type)
                )",
                (),
            )?;
            debug!("Database tables created/verified successfully");
            Ok(())
        }).await?;
        Ok(())
    }

    pub async fn log_interaction(&self, metrics: InteractionMetrics) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            tweet_id = %metrics.tweet_id,
            likes = %metrics.likes,
            retweets = %metrics.retweets,
            quotes = %metrics.quotes,
            replies = %metrics.replies,
            score = %metrics.engagement_score,
            "Logging tweet interaction metrics"
        );
        
        let tweet_id = metrics.tweet_id.clone();
        let content = metrics.content.clone();
        let timestamp = metrics.timestamp.to_rfc3339();
        let likes = metrics.likes;
        let retweets = metrics.retweets;
        let quotes = metrics.quotes;
        let replies = metrics.replies;
        let score = metrics.engagement_score;

        self.conn.call(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO interaction_history 
                (tweet_id, content, timestamp, likes, retweets, quotes, replies, engagement_score)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                (
                    tweet_id,
                    content,
                    timestamp,
                    likes,
                    retweets,
                    quotes,
                    replies,
                    score,
                ),
            )?;
            debug!("Interaction metrics stored successfully");
            Ok(())
        }).await?;
        Ok(())
    }

    pub async fn get_top_performing_content(&self, limit: i64) -> Result<Vec<InteractionMetrics>, Box<dyn std::error::Error>> {
        debug!(limit = %limit, "Fetching top performing content");
        let result = self.conn.call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT * FROM interaction_history 
                ORDER BY engagement_score DESC 
                LIMIT ?"
            )?;
            
            let metrics = stmt.query_map([limit], |row| {
                Ok(InteractionMetrics {
                    tweet_id: row.get(0)?,
                    content: row.get(1)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                        .unwrap()
                        .with_timezone(&Utc),
                    likes: row.get(3)?,
                    retweets: row.get(4)?,
                    quotes: row.get(5)?,
                    replies: row.get(6)?,
                    engagement_score: row.get(7)?,
                })
            })?;

            let mut result = Vec::new();
            for metric in metrics {
                result.push(metric?);
            }
            debug!(count = %result.len(), "Retrieved top performing content");
            Ok(result)
        }).await?;
        
        Ok(result)
    }

    pub async fn generate_performance_insights(&self) -> Result<String, Box<dyn std::error::Error>> {
        let top_tweets = self.get_top_performing_content(5).await?;
        
        if top_tweets.is_empty() {
            return Ok("Not enough historical data yet to generate insights.".to_string());
        }

        let avg_engagement: f32 = top_tweets.iter()
            .map(|t| t.engagement_score)
            .sum::<f32>() / top_tweets.len() as f32;

        let insights = format!(
            "Based on recent performance (avg engagement score: {:.2}), successful tweets tend to:\n\
            1. Generate meaningful discussions (avg replies: {:.1})\n\
            2. Get shared frequently (avg retweets: {:.1})\n\
            Example of high-performing content: {}",
            avg_engagement,
            top_tweets.iter().map(|t| t.replies as f32).sum::<f32>() / top_tweets.len() as f32,
            top_tweets.iter().map(|t| t.retweets as f32).sum::<f32>() / top_tweets.len() as f32,
            top_tweets.first().map(|t| t.content.clone()).unwrap_or_default()
        );

        Ok(insights)
    }

    pub async fn has_interaction(&self, tweet_id: &str, interaction_type: &str) -> Result<bool, Box<dyn std::error::Error>> {
        debug!(
            tweet_id = %tweet_id,
            interaction_type = %interaction_type,
            "Checking for existing interaction"
        );
        
        let tweet_id_owned = tweet_id.to_string();
        let interaction_type_owned = interaction_type.to_string();
        
        let result = self.conn.call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT COUNT(*) FROM tweet_interactions 
                WHERE tweet_id = ? AND interaction_type = ?"
            )?;
            
            let count: i64 = stmt.query_row(
                (tweet_id_owned, interaction_type_owned),
                |row| row.get(0)
            )?;
            
            Ok(count > 0)
        }).await?;
        
        debug!(
            tweet_id = %tweet_id,
            interaction_type = %interaction_type,
            exists = %result,
            "Interaction check complete"
        );
        
        Ok(result)
    }

    pub async fn record_interaction(&self, tweet_id: &str, interaction_type: &str) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            tweet_id = %tweet_id,
            interaction_type = %interaction_type,
            "Recording new interaction"
        );
        
        let tweet_id_owned = tweet_id.to_string();
        let interaction_type_owned = interaction_type.to_string();
        let timestamp = Utc::now().to_rfc3339();

        self.conn.call(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO tweet_interactions 
                (tweet_id, interaction_type, timestamp)
                VALUES (?, ?, ?)",
                (tweet_id_owned, interaction_type_owned, timestamp),
            )?;
            Ok(())
        }).await?;
        
        debug!(
            tweet_id = %tweet_id,
            interaction_type = %interaction_type,
            "Interaction recorded successfully"
        );
        
        Ok(())
    }

    pub async fn get_pending_interactions(&self, tweet_id: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        debug!(tweet_id = %tweet_id, "Checking pending interactions");
        let interaction_types = vec!["like", "retweet", "quote"];
        let mut pending = Vec::new();
        
        for itype in interaction_types {
            if !self.has_interaction(tweet_id, itype).await? {
                pending.push(itype.to_string());
            }
        }
        
        debug!(
            tweet_id = %tweet_id,
            pending_count = %pending.len(),
            pending_types = ?pending,
            "Retrieved pending interactions"
        );
        
        Ok(pending)
    }
} 