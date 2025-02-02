use std::collections::VecDeque;
use std::time::Duration;
use anyhow::Result;
use tokio::time;
use tokio::select;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use dotenv::dotenv;
use std::env;
use std::io::Write;

use crate::{
    agents::{TechnicalAgent, FundamentalAgent, TokenExtractor, SynopsisAgent, ModelProvider, Agent},
    api::coingecko::CoinGeckoClient,
};

const MAX_HISTORY_ROUNDS: usize = 50;
const MINUTES_BETWEEN_ROUNDS: u64 = 30;

// Default model constants
const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-chat";
const DEFAULT_GEMINI_MODEL: &str = "gemini-pro";
const DEFAULT_MISTRAL_MODEL: &str = "mistral-large-latest";

impl ModelProvider {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "deepseek" => Some(Self::DeepSeek),
            "gemini" => Some(Self::Gemini),
            "mistral" => Some(Self::Mistral),
            _ => None
        }
    }
}

pub struct MultiAgentSystem {
    api: CoinGeckoClient,
    technical_agent: TechnicalAgent,
    fundamental_agent: FundamentalAgent,
    token_extractor: TokenExtractor,
    synopsis_agent: SynopsisAgent,
    round_history: VecDeque<String>,
}

impl MultiAgentSystem {
    pub async fn new() -> Result<Self> {
        // Load environment variables
        dotenv().ok();
        
        // Initialize API client
        let api = CoinGeckoClient::new()?;
        
        // Get model configurations from environment variables
        let technical_model = env::var("TECHNICAL_MODEL").unwrap_or_else(|_| DEFAULT_DEEPSEEK_MODEL.to_string());
        let technical_provider = env::var("TECHNICAL_PROVIDER")
            .ok()
            .and_then(|p| ModelProvider::from_str(&p))
            .unwrap_or(ModelProvider::DeepSeek);

        let fundamental_model = env::var("FUNDAMENTAL_MODEL").unwrap_or_else(|_| DEFAULT_DEEPSEEK_MODEL.to_string());
        let fundamental_provider = env::var("FUNDAMENTAL_PROVIDER")
            .ok()
            .and_then(|p| ModelProvider::from_str(&p))
            .unwrap_or(ModelProvider::DeepSeek);

        let extractor_model = env::var("EXTRACTOR_MODEL").unwrap_or_else(|_| DEFAULT_MISTRAL_MODEL.to_string());
        let extractor_provider = env::var("EXTRACTOR_PROVIDER")
            .ok()
            .and_then(|p| ModelProvider::from_str(&p))
            .unwrap_or(ModelProvider::Mistral);

        let synopsis_model = env::var("SYNOPSIS_MODEL").unwrap_or_else(|_| DEFAULT_MISTRAL_MODEL.to_string());
        let synopsis_provider = env::var("SYNOPSIS_PROVIDER")
            .ok()
            .and_then(|p| ModelProvider::from_str(&p))
            .unwrap_or(ModelProvider::Mistral);
        
        // Initialize agents with configured models
        println!("🔄 Initializing Technical Analysis agent with {} ({})", technical_model, format!("{:?}", technical_provider));
        let technical_agent = TechnicalAgent::new(
            technical_model,
            technical_provider
        ).await?;
        
        println!("🔄 Initializing Fundamental Analysis agent with {} ({})", fundamental_model, format!("{:?}", fundamental_provider));
        let fundamental_agent = FundamentalAgent::new(
            fundamental_model,
            fundamental_provider
        ).await?;
        
        println!("🔄 Initializing Token Extractor agent with {} ({})", extractor_model, format!("{:?}", extractor_provider));
        let token_extractor = TokenExtractor::new(
            extractor_model,
            extractor_provider
        ).await?;
        
        println!("🔄 Initializing Synopsis agent with {} ({})", synopsis_model, format!("{:?}", synopsis_provider));
        let synopsis_agent = SynopsisAgent::new(
            synopsis_model,
            synopsis_provider
        ).await?;
        
        Ok(Self {
            api,
            technical_agent,
            fundamental_agent,
            token_extractor,
            synopsis_agent,
            round_history: VecDeque::with_capacity(MAX_HISTORY_ROUNDS),
        })
    }
    
