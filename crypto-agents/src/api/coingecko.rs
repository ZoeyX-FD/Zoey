use serde::{Serialize, Deserialize};
use crate::models::{MarketData, GlobalData, CoinData, TrendingCoin, AgentError};
use anyhow::{Result, Context};
use reqwest::Client;
use serde_json::Value;
use tokio::time::Duration;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::collections::HashMap;
use std::time::Instant;

const BASE_URL: &str = "https://api.coingecko.com/api/v3";
const BASE_DELAY: u64 = 3;  // Increase base delay to 3 seconds
const REQUEST_DELAY: u64 = 3;     // Delay between requests
const MAX_RETRIES: u32 = 3;
const DEMO_API_KEY: &str = "CG-mVrwoy4JYveQ5MvX2Z2jbsyn";

// Add this attribute to hide dead code warnings
#[allow(dead_code)]
const CATEGORY_AI: &[&str] = &[
    "ai-and-big-data",     // Main AI category
    "ai-agents",           // AI Agents specific
    "chatbot",             // AI chat/bot tokens
    "machine-learning"     // ML specific tokens
];

#[allow(dead_code)]
const CATEGORY_LAYER1: &[&str] = &[
    "smart-contract-platform",  // Main L1 category
    "cosmos-ecosystem",
    "ethereum-ecosystem",
    "solana-ecosystem",
    "avalanche-ecosystem",
    "polkadot-ecosystem"
];

#[allow(dead_code)]
const CATEGORY_LAYER2: &[&str] = &[
    "layer-2",             // Main L2 category
    "scaling",             // Scaling solutions
    "optimistic-rollups",  // Optimistic rollups
    "zk-rollups"          // ZK rollups
];

#[allow(dead_code)]
const CATEGORY_DEFI: &[&str] = &[
    "decentralized-finance-defi",
    "yield-farming",
    "decentralized-exchange",
    "lending-borrowing",
    "synthetic-issuer",
    "liquid-staking-derivatives",
    "automated-market-maker-amm"
];

