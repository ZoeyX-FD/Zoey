use async_trait::async_trait;
use anyhow::Result;
use chrono::Utc;
use csv::Writer;
use std::path::PathBuf;
use std::fs::OpenOptions;
use serde::{Serialize, Deserialize};
use serde_json;

use crate::models::{MarketData, AgentError};
use super::{Agent, BaseAgent, ModelProvider};

const TOKEN_LOG_FILE: &str = "data/agent_memory/tokens.csv";
const MONITORED_TOKENS_FILE: &str = "data/agent_memory/monitored_tokens.json";
const EXTRACTOR_SYSTEM_PROMPT: &str = r#"
You are a specialized Token Extraction AI focused on identifying cryptocurrency tokens and symbols from text.

Your task is to:
1. Extract ALL cryptocurrency symbols mentioned (BTC, ETH, SOL, etc.)
2. Convert cryptocurrency names to their symbols (Bitcoin -> BTC)
3. Include tokens from:
   - Direct mentions
   - Trading pairs
   - Price discussions
   - Market analysis

Rules:
1. Output format: One symbol per line in UPPERCASE
2. Only include valid symbols (2-10 alphanumeric characters)
3. Remove duplicates
4. No explanations or additional text
5. Convert full names to symbols (e.g., "Bitcoin" -> "BTC")

Example input:
"Bitcoin is trading at $50k, while the ETH/USD pair shows strength. Solana ecosystem..."

Example output:
BTC
ETH
SOL

Remember: Be thorough but precise. Only output valid symbols, one per line.
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoredToken {
    pub symbol: String,
    pub name: String,
    pub alert_threshold: f64,  // Price change threshold for alerts
    pub last_mention_round: Option<i32>,
}

pub struct TokenExtractor {
    base: BaseAgent,
    monitored_tokens: Vec<MonitoredToken>,
}

#[derive(Debug)]
pub struct ExtractedToken {
    pub timestamp: chrono::DateTime<Utc>,
    pub round: i32,
    pub token: String,
    pub context: String,
}

impl TokenExtractor {
    pub async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        // Create token log file if it doesn't exist
        if !PathBuf::from(TOKEN_LOG_FILE).exists() {
            let mut wtr = Writer::from_path(TOKEN_LOG_FILE)?;
            wtr.write_record(&["timestamp", "round", "token", "context"])?;
            wtr.flush()?;
        }

        // Load monitored tokens
        let monitored_tokens = Self::load_monitored_tokens().await?;
        
        Ok(Self {
            base: BaseAgent::new(
                "Token Extractor".to_string(),
                model,
                EXTRACTOR_SYSTEM_PROMPT.to_string(),
                provider
            )
            .await?
            .with_temperature(0.0),
            monitored_tokens,
        })
    }

    // Add a new token to monitor
    pub async fn add_monitored_token(&mut self, symbol: String, name: String, alert_threshold: f64) -> Result<()> {
        let token = MonitoredToken {
            symbol: symbol.to_uppercase(),
            name,
            alert_threshold,
            last_mention_round: None,
        };
        
        self.monitored_tokens.push(token);
        self.save_monitored_tokens().await?;
        Ok(())
    }

    // Remove a token from monitoring
    pub async fn remove_monitored_token(&mut self, symbol: &str) -> Result<()> {
        self.monitored_tokens.retain(|t| t.symbol != symbol.to_uppercase());
        self.save_monitored_tokens().await?;
        Ok(())
    }

    // Load monitored tokens from file
    async fn load_monitored_tokens() -> Result<Vec<MonitoredToken>> {
        let path = PathBuf::from(MONITORED_TOKENS_FILE);
        if path.exists() {
            let content = tokio::fs::read_to_string(&path).await?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Vec::new())
        }
    }

    // Save monitored tokens to file
    async fn save_monitored_tokens(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.monitored_tokens)?;
        tokio::fs::write(MONITORED_TOKENS_FILE, content).await?;
        Ok(())
    }

    pub async fn extract_tokens(
        &mut self,
        round: i32,
        agent_one_msg: &str,
        agent_two_msg: &str,
    ) -> Result<Vec<ExtractedToken>> {
        // Combine messages to reduce API calls
        let combined_prompt = format!(
            "Extract all cryptocurrency symbols from these messages. List only the symbols, one per line:\n\n{}\n\n{}",
            agent_one_msg, agent_two_msg
        );
        
        let response = self.base.generate_response(&combined_prompt, None).await?;
        
        let mut all_tokens = response
            .lines()
            .map(|line| line.trim().to_uppercase())
            .filter(|line| !line.is_empty())
            .filter(|line| line.chars().all(|c| c.is_ascii_alphanumeric()))
            .filter(|line| line.len() >= 2 && line.len() <= 10)
            .collect::<Vec<_>>();
            
        // Remove duplicates
        all_tokens.sort();
        all_tokens.dedup();

        // Check for monitored tokens
        for token in &all_tokens {
            if let Some(monitored) = self.monitored_tokens.iter_mut()
                .find(|t| t.symbol == *token) {
                monitored.last_mention_round = Some(round);
                println!("🔔 ALERT: Monitored token {} ({}) mentioned!", monitored.symbol, monitored.name);
            }
        }
            
        let timestamp = Utc::now();
        let extracted = all_tokens.into_iter()
            .map(|token| ExtractedToken {
                timestamp,
                round,
                token,
                context: format!("Round {} discussion", round),
            })
            .collect::<Vec<_>>();
            
        // Save to CSV
        self.save_tokens(&extracted)?;
        
        Ok(extracted)
    }

    // Get list of currently monitored tokens
    pub fn get_monitored_tokens(&self) -> &[MonitoredToken] {
        &self.monitored_tokens
    }
    
    fn save_tokens(&self, tokens: &[ExtractedToken]) -> Result<()> {
        let file = OpenOptions::new()
            .append(true)
            .open(TOKEN_LOG_FILE)?;
            
        let mut wtr = Writer::from_writer(file);
        
        for token in tokens {
            wtr.write_record(&[
                token.timestamp.to_rfc3339(),
                token.round.to_string(),
                token.token.clone(),
                token.context.clone(),
            ])?;
        }
        
        wtr.flush()?;
        Ok(())
    }
}

#[async_trait]
impl Agent for TokenExtractor {
    fn name(&self) -> &str {
        &self.base.name
    }
    
    fn model(&self) -> &str {
        &self.base.model
    }
    
    async fn think(&mut self, _market_data: &MarketData, _previous_message: Option<String>) -> Result<String> {
        Err(AgentError::InvalidData("Token extractor doesn't implement think()".to_string()).into())
    }
    
    async fn save_memory(&self) -> Result<()> {
        self.base.save_memory().await
    }
    
    async fn load_memory(&mut self) -> Result<()> {
        self.base.load_memory().await
    }
    
    fn memory_file(&self) -> PathBuf {
        self.base.memory_file()
    }
} 