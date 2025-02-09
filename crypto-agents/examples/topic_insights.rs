use anyhow::Result;
use crypto_agents::{
    agents::{ModelProvider, TopicAgent},
    api::social_media::SocialMediaClient,
};
use dotenv::dotenv;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TopicInsightReport {
    timestamp: DateTime<Utc>,
    topic: String,
    metrics: TopicMetrics,
    top_posts: Vec<PostData>,
    analysis: AnalysisData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TopicMetrics {
    total_posts: usize,
    total_engagement: i32,
    average_engagement: f64,
    sentiment_distribution: SentimentDistribution,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SentimentDistribution {
    positive: i32,
    neutral: i32,
    negative: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PostData {
    content: String,
    engagement: i32,
    sentiment: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AnalysisData {
    key_insights: Vec<String>,
    trends: Vec<String>,
    risks: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    println!("🔍 Crypto Topic Analysis System");
    println!("===============================");

    // Create data directory if it doesn't exist
    let data_dir = Path::new("data/topic_insights");
    fs::create_dir_all(data_dir)?;

    // Initialize agents
    let topic_agent = TopicAgent::new(
        "mistralai/mistral-nemo".to_string(),
        ModelProvider::OpenRouter
    ).await?;

    let social = SocialMediaClient::new().await?;

    let topics = vec![
        "artificial intelligence",
        "agent ai",
        "TAO",
        "bittensor",
    ];

    let mut reports = Vec::new();

    for topic in topics {
        println!("\n📊 Analyzing Topic: {}", topic);

        // Get relevant Twitter data
        let search_terms = match topic {
            "DeFi" => vec!["#DeFi", "defi protocol", "yield farming"],
            "NFT" => vec!["#NFT", "nft project", "digital art"],
            "Layer1" => vec!["#L1", "blockchain", "layer1"],
            "GameFi" => vec!["#GameFi", "blockchain gaming", "p2e"],
            "AI Crypto" => vec!["#AICrypto", "ai tokens", "artificial intelligence crypto"],
            "ZK Rollups" => vec!["#zk", "zero knowledge", "zkrollup"],
            "Meme Coins" => vec!["#memecoin", "meme token", "doge"],
            "Web3 Social" => vec!["#Web3", "social token", "decentralized social"],
            _ => vec![topic],
        };

        let mut all_posts = Vec::new();
        for term in search_terms {
            match social.get_twitter_sentiment(term).await {
                Ok(mut posts) => all_posts.append(&mut posts),
                Err(e) => println!("⚠️ Error fetching data for {}: {}", term, e),
            }
        }

        if all_posts.is_empty() {
            println!("⚠️ No data found for topic: {}", topic);
            continue;
        }

        // Calculate engagement metrics
        let total_engagement: i32 = all_posts.iter().map(|p| p.engagement).sum();
        let avg_engagement = total_engagement as f64 / all_posts.len() as f64;

        // Calculate sentiment distribution
        let mut positive = 0;
        let mut negative = 0;
        let mut neutral = 0;
        for post in &all_posts {
            match post.sentiment_score {
                Some(score) if score > 0.3 => positive += 1,
                Some(score) if score < -0.3 => negative += 1,
                _ => neutral += 1,
            }
        }

        println!("\n📈 Engagement Metrics:");
        println!("Total Posts: {}", all_posts.len());
        println!("Total Engagement: {}", total_engagement);
        println!("Average Engagement: {:.1}", avg_engagement);

        println!("\n🎭 Sentiment Distribution:");
        println!("Positive: {}%", (positive * 100) / all_posts.len());
        println!("Neutral: {}%", (neutral * 100) / all_posts.len());
        println!("Negative: {}%", (negative * 100) / all_posts.len());

        // Get top posts by engagement
        all_posts.sort_by(|a, b| b.engagement.cmp(&a.engagement));
        
        println!("\n🔝 Top Posts:");
        for post in all_posts.iter().take(3) {
            println!("\n📱 Post: {}", post.content);
            println!("💫 Engagement: {}", post.engagement);
            if let Some(sentiment) = post.sentiment_score {
                println!("🎭 Sentiment: {:.2}", sentiment);
            }
        }

        // Generate topic insights
        let context = format!(
            "Topic: {}\nTotal Posts: {}\nSentiment: {}% positive, {}% neutral, {}% negative\n\nTop Posts:\n{}",
            topic,
            all_posts.len(),
            (positive * 100) / all_posts.len(),
            (neutral * 100) / all_posts.len(),
            (negative * 100) / all_posts.len(),
            all_posts.iter().take(5).map(|p| p.content.clone()).collect::<Vec<_>>().join("\n\n")
        );

        println!("\n🤖 AI Analysis:");

        // Create initial report structure
        let mut report = TopicInsightReport {
            timestamp: Utc::now(),
            topic: topic.to_string(),
            metrics: TopicMetrics {
                total_posts: all_posts.len(),
                total_engagement,
                average_engagement: avg_engagement,
                sentiment_distribution: SentimentDistribution {
                    positive: ((positive as f64 / all_posts.len() as f64) * 100.0) as i32,
                    neutral: ((neutral as f64 / all_posts.len() as f64) * 100.0) as i32,
                    negative: ((negative as f64 / all_posts.len() as f64) * 100.0) as i32,
                },
            },
            top_posts: all_posts.iter().take(3).map(|post| PostData {
                content: post.content.clone(),
                engagement: post.engagement,
                sentiment: post.sentiment_score,
            }).collect(),
            analysis: AnalysisData {
                key_insights: Vec::new(),
                trends: Vec::new(),
                risks: Vec::new(),
            },
        };

        // Get AI analysis and update the report
        if let Ok(analysis) = topic_agent.analyze_topic(&context).await {
            // Display analysis in console with emojis and formatting
            println!("\n🧠 AI Analysis Results:");
            println!("====================");

            println!("\n💡 Key Insights:");
            for insight in &analysis.key_projects {
                println!("  • {}", insight);
            }

            println!("\n📈 Current Trends:");
            for trend in &analysis.catalysts {
                println!("  • {}", trend);
            }

            println!("\n⚠️ Risks and Challenges:");
            for risk in &analysis.risks {
                println!("  • {}", risk);
            }

            // Update report after displaying
            report.analysis = AnalysisData {
                key_insights: analysis.key_projects.clone(),
                trends: analysis.catalysts.clone(),
                risks: analysis.risks.clone(),
            };
            reports.push(report.clone());

            println!("\n📊 Sentiment Overview:");
            println!("  • Positive: {}%", report.metrics.sentiment_distribution.positive);
            println!("  • Neutral:  {}%", report.metrics.sentiment_distribution.neutral);
            println!("  • Negative: {}%", report.metrics.sentiment_distribution.negative);

        } else {
            println!("⚠️ Failed to generate AI analysis");
        }

        // Save individual report
        let filename = format!(
            "data/topic_insights/{}_report_{}.json",
            topic.to_lowercase().replace(' ', "_"),
            Utc::now().format("%Y%m%d_%H%M%S")
        );
        fs::write(
            &filename,
            serde_json::to_string_pretty(&report)?
        )?;
        println!("💾 Saved report to {}", filename);

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    // Save combined report
    let combined_filename = format!(
        "data/topic_insights/combined_report_{}.json",
        Utc::now().format("%Y%m%d_%H%M%S")
    );
    fs::write(
        &combined_filename,
        serde_json::to_string_pretty(&reports)?
    )?;
    println!("\n📁 Saved combined report to {}", combined_filename);

    Ok(())
} 