// Keep only the coin list constants that we use
#[allow(dead_code)]
const AI_COINS: &str = "fetch-ai,singularitynet,ocean-protocol,numeraire,oasis-network,graphlinq-protocol,matrix-ai-network,injective,render-token,akash-network,bittensor,cortex,vectorspace,aleph-zero";
#[allow(dead_code)]
const LAYER1_COINS: &str = "ethereum,bitcoin,solana,cardano,avalanche-2,cosmos,polkadot,near,tron,stellar,algorand,internet-computer,flow,tezos,hedera-hashgraph,elrond-erd-2,waves,neo,zilliqa";
#[allow(dead_code)]
const LAYER2_COINS: &str = "polygon,optimism,arbitrum,immutable-x,loopring,zkspace,starknet,metis-token,boba-network,zksync-era,mantle,base,linea,scroll";

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(skip)]
    pub ma_50: Option<f64>,
    #[serde(skip)]
    pub ma_200: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandleData {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalData {
    pub candles: Vec<CandleData>,
    pub rsi_14: Option<f64>,
    pub ma_50: Option<f64>,
    pub ma_200: Option<f64>,
    pub macd: Option<(f64, f64, f64)>, // (MACD, Signal, Histogram)
    pub bollinger_bands: Option<(f64, f64, f64)>, // (Upper, Middle, Lower)
    pub volume_24h: Option<f64>,
    pub current_price: Option<f64>,     // Added current price
    pub price_change_24h: Option<f64>,  // Added 24h price change
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketTechnicalData {
    pub btc_data: TechnicalData,
    pub eth_data: TechnicalData,
    pub sol_data: TechnicalData,
    pub trending_data: Vec<(String, TechnicalData)>,
    pub global_metrics: GlobalTechnicalMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTechnicalMetrics {
    pub total_market_cap: f64,
    pub btc_dominance: f64,
    pub eth_dominance: f64,
    pub sol_dominance: f64,
    pub total_volume_24h: f64,
    pub market_cap_change_24h: f64,
    pub volume_change_24h: f64,
    pub defi_dominance: f64,      // Add DeFi sector dominance
    pub layer1_dominance: f64,    // Add Layer 1 dominance
    pub top10_dominance: f64,     // Concentration in top 10
    pub volatility_index: f64,    // Market volatility measure
    pub ai_sector_dominance: f64,    // Add AI sector tracking
    pub ai_sector_volume: f64,       // AI token volume
    pub ai_sector_growth: f64,       // AI sector 24h change
    pub cross_chain_volume: f64,     // Bridge volume
    pub dex_volume_share: f64,       // DEX vs CEX ratio
    pub rwa_sector_dominance: f64,    // Add RWA sector dominance
    pub rwa_sector_volume: f64,       // Add RWA volume
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryData {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub market_cap: Option<f64>,
    #[serde(default)]
    pub market_cap_change_24h: Option<f64>,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub top_3_coins_id: Vec<String>,
    #[serde(default)]
    pub top_3_coins: Vec<String>,
    #[serde(default)]
    pub volume_24h: Option<f64>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

// Add new struct for category list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryListItem {
    pub category_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct HistoricalData {
    pub prices: Vec<[f64; 2]>,        // [timestamp, price]
    pub market_caps: Vec<[f64; 2]>,   // [timestamp, market_cap]
    pub total_volumes: Vec<[f64; 2]>, // [timestamp, volume]
}

pub struct CoinGeckoClient {
    client: Client,
    processed_coins: std::collections::HashSet<String>,
    processed_coins_file: String,
    cache: HashMap<String, (TechnicalData, Instant)>,
    cache_duration: Duration,
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
            cache: HashMap::new(),
            cache_duration: Duration::from_secs(300), // 5 minute cache
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
        let mut delay = BASE_DELAY;
        let mut retries = 0;
        
        loop {
            println!("🌐 Making request to: {}", url);
            
            // Add demo API key to query parameters
            let mut all_params = Vec::from(params);
            all_params.push(("x_cg_demo_api_key", DEMO_API_KEY));
            
            let response = tokio::time::timeout(
                Duration::from_secs(15),
                self.client
                .get(url)
                    .query(&all_params)
                    .header("accept", "application/json")
                .send()
            ).await;

            match response {
                Ok(res) => match res {
                    Ok(r) => {
                        // Add delay between requests
                        tokio::time::sleep(Duration::from_secs(6)).await;
                        
                        if r.status() == 429 {
                            println!("⚠️ Rate limited, waiting {} seconds...", delay);
                            tokio::time::sleep(Duration::from_secs(delay)).await;
                            delay *= 2;
                            if retries >= MAX_RETRIES {
                                return Err(anyhow::anyhow!("Rate limit exceeded after {} retries", MAX_RETRIES));
                            }
                            retries += 1;
                            continue;
                        }
                        
                        if r.status().is_success() {
                            let text = r.text().await
            .context("Failed to get response text")?;
            
                            return serde_json::from_str(&text)
                                .with_context(|| format!("Failed to parse JSON response: {}", text));
                        }
                        
                        println!("⚠️ Request failed with status: {}", r.status());
                        if retries >= MAX_RETRIES {
                            return Err(anyhow::anyhow!("Request failed with status: {}", r.status()));
                        }
                        retries += 1;
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                        delay *= 2;
                        continue;
                    }
                    Err(e) => {
                        println!("⚠️ Request error: {}", e);
                        if retries >= MAX_RETRIES {
                            return Err(anyhow::anyhow!("Request failed: {}", e));
                        }
                        retries += 1;
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                        delay *= 2;
                        continue;
                    }
                },
                Err(_) => {
                    println!("⚠️ Request timed out");
                    if retries >= MAX_RETRIES {
                        return Err(anyhow::anyhow!("Request timed out after {} retries", MAX_RETRIES));
                    }
                    retries += 1;
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    delay *= 2;
                    continue;
                }
            }
        }
    }
    
    async fn get_global_data(&self) -> Result<GlobalData> {
        let response: Value = self.client
            .get(&format!("{}/global", BASE_URL))
            .header("Accept", "application/json")
            .send()
            .await?
            .json()
            .await?;
            
        let data = response.get("data")
            .ok_or_else(|| AgentError::InvalidData("No data field in global response".to_string()))?;
            
        Ok(GlobalData {
            total_market_cap: data.get("total_market_cap")
                .and_then(|v| v.get("usd"))
                .and_then(|v| v.as_f64())
                .unwrap_or_default(),
            total_volume: data.get("total_volume")
                .and_then(|v| v.get("usd"))
                .and_then(|v| v.as_f64())
                .unwrap_or_default(),
            market_cap_change_percentage_24h: data.get("market_cap_change_percentage_24h_usd")
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
                    market_cap_rank: item.get("market_cap_rank")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32),
                });
            }
        }
        
        Ok(trending)
    }
    
    #[allow(dead_code)]
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
        let global_data = self.get_global_data().await?;
        
        println!("Debug: Global Market Data");
        println!("Market Cap: ${:.2}B", global_data.total_market_cap / 1_000_000_000.0);
        println!("Volume: ${:.2}B", global_data.total_volume / 1_000_000_000.0);
        println!("Active Coins: {}", global_data.active_cryptocurrencies);
        println!("24h Change: {:.2}%", global_data.market_cap_change_percentage_24h);

        let btc_data = self.get_detailed_coin_data("bitcoin").await?;
        
        let eth_data = self.get_detailed_coin_data("ethereum").await?;
        
        let trending = self.get_trending_coins().await?;
        
        Ok(MarketData {
            overview: global_data,
            trending,
            bitcoin: CoinData {
                id: "bitcoin".to_string(),
                symbol: "BTC".to_string(),
                name: "Bitcoin".to_string(),
                current_price: btc_data.current_price,
                market_cap: btc_data.market_cap,
                price_change_24h: btc_data.price_change_24h.unwrap_or(0.0),
            },
            ethereum: CoinData {
                id: "ethereum".to_string(),
                symbol: "ETH".to_string(),
                name: "Ethereum".to_string(),
                current_price: eth_data.current_price,
                market_cap: eth_data.market_cap,
                price_change_24h: eth_data.price_change_24h.unwrap_or(0.0),
            },
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
            ("price_change_percentage", "1h,24h")
        ];
        
        println!("📊 Fetching potential top gainers...");
        let data = self.make_request(&url, &params).await?;
        
        let coins_value: Value = serde_json::from_value(data.clone())
            .context("Failed to parse response as JSON Value")?;
            
        if let Some(error_msg) = coins_value.get("error") {
            println!("⚠️ API returned error: {}", error_msg);
            return Err(AgentError::ApiError(format!("API error: {}", error_msg)).into());
        }
        
        let all_coins: Vec<DetailedCoinData> = serde_json::from_value(data)
            .context("Failed to parse top gainers response")?;
            
        let mut filtered_coins: Vec<DetailedCoinData> = all_coins.into_iter()
            .filter(|coin| {
                !coin.symbol.to_lowercase().contains("usd") && 
                !coin.symbol.to_lowercase().contains("usdt") &&
                !coin.symbol.to_lowercase().contains("usdc") &&
                !coin.symbol.to_lowercase().contains("dai") &&
                !coin.symbol.to_lowercase().contains("busd") &&
                coin.current_price > 0.0 &&
                coin.volume_24h > 100000.0 &&
                coin.price_change_1h.unwrap_or_default() > 3.0
            })
            .collect();
            
        filtered_coins.sort_by(|a, b| {
            let a_score = a.price_change_1h.unwrap_or_default();
            let b_score = b.price_change_1h.unwrap_or_default();
            b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        let top_coins = filtered_coins.into_iter().take(20).collect::<Vec<_>>();
            
        println!("✅ Found {} coins with >3% gains in 1h", top_coins.len());
        
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
        
        println!("📝 Fetching new coins list...");
        let data = self.make_request(&url, &params).await?;
        
        let all_coins: Vec<Value> = serde_json::from_value(data)
            .context("Failed to parse coins list")?;
            
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
        
        for coin in new_coins {
            let id = coin["id"].as_str().unwrap_or_default();
            
            if self.processed_coins.contains(id) {
                println!("⏩ Skipping already processed coin: {}", id);
                continue;
            }
            
            match self.get_detailed_coin_data(id).await {
                Ok(coin_data) => {
                    if !coin_data.symbol.to_lowercase().contains("usd") && 
                       !coin_data.name.is_empty() &&
                       !coin_data.symbol.is_empty() {
                        println!("✅ Added new coin: {} ({}) - Price: ${:.8}", 
                            coin_data.name, 
                            coin_data.symbol.to_uppercase(),
                            coin_data.current_price
                        );
                        detailed_coins.push(coin_data);
                        self.processed_coins.insert(id.to_string());
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
            
            tokio::time::sleep(Duration::from_secs(REQUEST_DELAY)).await;
        }
        
        println!("✅ Successfully processed {} new unique coins", detailed_coins.len());
        
        if let Err(e) = self.save_processed_coins() {
            println!("⚠️ Failed to save processed coins: {}", e);
        }
        
        Ok(detailed_coins)
    }
    
    async fn get_coin_market_data(&self, coin_id: &str) -> Result<DetailedCoinData> {
        let url = format!("{}/coins/markets", BASE_URL);
        let params = [
            ("vs_currency", "usd"),
            ("ids", coin_id),
            ("order", "market_cap_desc"),
            ("per_page", "1"),
            ("page", "1"),
            ("sparkline", "false"),
            ("price_change_percentage", "1h,24h,7d,30d")
        ];
        
        let data = self.make_request(&url, &params).await?;
        let coins: Vec<DetailedCoinData> = serde_json::from_value(data)?;
        let coin_data = coins.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No data found"))?;
        
        Ok(coin_data)
    }

    pub async fn get_detailed_coin_data(&self, coin_id: &str) -> Result<DetailedCoinData> {
        println!("🔍 Fetching details for coin {}...", coin_id);
        
        // Get basic market data
        let mut coin_data = self.get_coin_market_data(coin_id).await?;
        
        // Get historical data for MAs
        let historical = self.get_historical_data(coin_id, 200).await?;
        
        println!("📊 Calculating Moving Averages...");
        println!("Historical data points: {}", historical.prices.len());
        
        // Calculate MAs
        coin_data.ma_50 = self.calculate_ma_from_prices(&historical.prices, 50);
        coin_data.ma_200 = self.calculate_ma_from_prices(&historical.prices, 200);
        
        println!("MA50: ${:.2}", coin_data.ma_50.unwrap_or_default());
        println!("MA200: ${:.2}", coin_data.ma_200.unwrap_or_default());
        
        println!("✅ Successfully fetched details for {}", coin_id);
        Ok(coin_data)
    }

    pub async fn get_candle_data(&self, coin_id: &str, days: u16) -> Result<Vec<CandleData>> {
        let url = format!("{}/coins/{}/ohlc", BASE_URL, coin_id);
        let params = [
            ("vs_currency", "usd"),
            ("days", &days.to_string()),
        ];

        println!("📊 Fetching candle data for {} over {} days", coin_id, days);
        let data = self.make_request(&url, &params).await?;

        let candles: Vec<Vec<f64>> = serde_json::from_value(data)?;

        let candle_data = candles.into_iter()
            .map(|candle| CandleData {
                timestamp: candle[0] as i64,
                open: candle[1],
                high: candle[2],
                low: candle[3],
                close: candle[4],
                volume: 0.0, // OHLC endpoint doesn't provide volume data
            })
            .collect();

        Ok(candle_data)
    }

    #[allow(dead_code)]
    async fn get_market_chart_fallback(&self, coin_id: &str, days: u16) -> Result<Vec<CandleData>> {
        let url = format!("{}/coins/{}/market_chart", BASE_URL, coin_id);
        let params = [
            ("vs_currency", "usd"),
            ("days", &days.to_string()),
            ("interval", "daily"),
        ];

        println!("📊 Using market_chart fallback for volume data...");
        let data = self.make_request(&url, &params).await?;

        let _prices: Vec<Vec<f64>> = serde_json::from_value(data["prices"].clone())?;
        let volumes: Vec<Vec<f64>> = serde_json::from_value(data["total_volumes"].clone())?;

        let mut candles = self.get_ohlc_data(coin_id, days.into()).await?;
        
        for (i, candle) in candles.iter_mut().enumerate() {
            if i < volumes.len() {
                candle.volume = volumes[i][1];
            }
        }

        Ok(candles)
    }

    pub async fn get_market_chart(&self, coin_id: &str, _days: u32) -> Result<TechnicalData> {
        println!("📈 Fetching market data for {}", coin_id);
        
        // Get historical price data for MA calculations
        let historical = self.get_historical_data(coin_id, 200).await?;
        
        // Get OHLC data for shorter timeframe analysis
        let candles = self.get_ohlc_data(coin_id, 1).await?;
        
        // Get current price data
        let price_url = format!("{}/simple/price", BASE_URL);
        let price_params = [
            ("ids", coin_id),
            ("vs_currencies", "usd"),
            ("include_market_cap", "true"),
            ("include_24hr_vol", "true"),
            ("include_24hr_change", "true"),
            ("include_last_updated_at", "true"),
            ("precision", "2")
        ];

        let price_data = self.make_request(&price_url, &price_params).await?;
        let volume_24h = price_data[coin_id]["usd_24h_vol"]
            .as_f64()
            .unwrap_or(0.0);
        let current_price = price_data[coin_id]["usd"]
            .as_f64()
            .unwrap_or(0.0);
        let price_change_24h = price_data[coin_id]["usd_24h_change"]
            .as_f64()
            .unwrap_or(0.0);
        
        // Calculate RSI using OHLC data
        let rsi_14 = self.calculate_rsi(&candles, 14);
        
        // Calculate MAs using historical price data
        let ma_50 = self.calculate_ma_from_prices(&historical.prices, 50);
        let ma_200 = self.calculate_ma_from_prices(&historical.prices, 200);
        
        let macd = self.calculate_macd(&candles);
        let bb = self.calculate_bollinger_bands(&candles);
        
        println!("📈 Calculated indicators:");
        println!("  • Current Price: ${:.2}", current_price);
        println!("  • Price Change 24h: {:.2}%", price_change_24h);
        println!("  • RSI (14): {:.2}", rsi_14);
        println!("  • 50 MA: ${:.2}", ma_50.unwrap_or_default());
        println!("  • 200 MA: ${:.2}", ma_200.unwrap_or_default());
        
        if let Some((macd_val, signal, hist)) = macd {
            println!("  • MACD: {:.2}/{:.2}/{:.2}", macd_val, signal, hist);
        }
        
        if let Some((upper, middle, lower)) = bb {
            println!("  • Bollinger Bands: {:.2}/{:.2}/{:.2}", upper, middle, lower);
        }
        
        println!("  • Volume 24h: ${:.2}B", volume_24h / 1e9);
        
        Ok(TechnicalData {
            candles,
            rsi_14: Some(rsi_14),
            ma_50,
            ma_200,
            macd,
            bollinger_bands: bb,
            volume_24h: Some(volume_24h),
            current_price: Some(current_price),
            price_change_24h: Some(price_change_24h),
        })
    }

    pub async fn get_ohlc_data(&self, coin_id: &str, days: u32) -> Result<Vec<CandleData>> {
        let url = format!("{}/coins/{}/ohlc", BASE_URL, coin_id);
        
        // Normalize days to allowed values: 1, 7, 14, 30, 90, 180, 365, max
        let normalized_days = match days {
            d if d <= 1 => "1",
            d if d <= 7 => "7",
            d if d <= 14 => "14",
            d if d <= 30 => "30",
            d if d <= 90 => "90",
            d if d <= 180 => "180",
            d if d <= 365 => "365",
            _ => "max"
        };
        
        let params = [("vs_currency", "usd"), ("days", normalized_days)];
        
        println!("📊 Fetching OHLC data for {} days...", normalized_days);
        let data = self.make_request(&url, &params).await?;
        
        // Parse OHLC data
        let candles = data.as_array()
            .context("Invalid OHLC data format")?
            .iter()
            .map(|v| {
                let parts = v.as_array().context("Invalid candle format")?;
                Ok(CandleData {
                    timestamp: parts[0].as_i64().context("Invalid timestamp")?,
                    open: parts[1].as_f64().context("Invalid open price")?,
                    high: parts[2].as_f64().context("Invalid high price")?,
                    low: parts[3].as_f64().context("Invalid low price")?,
                    close: parts[4].as_f64().context("Invalid close price")?,
                    volume: 0.0, // OHLC endpoint doesn't provide volume data
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(candles)
    }

    fn calculate_rsi(&self, candles: &[CandleData], period: usize) -> f64 {
        if candles.len() < period + 1 {
            return 50.0;
        }

        let mut gains = Vec::new();
        let mut losses = Vec::new();

        for i in 1..candles.len() {
            let change = candles[i].close - candles[i-1].close;
            if change > 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(change.abs());
            }
        }

        let mut avg_gain = gains.iter().take(period).sum::<f64>() / period as f64;
        let mut avg_loss = losses.iter().take(period).sum::<f64>() / period as f64;

        for i in period..gains.len() {
            avg_gain = (avg_gain * (period - 1) as f64 + gains[i]) / period as f64;
            avg_loss = (avg_loss * (period - 1) as f64 + losses[i]) / period as f64;
        }

        if avg_loss == 0.0 {
            return 100.0;
        }

        let rs = avg_gain / avg_loss;
        100.0 - (100.0 / (1.0 + rs))
    }

    fn calculate_ma_from_candles(&self, candles: &[CandleData], period: usize) -> Option<f64> {
        if candles.len() < period {
            return None;
        }

        let prices: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let start_idx = prices.len().saturating_sub(period);
        let sum: f64 = prices[start_idx..].iter().sum();
        Some(sum / period as f64)
    }

    fn calculate_ma_from_prices(&self, prices: &[[f64; 2]], period: usize) -> Option<f64> {
        if prices.len() < period {
            println!("⚠️ Not enough data points for MA{}: {} < {}", period, prices.len(), period);
            return None;
        }
        
        let recent_prices: Vec<f64> = prices.iter()
            .rev() // Get most recent first
            .take(period)
            .map(|p| p[1]) // Get price value
            .collect();
        
        let sum: f64 = recent_prices.iter().sum();
        let ma = Some(sum / period as f64);
        println!("Calculated MA{}: ${:.2}", period, ma.unwrap_or_default());
        ma
    }

    fn calculate_macd(&self, candles: &[CandleData]) -> Option<(f64, f64, f64)> {
        if candles.len() < 26 {
            return None;
        }

        let prices: Vec<f64> = candles.iter().map(|c| c.close).collect();
        
        let mut fast_ema = prices[0];
        let mut slow_ema = prices[0];
        let mut signal = 0.0;
        
        let fast_alpha = 2.0 / (12.0 + 1.0);
        let slow_alpha = 2.0 / (26.0 + 1.0);
        let signal_alpha = 2.0 / (9.0 + 1.0);
        
        let mut macd_values = Vec::new();
        
        for price in prices.iter() {
            fast_ema = price * fast_alpha + fast_ema * (1.0 - fast_alpha);
            slow_ema = price * slow_alpha + slow_ema * (1.0 - slow_alpha);
            let macd = fast_ema - slow_ema;
            macd_values.push(macd);
        }
        
        if !macd_values.is_empty() {
            signal = macd_values[0];
            for macd in macd_values.iter().skip(1) {
                signal = macd * signal_alpha + signal * (1.0 - signal_alpha);
            }
        }
        
        let latest_macd = *macd_values.last().unwrap_or(&0.0);
        let histogram = latest_macd - signal;
        
        Some((latest_macd, signal, histogram))
    }

    fn calculate_bollinger_bands(&self, candles: &[CandleData]) -> Option<(f64, f64, f64)> {
        let sma = self.calculate_ma_from_candles(candles, 20);
        
        let variance: f64 = candles.iter()
            .rev()
            .take(20)
            .map(|c| {
                let diff = c.close - sma.unwrap_or_default();
                diff * diff
            })
            .sum::<f64>() / 20.0;
        
        let std_dev = variance.sqrt();
        
        let upper_band = sma.unwrap_or_default() + (2.0 * std_dev);
        let lower_band = sma.unwrap_or_default() - (2.0 * std_dev);

        if upper_band.is_infinite() || lower_band.is_infinite() {
            return None;
        }

        Some((upper_band, sma.unwrap_or_default(), lower_band))
    }

    fn calculate_volume_change(&self, data_sets: &[&TechnicalData]) -> f64 {
        let mut total_change = 0.0;
        let mut valid_sets = 0;

        for data in data_sets {
            if data.candles.len() >= 2 {
                let current_volume = data.candles.last().unwrap().volume;
                let prev_volume = data.candles[data.candles.len() - 2].volume;
                
                if prev_volume > 0.0 {
                    total_change += (current_volume - prev_volume) / prev_volume * 100.0;
                    valid_sets += 1;
                }
            }
        }

        if valid_sets > 0 {
            total_change / valid_sets as f64
        } else {
            0.0
        }
    }

    pub async fn get_category_volumes(&self) -> Result<(f64, f64, f64, f64)> {
        println!("📊 Fetching category data...");
        
        // Fetch AI category data
        let ai_url = format!("{}/coins/categories/artificial-intelligence", BASE_URL);
        let ai_response = self.client
            .get(&ai_url)
            .header("accept", "application/json")
            .header("x-cg-demo-api-key", DEMO_API_KEY)
            .send()
            .await?;
        
        let ai_data: CategoryData = serde_json::from_str(&ai_response.text().await?)?;
        tokio::time::sleep(Duration::from_secs(6)).await;

        // Fetch Layer 1 data
        let l1_url = format!("{}/coins/categories/layer-1", BASE_URL);
        let l1_response = self.client
            .get(&l1_url)
            .header("accept", "application/json")
            .header("x-cg-demo-api-key", DEMO_API_KEY)
            .send()
            .await?;
        
        let l1_data: CategoryData = serde_json::from_str(&l1_response.text().await?)?;
        tokio::time::sleep(Duration::from_secs(6)).await;

        // Fetch Layer 2 data
        let l2_url = format!("{}/coins/categories/layer-2", BASE_URL);
        let l2_response = self.client
            .get(&l2_url)
            .header("accept", "application/json")
            .header("x-cg-demo-api-key", DEMO_API_KEY)
            .send()
            .await?;
        
        let l2_data: CategoryData = serde_json::from_str(&l2_response.text().await?)?;
        tokio::time::sleep(Duration::from_secs(6)).await;

        // Fetch RWA data
        let rwa_url = format!("{}/coins/categories/real-world-assets-rwa", BASE_URL);
        let rwa_response = self.client
            .get(&rwa_url)
            .header("accept", "application/json")
            .header("x-cg-demo-api-key", DEMO_API_KEY)
            .send()
            .await?;
        
        let rwa_data: CategoryData = serde_json::from_str(&rwa_response.text().await?)?;

        // Print sector summary
        println!("\n📊 Sector Analysis:");
        println!("🤖 AI Sector:");
        println!("  • Market Cap: ${:.2}B", ai_data.market_cap.unwrap_or(0.0) / 1e9);
        println!("  • Market Cap Change 24h: {:.2}%", ai_data.market_cap_change_24h.unwrap_or(0.0));
        println!("  • Volume 24h: ${:.2}B", ai_data.volume_24h.unwrap_or(0.0) / 1e9);

        println!("\n🔗 Layer 1 Sector:");
        println!("  • Market Cap: ${:.2}B", l1_data.market_cap.unwrap_or(0.0) / 1e9);
        println!("  • Market Cap Change 24h: {:.2}%", l1_data.market_cap_change_24h.unwrap_or(0.0));
        println!("  • Volume 24h: ${:.2}B", l1_data.volume_24h.unwrap_or(0.0) / 1e9);

        println!("\n⚡ Layer 2 Sector:");
        println!("  • Market Cap: ${:.2}B", l2_data.market_cap.unwrap_or(0.0) / 1e9);
        println!("  • Market Cap Change 24h: {:.2}%", l2_data.market_cap_change_24h.unwrap_or(0.0));
        println!("  • Volume 24h: ${:.2}B", l2_data.volume_24h.unwrap_or(0.0) / 1e9);

        println!("\n💎 RWA Sector:");
        println!("  • Market Cap: ${:.2}B", rwa_data.market_cap.unwrap_or(0.0) / 1e9);
        println!("  • Market Cap Change 24h: {:.2}%", rwa_data.market_cap_change_24h.unwrap_or(0.0));
        println!("  • Volume 24h: ${:.2}B", rwa_data.volume_24h.unwrap_or(0.0) / 1e9);

        Ok((
            ai_data.volume_24h.unwrap_or(0.0),
            l1_data.volume_24h.unwrap_or(0.0),
            l2_data.volume_24h.unwrap_or(0.0),
            rwa_data.volume_24h.unwrap_or(0.0)
        ))
    }

    fn calculate_market_metrics(
        &self,
        global: &GlobalData,
        btc_data: &TechnicalData,
        eth_data: &TechnicalData,
        sol_data: &TechnicalData,
        _trending_data: &[(String, TechnicalData)],
        category_volumes: (f64, f64, f64, f64),
    ) -> GlobalTechnicalMetrics {
        let (ai_volume, l1_volume, l2_volume, rwa_volume) = category_volumes;

        // Calculate BTC market cap using current price and circulating supply
        let btc_price = btc_data.current_price.unwrap_or_else(|| btc_data.candles.last().map(|c| c.close).unwrap_or(0.0));
        let btc_supply = 19_600_000.0; // Current approximate circulating supply
        let btc_mcap = btc_price * btc_supply;
        
        // Calculate BTC dominance dynamically
        let total_mcap = global.total_market_cap;
        let btc_dominance = if total_mcap > 0.0 {
            (btc_mcap / total_mcap) * 100.0
        } else {
            0.0
        };

        // Calculate other asset dominances using the same total market cap
        let eth_dominance = if total_mcap > 0.0 { 
            (eth_data.current_price.unwrap_or_else(|| eth_data.candles.last().map(|c| c.close).unwrap_or(0.0)) 
            * 120_000_000.0 / total_mcap) * 100.0 
        } else { 0.0 };
        
        let sol_dominance = if total_mcap > 0.0 { 
            (sol_data.current_price.unwrap_or_else(|| sol_data.candles.last().map(|c| c.close).unwrap_or(0.0)) 
            * 410_000_000.0 / total_mcap) * 100.0 
        } else { 0.0 };

        GlobalTechnicalMetrics {
            total_market_cap: total_mcap,
            btc_dominance,
            eth_dominance,
            sol_dominance,
            total_volume_24h: global.total_volume,
            market_cap_change_24h: global.market_cap_change_percentage_24h,
            volume_change_24h: self.calculate_volume_change(&[btc_data, eth_data, sol_data]),
            defi_dominance: 0.0,
            layer1_dominance: (l1_volume / global.total_volume) * 100.0,
            top10_dominance: 0.0,
            volatility_index: self.calculate_market_volatility(&[btc_data, eth_data, sol_data]),
            ai_sector_dominance: (ai_volume / global.total_volume) * 100.0,
            ai_sector_volume: ai_volume,
            ai_sector_growth: 0.0,
            cross_chain_volume: l2_volume,
            dex_volume_share: 0.0,
            rwa_sector_dominance: (rwa_volume / global.total_volume) * 100.0,
            rwa_sector_volume: rwa_volume,
        }
    }

    fn calculate_market_volatility(&self, data_sets: &[&TechnicalData]) -> f64 {
        let mut total_volatility = 0.0;
        let mut valid_sets = 0;
        
        for data in data_sets {
            if data.candles.len() >= 2 {
                let returns: Vec<f64> = data.candles.windows(2)
                    .map(|window| {
                        (window[1].close - window[0].close) / window[0].close
                    })
                    .collect();
                
                if !returns.is_empty() {
                    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
                    let variance = returns.iter()
                        .map(|r| (r - mean).powi(2))
                        .sum::<f64>() / returns.len() as f64;
                    
                    total_volatility += variance.sqrt();
                    valid_sets += 1;
                }
            }
        }
        
        if valid_sets > 0 {
            (total_volatility / valid_sets as f64) * 100.0
        } else {
            0.0
        }
    }

    pub async fn get_technical_analysis(&self) -> Result<MarketTechnicalData> {
        println!("📊 Fetching comprehensive technical data...");
        
        let category_volumes = self.get_category_volumes().await?;
        
        println!("🔍 Analyzing BTC...");
        let btc_data = self.get_market_chart("bitcoin", 14).await?;
        
        println!("🔍 Analyzing ETH...");
        tokio::time::sleep(Duration::from_secs(BASE_DELAY)).await;
        let eth_data = self.get_market_chart("ethereum", 14).await?;
        
        println!("🔍 Analyzing SOL...");
        tokio::time::sleep(Duration::from_secs(BASE_DELAY)).await;
        let sol_data = self.get_market_chart("solana", 14).await?;
        
        println!("🔥 Analyzing top trending coins...");
        let trending = self.get_trending_coins().await?;
        let mut trending_data = Vec::new();
        
        let relevant_coins: Vec<_> = trending.iter()
            .filter(|c| {
                let symbol = c.symbol.to_lowercase();
                !symbol.contains("pepe") && 
                !symbol.contains("meow") &&
                !symbol.contains("doge") &&
                !symbol.contains("shib") &&
                !symbol.contains("moon") &&
                !symbol.contains("safe") &&
                !symbol.contains("elon") &&
                !symbol.contains("inu") &&
                !symbol.contains("meme") &&
                c.market_cap_rank
                    .map(|rank| rank < 150)
                    .unwrap_or(false)
            })
            .take(3)
            .collect();

        for coin in relevant_coins {
            let rank_display = match coin.market_cap_rank {
                Some(rank) => rank.to_string(),
                None => "N/A".to_string()
            };
            
            println!("Analyzing trending coin: {} (Rank: {})", coin.symbol, rank_display);
            
            match self.get_market_chart(&coin.id, 14).await {
                Ok(data) => {
                    trending_data.push((coin.symbol.clone(), data));
                    tokio::time::sleep(Duration::from_secs(BASE_DELAY)).await;
                },
                Err(e) => {
                    println!("⚠️ Failed to analyze {}: {}", coin.symbol, e);
                    continue;
                }
            }
        }

        let global = self.get_global_data().await?;
        let global_metrics = self.calculate_market_metrics(
            &global,
            &btc_data,
            &eth_data,
            &sol_data,
            &trending_data,
            category_volumes
        );

        println!("📊 Market Metrics:");
        println!("  • Total Market Cap: ${:.2}B", global_metrics.total_market_cap / 1e9);
        println!("  • BTC Dominance: {:.2}%", global_metrics.btc_dominance);
        println!("  • ETH Dominance: {:.2}%", global_metrics.eth_dominance);
        println!("  • SOL Dominance: {:.2}%", global_metrics.sol_dominance);
        println!("  • Layer 1 Dominance: {:.2}%", global_metrics.layer1_dominance);
        println!("  • AI Sector Dominance: {:.2}%", global_metrics.ai_sector_dominance);

        Ok(MarketTechnicalData {
            btc_data,
            eth_data,
            sol_data,
            trending_data,
            global_metrics,
        })
    }

    pub async fn get_coin_technical_analysis(&mut self, coin_id: &str, days: u32) -> Result<TechnicalData> {
        if let Some((data, timestamp)) = self.cache.get(coin_id) {
            if timestamp.elapsed() < self.cache_duration {
                println!("📊 Using cached data for {}", coin_id);
                return Ok(data.clone());
            }
        }

        let data = self.get_market_chart(coin_id, days).await?;
        self.cache.insert(coin_id.to_string(), (data.clone(), Instant::now()));
        Ok(data)
    }

    pub async fn get_historical_data(&self, coin_id: &str, days: u32) -> Result<HistoricalData> {
        let url = format!("{}/coins/{}/market_chart", BASE_URL, coin_id);
        let params = [
            ("vs_currency", "usd"),
            ("days", &days.to_string()),
            ("interval", "daily"),
            ("precision", "2"),
        ];

        println!("📈 Fetching historical data for {} over {} days...", coin_id, days);
        let data = self.make_request(&url, &params).await?;
        let historical: HistoricalData = serde_json::from_value(data)?;
        
        Ok(historical)
    }
}

// Add public accessor for sector data
impl MarketTechnicalData {
    pub fn sector_volumes(&self) -> (f64, f64, f64, f64) {
        (
            self.global_metrics.ai_sector_volume,
            self.global_metrics.layer1_dominance * self.global_metrics.total_volume_24h / 100.0,
            self.global_metrics.cross_chain_volume,
            self.global_metrics.rwa_sector_volume
        )
    }
} 
 