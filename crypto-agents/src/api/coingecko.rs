use crate::models::{MarketData, GlobalData, CoinData, TrendingCoin, AgentError};
use anyhow::{Result, Context};
use reqwest::Client;
use serde_json::Value;
use std::env;
use tokio::time::Duration;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;

const BASE_URL: &str = "https://api.coingecko.com/api/v3";
const RATE_LIMIT_DELAY: u64 = 60; // Delay in seconds when rate limited
const REQUEST_DELAY: u64 = 6;     // Delay between requests

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DetailedCoinData {
    pub id: String,
    pub name: String,
    pub symbol: String,
    #[serde(default)]
    pub current_price: f64,
    #[serde(default)]
    pub market_cap: f64,
    #[serde(default, rename = "total_volume")]
    pub volume_24h: f64,
    #[serde(default, rename = "price_change_percentage_1h_in_currency")]
    pub price_change_1h: Option<f64>,
    #[serde(default, rename = "price_change_percentage_24h_in_currency")]
    pub price_change_24h: Option<f64>,
    #[serde(default, rename = "price_change_percentage_7d_in_currency")]
    pub price_change_7d: Option<f64>,
    #[serde(default, rename = "price_change_percentage_30d_in_currency")]
    pub price_change_30d: Option<f64>,
}

pub struct CoinGeckoClient {
    client: Client,
    processed_coins: std::collections::HashSet<String>,
    processed_coins_file: String,
}

