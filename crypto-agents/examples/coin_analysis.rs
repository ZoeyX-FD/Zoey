use anyhow::Result;
use crypto_agents::{
    agents::{ModelProvider, TopicAgent},
    api::{coingecko::CoinGeckoClient, social_media::SocialMediaClient},
};
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();
    
    println!("🔄 Initializing Analysis...");
    
    // Initialize with Gemini
    let mut agent = TopicAgent::new(
        "gemini-2.0-flash-exp".to_string(),
        ModelProvider::Gemini
    ).await?;
    
    let coingecko = CoinGeckoClient::new()?;
    let social = SocialMediaClient::new().await?;
    
    // Focus on major Layer 1s and trending tokens
    let coins = vec![
        ("BERA", "berachain-bera"),
        ("PLUME", "plume"),
        ("ARC", "ai-rig-complex"),
    ];
    
    for (symbol, id) in coins {
        println!("\n📊 Analyzing {}", symbol);
        
        // Use get_detailed_coin_data
        let coin_data = match coingecko.get_detailed_coin_data(id).await {
            Ok(data) => data,
            Err(e) => {
                println!("⚠️ Failed to get market data for {}: {}", symbol, e);
                continue;
            }
        };
        
        // Get social sentiment with error handling
        let sentiment = match social.get_twitter_sentiment(symbol).await {
            Ok(data) => data.to_vec(),
            Err(e) => {
                println!("⚠️ Failed to get social data for {}: {}", symbol, e);
                continue;
            }
        };
        
        if sentiment.is_empty() {
            println!("⚠️ No social data found for {}", symbol);
            continue;
        }
        
        // Enhanced prompt for better analysis
        let analysis = agent.analyze_coin_with_sentiment(
            symbol,
            &coin_data,
            &sentiment
        ).await?;
        
        // Print comprehensive analysis
        println!("\n🎯 {} ({}) Analysis:", symbol, coin_data.name);
        println!("Overall Sentiment: {:.2}", analysis.sentiment);
        
        println!("\n📈 Market Stats:");
        println!("{}", agent.format_market_data(&coin_data));
        
        println!("\n🐦 Social Media Activity:");
        println!("Total Tweets Analyzed: {}", sentiment.len());
        
        let total_engagement: i32 = sentiment.iter().map(|p| p.engagement).sum();
        let avg_engagement = total_engagement as f64 / sentiment.len() as f64;
        println!("Total Engagement: {}", total_engagement);
        println!("Average Engagement: {:.1}", avg_engagement);
        
        // Sentiment distribution
        let mut positive = 0;
        let mut negative = 0;
        let mut neutral = 0;
        for post in &sentiment {
            match post.sentiment_score {
                Some(score) if score > 0.3 => positive += 1,
                Some(score) if score < -0.3 => negative += 1,
                _ => neutral += 1,
            }
        }
        
        println!("\n🎭 Sentiment Distribution:");
        println!("Positive: {}%", (positive * 100) / sentiment.len());
        println!("Neutral: {}%", (neutral * 100) / sentiment.len());
        println!("Negative: {}%", (negative * 100) / sentiment.len());
        
        if !analysis.key_projects.is_empty() {
            println!("\n💡 Key Points:");
            for point in &analysis.key_projects {
                println!("- {}", point);
            }
        }
        
        if !analysis.catalysts.is_empty() {
            println!("\n🚀 Growth Catalysts:");
            for catalyst in &analysis.catalysts {
                println!("- {}", catalyst);
            }
        }
        
        if !analysis.risks.is_empty() {
            println!("\n⚠️ Risk Factors:");
            for risk in &analysis.risks {
                println!("- {}", risk);
            }
        }
        
        // Sample tweets
        println!("\n📱 Sample Tweets:");
        for (i, post) in sentiment.iter().take(3).enumerate() {
            println!("{}. {} (Engagement: {})", i+1, post.content, post.engagement);
        }
        
        // Add delay between requests
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
    
    Ok(())
} 
