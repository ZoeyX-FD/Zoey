use serde::{Serialize, Deserialize};
use crate::models::{MarketData, GlobalData, CoinData, TrendingCoin, AgentError};
use anyhow::{Result, Context};
use reqwest::Client;
use serde_json::Value;
use tokio::time::Duration;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;

const BASE_URL: &str = "https://api.coingecko.com/api/v3";
const BASE_DELAY: u64 = 6;  // Base delay in seconds
const REQUEST_DELAY: u64 = 6;     // Delay between requests

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
    pub stochastic_rsi: Option<f64>,
    pub volume_sma: Option<f64>,
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
        let mut delay = BASE_DELAY;
        let mut retries = 0;
        const MAX_RETRIES: u32 = 5;
        
        loop {
            println!("🌐 Making request to: {}", url);
            
            let response = self.client
                .get(url)
                .query(params)
                .header("Accept", "application/json")
                .send()
                .await
                .context("Failed to send request")?;
            
            // Check rate limits and adjust delay
            if let Some(remaining) = response.headers()
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u32>().ok()) 
            {
                delay = match remaining {
                    0..=5 => BASE_DELAY * 4,
                    6..=10 => BASE_DELAY * 2,
                    11..=20 => BASE_DELAY,
                    _ => BASE_DELAY / 2,
                };
            }
            
            if response.status() == 429 {
                if retries >= MAX_RETRIES {
                    return Err(anyhow::anyhow!("Max retries exceeded"));
                }
                println!("⚠️ Rate limited, waiting {} seconds...", delay);
                tokio::time::sleep(Duration::from_secs(delay)).await;
                delay *= 2;
                retries += 1;
                continue;
            }
            
            // Process response
            if response.status().is_success() {
                let text = response.text().await
                    .context("Failed to get response text")?;
                
                return serde_json::from_str(&text)
                    .with_context(|| format!("Failed to parse JSON response: {}", text));
            }
            
            return Err(anyhow::anyhow!("Request failed: {}", response.status()));
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

        // The data is nested under "data" in the response
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
        // Get global data
        let global_data = self.get_global_data().await?;
        
        // Debug print
        println!("Debug: Global Market Data");
        println!("Market Cap: ${:.2}B", global_data.total_market_cap / 1_000_000_000.0);
        println!("Volume: ${:.2}B", global_data.total_volume / 1_000_000_000.0);
        println!("Active Coins: {}", global_data.active_cryptocurrencies);
        println!("24h Change: {:.2}%", global_data.market_cap_change_percentage_24h);

        // Get Bitcoin data
        let btc_data = self.get_detailed_coin_data("bitcoin").await?;
        
        // Get Ethereum data
        let eth_data = self.get_detailed_coin_data("ethereum").await?;
        
        // Get trending coins
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
        
        println!("🔍 Fetching details for coin {}...", coin_id);
        let data = self.make_request(&url, &params).await?;
        let coins: Vec<DetailedCoinData> = serde_json::from_value(data)?;
        let coin_data = coins.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No data found"))?;
            
        println!("✅ Successfully fetched details for {}", coin_id);
        Ok(coin_data)
    }

    // Get OHLCV candle data
    pub async fn get_candle_data(&self, coin_id: &str, days: u16) -> Result<Vec<CandleData>> {
        let url = format!("{}/coins/{}/ohlc", BASE_URL, coin_id);
        let params = [
            ("vs_currency", "usd"),
            ("days", &days.to_string()),
        ];

        println!("📊 Fetching candle data for {} over {} days", coin_id, days);
        let data = self.make_request(&url, &params).await?;

        // CoinGecko returns array of arrays: [timestamp, open, high, low, close]
        let candles: Vec<Vec<f64>> = serde_json::from_value(data)?;

        let candle_data = candles.into_iter()
            .map(|candle| CandleData {
                timestamp: candle[0] as i64,
                open: candle[1],
                high: candle[2],
                low: candle[3],
                close: candle[4],
                volume: candle[5],
            })
            .collect();

        Ok(candle_data)
    }

    // Modify get_market_chart to work with free tier
    pub async fn get_market_chart(&self, coin_id: &str, days: u16) -> Result<TechnicalData> {
        let url = format!("{}/coins/{}/market_chart", BASE_URL, coin_id);
        let params = [
            ("vs_currency", "usd"),
            ("days", &days.to_string()),
            // Remove the interval parameter as it's not needed for free tier
        ];

        println!("📈 Fetching market chart for {}", coin_id);
        let data = match self.make_request(&url, &params).await {
            Ok(data) => data,
            Err(e) => {
                println!("❌ Failed to fetch market chart: {}", e);
                return Err(e);
            }
        };

        // Debug print the raw response
        println!("🔍 Raw market chart data received");

        // Extract price and volume data with error handling
        let prices: Vec<Vec<f64>> = match serde_json::from_value(data["prices"].clone()) {
            Ok(p) => p,
            Err(e) => {
                println!("❌ Failed to parse prices: {}", e);
                return Err(anyhow::anyhow!("Failed to parse prices"));
            }
        };

        let volumes: Vec<Vec<f64>> = match serde_json::from_value(data["total_volumes"].clone()) {
            Ok(v) => v,
            Err(e) => {
                println!("❌ Failed to parse volumes: {}", e);
                return Err(anyhow::anyhow!("Failed to parse volumes"));
            }
        };

        println!("✅ Successfully parsed {} price points and {} volume points", 
            prices.len(), volumes.len());

        // Combine into candles (daily)
        let mut candles = Vec::new();
        let points_per_day = prices.len() as f64 / days as f64;
        
        for chunk_start in (0..prices.len()).step_by(points_per_day.round() as usize) {
            let chunk_end = (chunk_start + points_per_day.round() as usize).min(prices.len());
            let day_prices = &prices[chunk_start..chunk_end];
            let day_volumes = &volumes[chunk_start..chunk_end];

            if !day_prices.is_empty() {
                let high = day_prices.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
                let low = day_prices.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
                let open = day_prices.first().unwrap()[1];
                let close = day_prices.last().unwrap()[1];
                let volume = day_volumes.iter().map(|v| v[1]).sum();

                let candle = CandleData {
                    timestamp: day_prices[0][0] as i64,
                    open,
                    high,
                    low,
                    close,
                    volume,
                };
                candles.push(candle);
            }
        }

        println!("📊 Generated {} daily candles", candles.len());

        // Calculate all technical indicators
        let rsi_14 = self.calculate_rsi(&candles, 14);
        let ma_50 = self.calculate_ma(&candles, 50);
        let ma_200 = self.calculate_ma(&candles, 200);
        let macd = self.calculate_macd(&candles);
        let bollinger_bands = self.calculate_bollinger_bands(&candles, 20); // 20-day BB
        let stochastic_rsi = self.calculate_stochastic_rsi(&candles, 14);
        let volume_sma = self.calculate_volume_sma(&candles, 20); // 20-day Volume SMA

        println!("📈 Calculated indicators:");
        println!("  • RSI (14): {:.2}", rsi_14);
        println!("  • 50 MA: ${:.2}", ma_50);
        println!("  • 200 MA: ${:.2}", ma_200);
        println!("  • MACD: {:.2}/{:.2}/{:.2}", macd.0, macd.1, macd.2);
        println!("  • Bollinger Bands: {:.2}/{:.2}/{:.2}", bollinger_bands.0, bollinger_bands.1, bollinger_bands.2);
        println!("  • Stochastic RSI: {:.2}", stochastic_rsi);
        println!("  • Volume SMA: ${:.2}", volume_sma);

        Ok(TechnicalData {
            candles,
            rsi_14: Some(rsi_14),
            ma_50: Some(ma_50),
            ma_200: Some(ma_200),
            macd: Some(macd),
            bollinger_bands: Some(bollinger_bands),
            stochastic_rsi: Some(stochastic_rsi),
            volume_sma: Some(volume_sma),
        })
    }

    // Calculate RSI
    fn calculate_rsi(&self, candles: &[CandleData], period: usize) -> f64 {
        if candles.len() < period + 1 {
            return 50.0; // Default value if not enough data
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        // Calculate initial gains/losses
        for i in 1..=period {
            let diff = candles[i].close - candles[i-1].close;
            if diff > 0.0 {
                gains += diff;
            } else {
                losses -= diff;
            }
        }

        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;

        if avg_loss == 0.0 {
            return 100.0;
        }

        let rs = avg_gain / avg_loss;
        100.0 - (100.0 / (1.0 + rs))
    }

    // Calculate Moving Average
    fn calculate_ma(&self, candles: &[CandleData], period: usize) -> f64 {
        if candles.len() < period {
            return candles.last().map(|c| c.close).unwrap_or_default();
        }

        let sum: f64 = candles.iter()
            .rev()
            .take(period)
            .map(|c| c.close)
            .sum();

        sum / period as f64
    }

    // Calculate MACD
    fn calculate_macd(&self, candles: &[CandleData]) -> (f64, f64, f64) {
        let prices: Vec<f64> = candles.iter().map(|c| c.close).collect();
        
        // Calculate EMAs for the entire price series
        let fast_ema = self.calculate_ema_series(&prices, 12);
        let slow_ema = self.calculate_ema_series(&prices, 26);
        
        // Calculate MACD line
        let macd_line: Vec<f64> = fast_ema.iter()
            .zip(slow_ema.iter())
            .map(|(f, s)| f - s)
            .collect();
        
        // Calculate signal line from MACD values
        let signal_line = self.calculate_ema_series(&macd_line, 9);
        
        // Get latest values
        let latest_macd = *macd_line.last().unwrap_or(&0.0);
        let latest_signal = *signal_line.last().unwrap_or(&0.0);
        let histogram = latest_macd - latest_signal;
        
        (latest_macd, latest_signal, histogram)
    }

    fn calculate_bollinger_bands(&self, candles: &[CandleData], period: usize) -> (f64, f64, f64) {
        let sma = self.calculate_ma(candles, period);
        
        // Calculate standard deviation
        let variance: f64 = candles.iter()
            .rev()
            .take(period)
            .map(|c| {
                let diff = c.close - sma;
                diff * diff
            })
            .sum::<f64>() / period as f64;
        
        let std_dev = variance.sqrt();
        
        // Upper and lower bands (2 standard deviations)
        let upper_band = sma + (2.0 * std_dev);
        let lower_band = sma - (2.0 * std_dev);

        (upper_band, sma, lower_band)
    }

    fn calculate_stochastic_rsi(&self, candles: &[CandleData], period: usize) -> f64 {
        let rsi_values: Vec<f64> = candles.windows(period + 1)
            .map(|window| self.calculate_rsi(window, period))
            .collect();

        if rsi_values.is_empty() {
            return 50.0;
        }

        // Explicitly specify f64 for min/max operations
        let min_rsi = rsi_values.iter()
            .fold(100.0_f64, |a: f64, &b| a.min(b));
        let max_rsi = rsi_values.iter()
            .fold(0.0_f64, |a: f64, &b| a.max(b));

        // Use f64::abs() instead of the abs() method
        if f64::abs(max_rsi - min_rsi) < f64::EPSILON {
            return 50.0;
        }

        // Calculate Stochastic RSI
        let current_rsi = *rsi_values.last().unwrap();
        (current_rsi - min_rsi) / (max_rsi - min_rsi) * 100.0
    }

    fn calculate_volume_sma(&self, candles: &[CandleData], period: usize) -> f64 {
        if candles.len() < period {
            return candles.last().map(|c| c.volume).unwrap_or_default();
        }

        let sum: f64 = candles.iter()
            .rev()
            .take(period)
            .map(|c| c.volume)
            .sum();

        sum / period as f64
    }

    // Helper method for MACD
    fn calculate_ema_series(&self, prices: &[f64], period: usize) -> Vec<f64> {
        let mut ema_values = Vec::with_capacity(prices.len());
        let multiplier = 2.0 / (period as f64 + 1.0);
        
        // Initialize with SMA
        let first_sma = prices.iter().take(period).sum::<f64>() / period as f64;
        ema_values.push(first_sma);
        
        // Calculate EMA for remaining prices
        for i in period..prices.len() {
            let previous_ema = ema_values.last().unwrap();
            let ema = (prices[i] - previous_ema) * multiplier + previous_ema;
            ema_values.push(ema);
        }
        
        ema_values
    }

    // Add the missing volume change calculation method
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

    // Update market metrics calculation to use the new volume change method
    fn calculate_market_metrics(
        &self,
        global: &GlobalData,
        btc_data: &TechnicalData,
        eth_data: &TechnicalData,
        sol_data: &TechnicalData,
    ) -> GlobalTechnicalMetrics {
        let total_mcap = global.total_market_cap;
        
        // Fix market cap calculation
        let btc_mcap = btc_data.candles.last()
            .map(|c| c.close)  // Remove volume multiplication
            .unwrap_or_default();
            
        let eth_mcap = eth_data.candles.last()
            .map(|c| c.close)
            .unwrap_or_default();
            
        let sol_mcap = sol_data.candles.last()
            .map(|c| c.close)
            .unwrap_or_default();

        // Calculate dominance correctly
        let btc_dom = if total_mcap > 0.0 { btc_mcap * 100.0 / total_mcap } else { 0.0 };
        let eth_dom = if total_mcap > 0.0 { eth_mcap * 100.0 / total_mcap } else { 0.0 };
        let sol_dom = if total_mcap > 0.0 { sol_mcap * 100.0 / total_mcap } else { 0.0 };
        
        // Calculate Layer 1 dominance (BTC + ETH + SOL + other major L1s)
        let layer1_dom = btc_dom + eth_dom + sol_dom;

        // Calculate volume metrics with all three coins
        let volume_change = self.calculate_volume_change(&[btc_data, eth_data, sol_data]);
        
        // Calculate volatility index
        let volatility = self.calculate_market_volatility(&[btc_data, eth_data, sol_data]);

        GlobalTechnicalMetrics {
            total_market_cap: total_mcap,
            btc_dominance: btc_dom,
            eth_dominance: eth_dom,
            sol_dominance: sol_dom,
            total_volume_24h: global.total_volume,
            market_cap_change_24h: global.market_cap_change_percentage_24h,
            volume_change_24h: volume_change,
            defi_dominance: 0.0,  // Would need additional API calls
            layer1_dominance: layer1_dom,
            top10_dominance: btc_dom + eth_dom + sol_dom, // Simplified version
            volatility_index: volatility,
            ai_sector_dominance: 0.0,    // Add AI sector tracking
            ai_sector_volume: 0.0,       // AI token volume
            ai_sector_growth: 0.0,       // AI sector 24h change
            cross_chain_volume: 0.0,     // Bridge volume
            dex_volume_share: 0.0,       // DEX vs CEX ratio
        }
    }

    // Add method to calculate market volatility
    fn calculate_market_volatility(&self, data_sets: &[&TechnicalData]) -> f64 {
        let mut total_volatility = 0.0;
        let mut valid_sets = 0;
        
        for data in data_sets {
            if data.candles.len() >= 2 {
                // Calculate returns for consecutive candles
                let returns: Vec<f64> = data.candles.windows(2)
                    .map(|window| {
                        (window[1].close - window[0].close) / window[0].close
                    })
                    .collect();
                
                if !returns.is_empty() {
                    // Calculate standard deviation of returns
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
            (total_volatility / valid_sets as f64) * 100.0 // Convert to percentage
        } else {
            0.0
        }
    }

    // Update get_technical_analysis to use new metrics
    pub async fn get_technical_analysis(&self) -> Result<MarketTechnicalData> {
        println!("📊 Fetching comprehensive technical data...");
        
        // Get BTC technical data
        println!("🔍 Analyzing BTC...");
        let btc_data = self.get_market_chart("bitcoin", 14).await?;
        
        // Get ETH technical data with delay
        println!("🔍 Analyzing ETH...");
        tokio::time::sleep(Duration::from_secs(BASE_DELAY)).await;
        let eth_data = self.get_market_chart("ethereum", 14).await?;
        
        // Get SOL technical data with delay
        println!("🔍 Analyzing SOL...");
        tokio::time::sleep(Duration::from_secs(BASE_DELAY)).await;
        let sol_data = self.get_market_chart("solana", 14).await?;
        
        // Get trending coins technical data (limited to top 3)
        println!("🔥 Analyzing top trending coins...");
        let trending = self.get_trending_coins().await?;
        let mut trending_data = Vec::new();
        
        // Improve trending coin filtering with AI consideration
        let relevant_coins: Vec<_> = trending.iter()
            .filter(|c| {
                let symbol = c.symbol.to_lowercase();
                // More strict filtering - exclude BTC, ETH, and SOL
                symbol != "btc" && 
                symbol != "eth" && 
                symbol != "sol" && 
                // Filter out low quality/meme coins but keep AI
                !symbol.contains("elon") &&
                !symbol.contains("pepe") &&
                !symbol.contains("moon") &&
                !symbol.contains("meme") &&
                !symbol.contains("inu") &&
                // Only include coins with decent market cap rank
                c.market_cap_rank
                    .map(|rank| rank < 200)
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
                    // Add delay between requests
                    tokio::time::sleep(Duration::from_secs(BASE_DELAY)).await;
                },
                Err(e) => {
                    println!("⚠️ Failed to analyze {}: {}", coin.symbol, e);
                    continue;
                }
            }
        }

        // Get global market data and calculate comprehensive metrics
        let global = self.get_global_data().await?;
        let global_metrics = self.calculate_market_metrics(
            &global,
            &btc_data,
            &eth_data,
            &sol_data
        );

        println!("📊 Market Metrics:");
        println!("  • Total Market Cap: ${:.2}B", global_metrics.total_market_cap / 1e9);
        println!("  • BTC Dominance: {:.2}%", global_metrics.btc_dominance);
        println!("  • ETH Dominance: {:.2}%", global_metrics.eth_dominance);
        println!("  • SOL Dominance: {:.2}%", global_metrics.sol_dominance);
        println!("  • Layer 1 Dominance: {:.2}%", global_metrics.layer1_dominance);
        println!("  • Market Volatility: {:.2}%", global_metrics.volatility_index);

        Ok(MarketTechnicalData {
            btc_data,
            eth_data,
            sol_data,
            trending_data,
            global_metrics,
        })
    }

    fn calculate_ai_metrics(&self, trending_data: &[(String, TechnicalData)]) -> (f64, f64, f64) {
        // Calculate AI sector metrics
        let ai_tokens: Vec<_> = trending_data.iter()
            .filter(|(symbol, _)| {
                let s = symbol.to_lowercase();
                s.contains("ai") || 
                s.contains("ml") || 
                s.contains("graph") ||
                s.contains("ocean") ||
                s.contains("fetch")
            })
            .collect();

        if ai_tokens.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        // Calculate total volume
        let total_volume: f64 = ai_tokens.iter()
            .map(|(_, data)| data.candles.last().unwrap().volume)
            .sum();

        // Calculate average price change (growth)
        let avg_growth: f64 = ai_tokens.iter()
            .map(|(_, data)| {
                let candles = &data.candles;
                if candles.len() >= 2 {
                    let last = candles.last().unwrap();
                    let prev = &candles[candles.len() - 2];
                    ((last.close - prev.close) / prev.close) * 100.0
                } else {
                    0.0
                }
            })
            .sum::<f64>() / ai_tokens.len() as f64;

        // Calculate sector dominance (using market cap)
        let sector_mcap: f64 = ai_tokens.iter()
            .map(|(_, data)| data.candles.last().unwrap().close)
            .sum();

        // Return (dominance, volume, growth)
        (
            sector_mcap,  // Dominance (raw market cap sum)
            total_volume, // Volume
            avg_growth    // Growth percentage
        )
    }
} 