impl CoinGeckoClient {
    pub fn new() -> Result<Self> {
        let processed_coins_file = "data/processed_coins.json".to_string();
        let processed_coins = Self::load_processed_coins(&processed_coins_file)?;
        
        println!("📚 Loaded {} previously processed coins", processed_coins.len());
        
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
                .build()?,
            processed_coins,
            processed_coins_file,
        })
    }
    
    fn load_processed_coins(file_path: &str) -> Result<std::collections::HashSet<String>> {
        // Create directory if it doesn't exist
        if let Some(dir) = Path::new(file_path).parent() {
            fs::create_dir_all(dir)?;
        }
        
        // Try to load existing processed coins
        if Path::new(file_path).exists() {
            let file = File::open(file_path)?;
            let reader = BufReader::new(file);
            Ok(serde_json::from_reader(reader).unwrap_or_default())
        } else {
            Ok(std::collections::HashSet::new())
        }
    }
    
    fn save_processed_coins(&self) -> Result<()> {
        let file = File::create(&self.processed_coins_file)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &self.processed_coins)?;
        Ok(())
    }
    
    async fn make_request(&self, url: &str, params: &[(&str, &str)]) -> Result<Value> {
        println!("🌐 Making request to: {}", url);
        
        let mut response = self.client
            .get(url)
            .query(params)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to send request")?;
            
        // Handle rate limiting
        while response.status() == 429 {
            println!("⚠️ Rate limited, waiting {} seconds...", RATE_LIMIT_DELAY);
            tokio::time::sleep(Duration::from_secs(RATE_LIMIT_DELAY)).await;
            response = self.client
                .get(url)
                .query(params)
                .header("Accept", "application/json")
                .send()
                .await
                .context("Failed to send request after rate limit")?;
        }
        
        // Check for other error status codes
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AgentError::ApiError(format!(
                "CoinGecko API error: {} - {}", 
                status, 
                error_text
            )).into());
        }
        
        // Get the response text for debugging if JSON parsing fails
        let text = response.text().await
            .context("Failed to get response text")?;
            
        if text.trim().is_empty() {
            return Err(AgentError::ApiError("Empty response from CoinGecko".to_string()).into());
        }
        
        serde_json::from_str(&text)
            .with_context(|| format!("Failed to parse JSON response: {}", text))
    }
    
    pub async fn ping(&self) -> Result<bool> {
        let response = self.client
            .get(&format!("{}/ping", BASE_URL))
            .header("Accept", "application/json")
            .send()
            .await?;
            
        Ok(response.status().is_success())
    }
    
    pub async fn get_global_data(&self) -> Result<GlobalData> {
        let response: Value = self.client
            .get(&format!("{}/global", BASE_URL))
            .header("Accept", "application/json")
            .send()
            .await?
            .json()
            .await?;
            
        let data = response.as_object()
            .ok_or_else(|| AgentError::InvalidData("Invalid global data format".to_string()))?;
            
        Ok(GlobalData {
            total_market_cap: data.get("total_mcap")
                .and_then(|v| v.as_f64())
                .unwrap_or_default(),
            total_volume: data.get("total_volume")
                .and_then(|v| v.as_f64())
                .unwrap_or_default(),
            market_cap_change_percentage_24h: data.get("mcap_change_percentage")
                .and_then(|v| v.as_f64())
                .unwrap_or_default(),
            active_cryptocurrencies: data.get("active_cryptocurrencies")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32)
                .unwrap_or_default(),
        })
    }
    
    async fn get_trending_coins(&self) -> Result<Vec<TrendingCoin>> {
        let response: Value = self.client
            .get(&format!("{}/search/trending", BASE_URL))
            .header("Accept", "application/json")
            .send()
            .await?
            .json()
            .await?;
            
        let coins = response.get("coins")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AgentError::InvalidData("Invalid trending coins format".to_string()))?;
            
        let mut trending = Vec::new();
        for coin in coins {
            if let Some(item) = coin.get("item").and_then(|v| v.as_object()) {
                trending.push(TrendingCoin {
                    id: item.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    symbol: item.get("symbol").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    name: item.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    price_btc: item.get("price_btc").and_then(|v| v.as_f64()).unwrap_or_default(),
                });
            }
        }
        
        Ok(trending)
    }
    
    async fn get_coin_data(&self, id: &str) -> Result<CoinData> {
        let response: Value = self.client
            .get(&format!("{}/coins/{}", BASE_URL, id))
            .header("Accept", "application/json")
            .send()
            .await?
            .json()
            .await?;
            
        Ok(CoinData {
            id: response["id"].as_str().unwrap_or_default().to_string(),
            symbol: response["symbol"].as_str().unwrap_or_default().to_string(),
            name: response["name"].as_str().unwrap_or_default().to_string(),
            current_price: response["market_data"]["current_price"]["usd"].as_f64().unwrap_or_default(),
            market_cap: response["market_data"]["market_cap"]["usd"].as_f64().unwrap_or_default(),
            price_change_24h: response["market_data"]["price_change_24h"].as_f64().unwrap_or_default(),
        })
    }
    
    pub async fn get_market_data(&self) -> Result<MarketData> {
        // Get global data
        let overview = self.get_global_data().await?;
        
        // Get trending coins
        let trending = self.get_trending_coins().await?;
        
        // Get BTC and ETH data
        let bitcoin = self.get_coin_data("bitcoin").await?;
        let ethereum = self.get_coin_data("ethereum").await?;
        
        Ok(MarketData {
            overview,
            trending,
            bitcoin,
            ethereum,
            recent_history: None,
        })
    }
    
    pub async fn get_top_gainers(&self) -> Result<Vec<DetailedCoinData>> {
        let url = format!("{}/coins/markets", BASE_URL);
        let params = [
            ("vs_currency", "usd"),
            ("order", "volume_desc"),
            ("per_page", "250"),
            ("sparkline", "false"),
            ("price_change_percentage", "1h,24h")  // Keep both 1h and 24h for reference
        ];
        
        println!("📊 Fetching potential top gainers...");
        let data = self.make_request(&url, &params).await?;
        
        // First deserialize as Value to check the structure
        let coins_value: Value = serde_json::from_value(data.clone())
            .context("Failed to parse response as JSON Value")?;
            
        if let Some(error_msg) = coins_value.get("error") {
            println!("⚠️ API returned error: {}", error_msg);
            return Err(AgentError::ApiError(format!("API error: {}", error_msg)).into());
        }
        
        let all_coins: Vec<DetailedCoinData> = serde_json::from_value(data)
            .context("Failed to parse top gainers response")?;
            
        // Filter for significant gains and sort by combined performance
        let mut filtered_coins: Vec<DetailedCoinData> = all_coins.into_iter()
            .filter(|coin| {
                // Filter out stablecoins and low volatility coins
                !coin.symbol.to_lowercase().contains("usd") && 
                !coin.symbol.to_lowercase().contains("usdt") &&
                !coin.symbol.to_lowercase().contains("usdc") &&
                !coin.symbol.to_lowercase().contains("dai") &&
                !coin.symbol.to_lowercase().contains("busd") &&
                // Ensure we have valid price data
                coin.current_price > 0.0 &&
                coin.volume_24h > 100000.0 &&  // Ensure decent liquidity
                // Check if we have valid price change data and >3% gains in 1h
                coin.price_change_1h.unwrap_or_default() > 3.0
            })
            .collect();
            
        // Sort by 1h performance
        filtered_coins.sort_by(|a, b| {
            let a_score = a.price_change_1h.unwrap_or_default();
            let b_score = b.price_change_1h.unwrap_or_default();
            b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Take top 20
        let top_coins = filtered_coins.into_iter().take(20).collect::<Vec<_>>();
            
        println!("✅ Found {} coins with >3% gains in 1h", top_coins.len());
        
        // Print detailed gains for monitoring
        for coin in &top_coins {
            println!("🚀 {}: 1h: {:.2}%, Vol: ${:.2}M", 
                coin.symbol.to_uppercase(), 
                coin.price_change_1h.unwrap_or_default(),
                coin.volume_24h / 1_000_000.0
            );
        }
        
        Ok(top_coins)
    }
    
    pub async fn get_new_coins(&mut self) -> Result<Vec<DetailedCoinData>> {
        let url = format!("{}/coins/list", BASE_URL);
        let params = [
            ("include_platform", "false")
        ];
        
        println!("🆕 Fetching new coins list...");
        let data = self.make_request(&url, &params).await?;
        
        // Get list of all coins first
        let all_coins: Vec<Value> = serde_json::from_value(data)
            .context("Failed to parse coins list")?;
            
        // Filter out coins we've already processed
        let new_coins = all_coins.into_iter()
            .filter(|coin| {
                let id = coin["id"].as_str().unwrap_or_default();
                !self.processed_coins.contains(id) && !id.is_empty()
            })
            .take(10)
            .collect::<Vec<Value>>();
            
        if new_coins.is_empty() {
            println!("ℹ️ No new coins found since last check");
            return Ok(Vec::new());
        }
            
        println!("📝 Found {} potential new coins, getting details...", new_coins.len());
        
        let mut detailed_coins = Vec::new();
        
        // Get detailed data for each new coin
        for coin in new_coins {
            let id = coin["id"].as_str().unwrap_or_default();
            
            // Skip if we've already processed this coin (double check)
            if self.processed_coins.contains(id) {
                println!("⏩ Skipping already processed coin: {}", id);
                continue;
            }
            
            match self.get_detailed_coin_data(id).await {
                Ok(coin_data) => {
                    // Only add non-stablecoins with valid data
                    if !coin_data.symbol.to_lowercase().contains("usd") && 
                       !coin_data.name.is_empty() &&
                       !coin_data.symbol.is_empty() {
                        println!("✅ Added new coin: {} ({}) - Price: ${:.8}", 
                            coin_data.name, 
                            coin_data.symbol.to_uppercase(),
                            coin_data.current_price
                        );
                        detailed_coins.push(coin_data);
                        // Add to processed coins set
                        self.processed_coins.insert(id.to_string());
                        // Save after each successful addition
                        if let Err(e) = self.save_processed_coins() {
                            println!("⚠️ Failed to save processed coins: {}", e);
                        }
                    } else {
                        println!("⏩ Skipping invalid coin data: {} ({})", coin_data.name, coin_data.symbol);
                    }
                },
                Err(e) => {
                    println!("⚠️ Failed to get details for {}: {}", id, e);
                    continue;
                }
            }
            
            // Add delay between requests to avoid rate limiting
            tokio::time::sleep(Duration::from_secs(REQUEST_DELAY)).await;
        }
        
        println!("✅ Successfully processed {} new unique coins", detailed_coins.len());
        
        // Final save of processed coins
        if let Err(e) = self.save_processed_coins() {
            println!("⚠️ Failed to save processed coins: {}", e);
        }
        
        Ok(detailed_coins)
    }
    
    pub async fn get_detailed_coin_data(&self, coin_id: &str) -> Result<DetailedCoinData> {
        let url = format!("{}/coins/{}", BASE_URL, coin_id);
        let params = [
            ("localization", "false"),
            ("tickers", "false"),
            ("market_data", "true"),
            ("community_data", "false"),
            ("developer_data", "false"),
            ("sparkline", "false")
        ];
        
        println!("🔍 Fetching details for coin {}...", coin_id);
        let data = self.make_request(&url, &params).await?;
        let coin_data: DetailedCoinData = serde_json::from_value(data)
            .context("Failed to parse coin details response")?;
            
        println!("✅ Successfully fetched details for {}", coin_id);
        Ok(coin_data)
    }
} 