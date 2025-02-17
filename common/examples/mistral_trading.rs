use anyhow::Result;
use common::providers::mistral::Client;
use dotenv::dotenv;
use rig::completion::{Message, Prompt};
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
    fn new(api_key: &str) -> Result<Self> {
        let client = Client::new(api_key);
        let agent = client.agent("mistral-large-latest")
            .preamble(TRADING_AGENT_PROMPT)
            .temperature(0.7)
            .build();
            
        Ok(Self { agent })
    }
    
    async fn analyze_market(&self, price: f64, volume: f64) -> Result<String> {
        let prompt = format!(
            "Current BTC price: ${:.2}\nTrading volume: ${:.2}\nWhat's your analysis?",
            price, volume
        );
        
        Ok(self.agent.prompt(Message::user(prompt)).await?)
    }
    
    async fn suggest_trade(&self, analysis: &str) -> Result<String> {
        let prompt = format!(
            "Based on this analysis:\n{}\n\nWhat trade would you suggest?",
            analysis
        );
        
        Ok(self.agent.prompt(Message::user(prompt)).await?)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();
    
    println!("🤖 Mistral Trading Agent Example");
    println!("================================\n");

    let api_key = std::env::var("MISTRAL_API_KEY")
        .expect("MISTRAL_API_KEY must be set");
        
    let agent = TradingAgent::new(&api_key)?;
    
    let price = 65000.0;
    let volume = 1_500_000_000.0;
    
    let analysis = agent.analyze_market(price, volume).await?;
    println!("Market Analysis:\n{}\n", analysis);
    
    let suggestion = agent.suggest_trade(&analysis).await?;
    println!("Trade Suggestion:\n{}", suggestion);
    
    Ok(())
} 