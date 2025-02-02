use anyhow::Result;
use common::providers::{MistralClient, MISTRAL_LARGE};
use dotenv::dotenv;
use serde_json::json;
use rig::completion::Prompt;
use rig::agent::Agent;


const TRADING_AGENT_PROMPT: &str = r#"
You are an expert cryptocurrency trading analyst. Your role is to analyze market data and provide clear, actionable trading insights.

Guidelines:
- Focus on technical and fundamental analysis
- Provide specific entry/exit points
- Consider risk management
- Explain your reasoning clearly
- Be concise but thorough

Format your responses like this:

📊 Market Analysis:
[Your analysis of the current market situation]

💡 Trading Opportunities:
- [Opportunity 1 with entry/exit points]
- [Opportunity 2 with entry/exit points]

⚠️ Risk Management:
[Risk considerations and stop-loss recommendations]
"#;

struct TradingAgent {
    agent: Agent<common::providers::mistral::MistralCompletionModel>,
}

impl TradingAgent {
    pub fn new(client: MistralClient) -> Self {
        let agent = client.agent(MISTRAL_LARGE)
            .temperature(0.7)
            .max_tokens(2000)
            .preamble(TRADING_AGENT_PROMPT)
            .build();
            
        Self { agent }
    }
    
    pub async fn analyze_market(&self, market_data: serde_json::Value) -> Result<String> {
        let prompt = format!(
            "Please analyze this market data and provide trading recommendations:\n{}",
            serde_json::to_string_pretty(&market_data)?
        );
        
        Ok(self.agent.prompt(&prompt).await?)
    }
    
    pub async fn get_trading_advice(&self, symbol: &str, timeframe: &str) -> Result<String> {
        let prompt = format!(
            "Please provide trading advice for {} on the {} timeframe. Include entry points, exit targets, and risk management.",
            symbol, timeframe
        );
        
        Ok(self.agent.prompt(&prompt).await?)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();
    
    println!("🤖 Mistral Trading Agent Example");
    println!("================================\n");

    // Initialize Mistral client and create trading agent
    let client = MistralClient::from_env()?;
    let trading_agent = TradingAgent::new(client);
    
    // Example 1: Analyze market data
    println!("📊 Market Analysis Example");
    println!("-------------------------");
    
    let market_data = json!({
        "bitcoin": {
            "price": 65000.0,
            "24h_change": 2.5,
            "volume": 28_000_000_000_i64,
            "market_cap": 1_200_000_000_000_i64
        },
        "market_sentiment": "bullish",
        "fear_greed_index": 75
    });
    
    match trading_agent.analyze_market(market_data).await {
        Ok(analysis) => println!("{}", analysis),
        Err(e) => println!("❌ Error: {}", e),
    }
    
    // Example 2: Get trading advice
    println!("\n💡 Trading Advice Example");
    println!("------------------------");
    
    match trading_agent.get_trading_advice("BTC/USD", "4h").await {
        Ok(advice) => println!("{}", advice),
        Err(e) => println!("❌ Error: {}", e),
    }
    
    Ok(())
} 