    pub async fn handle_command(&mut self, command: &str) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "monitor" => {
                if parts.len() < 3 {
                    println!("Usage: monitor <symbol> <name> [alert_threshold]");
                    return Ok(());
                }
                let symbol = parts[1].to_string();
                let name = parts[2].to_string();
                let threshold = parts.get(3)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5.0); // Default 5% threshold

                self.token_extractor.add_monitored_token(symbol.clone(), name.clone(), threshold).await?;
                println!("✅ Added {} ({}) to monitored tokens", name, symbol);
            }
            "unmonitor" => {
                if parts.len() < 2 {
                    println!("Usage: unmonitor <symbol>");
                    return Ok(());
                }
                self.token_extractor.remove_monitored_token(parts[1]).await?;
                println!("✅ Removed {} from monitored tokens", parts[1]);
            }
            "list" => {
                println!("\n📋 Currently Monitored Tokens:");
                for token in self.token_extractor.get_monitored_tokens() {
                    println!("  • {} ({}) - Alert threshold: {}%", 
                        token.symbol, 
                        token.name,
                        token.alert_threshold
                    );
                }
            }
            _ => {
                println!("Unknown command. Available commands:");
                println!("  monitor <symbol> <name> [alert_threshold] - Add a token to monitor");
                println!("  unmonitor <symbol> - Remove a token from monitoring");
                println!("  list - Show all monitored tokens");
            }
        }
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("\n🤖 Crypto Multi-Agent System");
        println!("Available commands:");
        println!("  monitor <symbol> <name> [alert_threshold] - Add a token to monitor");
        println!("  unmonitor <symbol> - Remove a token from monitoring");
        println!("  list - Show all monitored tokens");
        println!("Press Ctrl+C to exit\n");

        let mut interval = time::interval(Duration::from_secs(MINUTES_BETWEEN_ROUNDS * 60));
        let stdin = BufReader::new(io::stdin());
        let mut lines = stdin.lines();

        loop {
            print!("> ");
            std::io::stdout().flush()?;

            select! {
                _ = interval.tick() => {
                    self.run_conversation_cycle().await?;
                    println!("\n⏳ Waiting {} minutes until next round...", MINUTES_BETWEEN_ROUNDS);
                    print!("> ");
                    std::io::stdout().flush()?;
                }
                
                Ok(Some(line)) = lines.next_line() => {
                    if !line.trim().is_empty() {
                        self.handle_command(&line.trim()).await?;
                    }
                    print!("> ");
                    std::io::stdout().flush()?;
                }
            }
        }
    }
    
    pub async fn run_conversation_cycle(&mut self) -> Result<()> {
        println!("\n🔄 Starting New Trading Round!");
        
        // Get fresh market data
        println!("📊 Gathering Market Intelligence...");
        let market_data = self.api.get_market_data().await?;
        
        // Get technical analysis
        println!("\n🔍 Technical Analysis Phase...");
        let technical_response = self.technical_agent
            .think(&market_data, None)
            .await?;
        println!("{}", technical_response);
        
        // Get fundamental analysis
        println!("\n🌍 Fundamental Analysis Phase...");
        let fundamental_response = self.fundamental_agent
            .think(&market_data, Some(technical_response.clone()))
            .await?;
        println!("{}", fundamental_response);
        
        // Extract tokens
        println!("\n🔍 Extracting Token Mentions...");
        let round = self.round_history.len() as i32;
        let tokens = self.token_extractor
            .extract_tokens(round, &technical_response, &fundamental_response)
            .await?;
        println!("Found {} token mentions:", tokens.len());
        for token in &tokens {
            println!("  • {} ({})", token.token, token.context);
        }
        
        // Generate synopsis
        println!("\n📝 Generating Round Synopsis...");
        let synopsis = self.synopsis_agent
            .generate_synopsis(&technical_response, &fundamental_response)
            .await?;
        println!("{}", synopsis);
        
        // Update history
        self.round_history.push_back(synopsis);
        if self.round_history.len() > MAX_HISTORY_ROUNDS {
            self.round_history.pop_front();
        }
        
        Ok(())
    }
} 
