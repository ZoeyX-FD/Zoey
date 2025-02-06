use async_trait::async_trait;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs::{self, OpenOptions};
use csv::Writer;
use tokio;

use crate::models::MarketData;
use crate::api::CoinGeckoClient;
use super::{Agent, BaseAgent, ModelProvider};

// const COINGECKO_BASE_URL: &str = "https://api.coingecko.com/api/v3";  // Changed to free tier URL

const NEW_TOP_SYSTEM_PROMPT: &str = r#"
You are the New & Top Coins Analysis Agent 🔍
Your role is to analyze new tokens and top performing coins on CoinGecko.

Focus on:
- Technical indicators and price action
- Market sentiment and momentum
- Project fundamentals and potential
- Risk assessment and red flags
- Entry/exit points and strategy

Format your response like this:

🤖 New & Top Coins Analysis
=========================

📊 Market Analysis:
[Your analysis in simple terms]

💡 Key Points:
- [Point 1]
- [Point 2]
- [Point 3]

🎯 Recommendation:
[BUY/SELL/DO NOTHING with clear reasoning]

⚠️ Risk Assessment:
[Key risks and mitigation strategies]
"#;

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub timestamp: String,
    pub coin_data: crate::api::coingecko::DetailedCoinData,
    pub recommendation: String,
    pub analysis: String,
}

pub struct NewTopAgent {
    base: BaseAgent,
    coingecko: CoinGeckoClient,
    results_dir: PathBuf,
}

impl NewTopAgent {
    pub async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        // Create results directory
        let results_dir = PathBuf::from("data/market_analysis");
        fs::create_dir_all(&results_dir)?;
        
        Ok(Self {
            base: BaseAgent::new(
                "New & Top Agent".to_string(),
                model,
                NEW_TOP_SYSTEM_PROMPT.to_string(),
                provider
            ).await?,
            coingecko: CoinGeckoClient::new()?,
            results_dir,
        })
    }
    
    async fn analyze_coin(&self, coin: &crate::api::coingecko::DetailedCoinData) -> Result<AnalysisResult> {
        if coin.current_price == 0.0 {
            // For new coins without price data, just record them without analysis
            return Ok(AnalysisResult {
                timestamp: Utc::now().to_rfc3339(),
                coin_data: coin.clone(),
                recommendation: "NEW LISTING".to_string(),
                analysis: "New coin just listed on CoinGecko. No price data available yet.".to_string(),
            });
        }

        let prompt = format!(
            "Analyze this cryptocurrency:\n\nName: {}\nSymbol: {}\nPrice: ${:.8}\n1h Change: {:.2}%\nVolume: ${:.2}M\n\nProvide a BUY/SELL/DO NOTHING recommendation with analysis.",
            coin.name, 
            coin.symbol, 
            coin.current_price, 
            coin.price_change_1h.unwrap_or_default(), 
            coin.volume_24h / 1_000_000.0
        );
        
        // Try up to 3 times to get a valid analysis
        let mut retries = 3;
        let mut last_error = None;
        
        while retries > 0 {
            match self.base.generate_response(&prompt, None).await {
                Ok(analysis) => {
                    let recommendation = if analysis.contains("BUY") {
                        "BUY"
                    } else if analysis.contains("SELL") {
                        "SELL"
                    } else {
                        "DO NOTHING"
                    }.to_string();
                    
                    return Ok(AnalysisResult {
                        timestamp: Utc::now().to_rfc3339(),
                        coin_data: coin.clone(),
                        recommendation,
                        analysis,
                    });
                },
                Err(e) => {
                    last_error = Some(e);
                    retries -= 1;
                    if retries > 0 {
                        println!("⚠️ Analysis failed for {}, retrying... ({} attempts left)", coin.name, retries);
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Failed to analyze coin after multiple attempts").into()))
    }
    
    async fn save_analysis(&self, result: &AnalysisResult) -> Result<()> {
        // Save to CSV
        let file_path = self.results_dir.join("analysis_results.csv");
        let mut wtr = if file_path.exists() {
            Writer::from_writer(OpenOptions::new()
                .append(true)
                .open(&file_path)?)
        } else {
            let mut wtr = Writer::from_writer(fs::File::create(&file_path)?);
            wtr.write_record(&[
                "timestamp", "coin_id", "name", "symbol", "price",
                "price_change_1h", "recommendation", "analysis"
            ])?;
            wtr
        };
        
        wtr.write_record(&[
            &result.timestamp,
            &result.coin_data.id,
            &result.coin_data.name,
            &result.coin_data.symbol,
            &result.coin_data.current_price.to_string(),
            &format!("{:.2}", result.coin_data.price_change_1h.unwrap_or_default()),
            &result.recommendation,
            &result.analysis,
        ])?;
        
        wtr.flush()?;
        Ok(())
    }
    
    pub async fn run_analysis_cycle(&mut self) -> Result<()> {
        // Get top gainers
        match self.coingecko.get_top_gainers().await {
            Ok(gainers) => {
                for coin in gainers {
                    match self.analyze_coin(&coin).await {
                        Ok(result) => {
                            if let Err(e) = self.save_analysis(&result).await {
                                println!("⚠️ Failed to save analysis for {}: {}", coin.name, e);
                            }
                        },
                        Err(e) => println!("⚠️ Failed to analyze {}: {}", coin.name, e)
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            },
            Err(e) => println!("⚠️ Failed to get top gainers: {}", e)
        }
        
        // Get new coins
        match self.coingecko.get_new_coins().await {
            Ok(new_coins) => {
                for coin in new_coins {
                    match self.analyze_coin(&coin).await {
                        Ok(result) => {
                            if let Err(e) = self.save_analysis(&result).await {
                                println!("⚠️ Failed to save analysis for {}: {}", coin.name, e);
                            }
                        },
                        Err(e) => println!("⚠️ Failed to analyze {}: {}", coin.name, e)
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            },
            Err(e) => println!("⚠️ Failed to get new coins: {}", e)
        }
        
        Ok(())
    }
}

#[async_trait]
impl Agent for NewTopAgent {
    fn name(&self) -> &str {
        &self.base.name
    }
    
    fn model(&self) -> &str {
        &self.base.model
    }
    
    async fn think(&mut self, _market_data: &MarketData, _previous_message: Option<String>) -> Result<String> {
        // Run analysis cycle and return summary
        self.run_analysis_cycle().await?;
        Ok("Analysis cycle completed successfully".to_string())
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