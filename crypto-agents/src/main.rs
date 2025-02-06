use anyhow::Result;
use crypto_agents::MultiAgentSystem;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();
    
    // Print startup banner
    println!("🚀 Starting Crypto Agents Research System");
    println!("=================================");
    
    // Initialize and run the system
    let mut system = MultiAgentSystem::new().await?;
    
    println!("\n✅ System initialized successfully!");
    println!("\n🔄 Starting research rounds...\n");
    
    // Run the system
    system.run().await?;
    
    Ok(())
} 