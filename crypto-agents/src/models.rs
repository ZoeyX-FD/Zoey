use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub overview: GlobalData,
    pub trending: Vec<TrendingCoin>,
    pub bitcoin: CoinData,
    pub ethereum: CoinData,
    pub recent_history: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalData {
    pub total_market_cap: f64,
    pub total_volume: f64,
    pub market_cap_change_percentage_24h: f64,
    pub active_cryptocurrencies: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendingCoin {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub price_btc: f64,
    #[serde(default)]
    pub market_cap_rank: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinData {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub current_price: f64,
    pub market_cap: f64,
    pub price_change_24h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub conversations: Vec<Conversation>,
    pub decisions: Vec<Decision>,
    pub portfolio_history: Vec<PortfolioUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub timestamp: DateTime<Utc>,
    pub market_data: MarketData,
    pub other_message: Option<String>,
    pub response: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub timestamp: DateTime<Utc>,
    pub action: String,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioUpdate {
    pub timestamp: DateTime<Utc>,
    pub total_value: f64,
    pub holdings: Vec<Holding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Holding {
    pub symbol: String,
    pub amount: f64,
    pub value_usd: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Memory storage error: {0}")]
    StorageError(String),
    
    #[error("Model error: {0}")]
    ModelError(String),
    
    #[error("Invalid data: {0}")]
    InvalidData(String),
} 