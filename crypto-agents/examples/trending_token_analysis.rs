use anyhow::Result;
use crypto_agents::agents::{NewTopAgent, ModelProvider};
use dotenv::dotenv;
use tokio::time::{sleep, Duration};
use chrono::Utc;

const UPDATE_INTERVAL: u64 = 30 * 60; // 30 minutes in seconds

fn format_duration(seconds: i64) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{:02}:{:02}", minutes, seconds)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();
    
    println!("🚀 Starting New & Top Coins Analysis");
    println!("====================================");
    println!("ℹ️ Update interval: {} minutes", UPDATE_INTERVAL / 60);
    
    // Initialize agent with Mistral model
    let mut agent = NewTopAgent::new(
        "mistral-large-latest".to_string(),
        ModelProvider::Mistral
    ).await?;
    
    println!("✅ Agent initialized successfully!");
    
    loop {
        let start_time = Utc::now();
        let next_update = start_time + chrono::Duration::seconds(UPDATE_INTERVAL as i64);
        
        println!("\n🔄 Starting analysis cycle at {}...\n", start_time.format("%Y-%m-%d %H:%M:%S"));
        
        // Run analysis cycle
        if let Err(e) = agent.run_analysis_cycle().await {
            println!("⚠️ Error during analysis cycle: {}", e);
        }
        
        println!("\n✨ Analysis complete! Check data/market_analysis/analysis_results.csv for results.");
        println!("⏰ Next update at: {}", next_update.format("%Y-%m-%d %H:%M:%S"));
        
        // Calculate time to wait
        let now = Utc::now();
        let wait_duration = next_update - now;
        let wait_seconds = wait_duration.num_seconds();
        
        if wait_seconds > 0 {
            println!("💤 Waiting {} minutes {} seconds until next update...", 
                wait_seconds / 60,
                wait_seconds % 60
            );
            
            // Sleep until next update, showing countdown every minute
            let mut remaining = wait_seconds;
            while remaining > 0 {
                sleep(Duration::from_secs(60)).await;
                remaining -= 60;
                if remaining > 0 {
                    println!("⏳ Time until next update: {}", format_duration(remaining));
                }
            }
        }
    }
} 