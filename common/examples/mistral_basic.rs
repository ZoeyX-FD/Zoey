use anyhow::Result;
use common::providers::{MistralClient, MISTRAL_MEDIUM};
use rig::completion::Prompt;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();
    
    println!("🤖 Basic Mistral AI Agent Example");
    println!("=================================\n");

    // Initialize Mistral client
    let client = MistralClient::from_env()?;
    
    // Create an agent with the Mistral model
    let agent = client.agent(MISTRAL_MEDIUM)
        .preamble("You are a helpful AI assistant specialized in cryptocurrency and trading analysis.")
        .temperature(0.7)
        .max_tokens(1000)
        .build();

    // Test the agent with a simple prompt
    let response = agent.prompt("Tell me about cryptocurrency trading.").await?;
    println!("Response: {}", response);

    Ok(())
} 