use crypto_agents::api::social_media::SocialMediaClient;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenv().ok();
    
    // Print the token length for debugging (don't print the actual token)
    if let Ok(cookie_string) = std::env::var("TWITTER_COOKIE_STRING") {
        println!("📝 Found Twitter cookie string (length: {})", cookie_string.len());
    } else {
        println!("❌ No Twitter cookie string found in environment!");
        return Ok(());
    }
    
    // Initialize client with async/await
    let client = SocialMediaClient::new().await?;
    
    // Test with BTC
    println!("🔍 Fetching BERA tweets...");
    match client.get_twitter_sentiment("BERA").await {
        Ok(tweets) => {
            println!("\n📊 Found {} tweets", tweets.len());
            
            if tweets.is_empty() {
                println!("⚠️ No tweets found!");
                return Ok(());
            }
            
            // Display sample tweets with sentiment
            println!("\n📝 Sample tweets (sorted by engagement):");
            for tweet in tweets.iter().take(5) {
                let sentiment = match tweet.sentiment_score {
                    Some(s) if s > 0.3 => "positive 📈",
                    Some(s) if s < -0.3 => "negative 📉",
                    _ => "neutral ➖",
                };
                
                println!("\n🐦 Tweet: {}", tweet.content);
                println!("💭 Sentiment: {}", sentiment);
                println!("📊 Total Engagement: {}", tweet.engagement);
                println!("⏰ Posted: {}", tweet.timestamp);
                println!("---");
            }
        },
        Err(e) => {
            println!("❌ Error fetching tweets: {}", e);
        }
    }
    
    Ok(())
} 
