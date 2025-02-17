use anyhow::Result;
use crypto_agents::{
    agents::{ModelProvider, TopicAgent},
    api::coingecko::CoinGeckoClient,
};
use dotenv::dotenv;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    println!("🧪 Topic Analysis Test Suite");
    println!("===========================\n");

    // Initialize agents and clients
    let mut topic_agent = TopicAgent::new(
        "gemini-2.0-flash-exp".to_string(),
        ModelProvider::Gemini
    ).await?;

    let coingecko = CoinGeckoClient::new()?;

    // Test cases
    let test_cases = vec![
        ("AI & ML Tokens", "Testing AI sector analysis"),
        ("Layer 1", "Testing L1 blockchain analysis"),
        ("DeFi", "Testing DeFi sector analysis"),
        ("NFT & Gaming", "Testing NFT/Gaming sector analysis"),
        ("Meme Coins", "Testing meme coin sector analysis"),
    ];

    // Get market data once to use across all tests
    println!("📊 Fetching market data...");
    let market_data = coingecko.get_market_data().await?;
    let technical_data = coingecko.get_technical_analysis().await?;

    // Run test cases
    for (sector, description) in test_cases {
        println!("\n🔍 Test Case: {}", description);
        println!("Analyzing sector: {}", sector);

        // Test market topics analysis
        match topic_agent.analyze_market_topics(&market_data, &format!("{:?}", technical_data)).await {
            Ok(analysis) => {
                println!("\n✅ Market Topics Analysis Result:");
                println!("{}", analysis);
            }
            Err(e) => {
                println!("❌ Error in market topics analysis: {}", e);
            }
        }

        // Test sector-specific analysis
        match topic_agent.analyze_sector(sector, &market_data).await {
            Ok(analysis) => {
                println!("\n✅ Sector Analysis Result:");
                println!("Sentiment Score: {:.2}", analysis.sentiment);
                
                println!("\n📊 Key Projects:");
                for project in analysis.key_projects {
                    println!("• {}", project);
                }

                println!("\n🚀 Catalysts:");
                for catalyst in analysis.catalysts {
                    println!("• {}", catalyst);
                }

                println!("\n⚠️ Risks:");
                for risk in analysis.risks {
                    println!("• {}", risk);
                }

                println!("\n💡 Trading Implications:");
                for implication in analysis.trading_implications {
                    println!("• {}", implication);
                }
            }
            Err(e) => {
                println!("❌ Error in sector analysis: {}", e);
            }
        }

        // Add delay between tests to avoid rate limiting
        sleep(Duration::from_secs(5)).await;
    }

    println!("\n🏁 Test Suite Completed");
    Ok(())
} 