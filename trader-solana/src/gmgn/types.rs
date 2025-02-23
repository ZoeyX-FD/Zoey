use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct TopHoldersResponse {
    pub code: i32,
    pub msg: String,
    pub data: Vec<HolderInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HolderInfo {
    pub address: String,
    #[serde(default)]
    pub addr_type: Option<i32>,
    #[serde(default)]
    pub amount_cur: Option<f64>,
    #[serde(default)]
    pub usd_value: Option<f64>,
    #[serde(default)]
    pub cost_cur: Option<f64>,
    #[serde(default)]
    pub sell_amount_cur: Option<f64>,
    #[serde(default)]
    pub sell_amount_percentage: Option<f64>,
    #[serde(default)]
    pub sell_volume_cur: Option<f64>,
    #[serde(default)]
    pub buy_volume_cur: Option<f64>,
    #[serde(default)]
    pub buy_amount_cur: Option<f64>,
    #[serde(default)]
    pub netflow_usd: Option<f64>,
    #[serde(default)]
    pub netflow_amount: Option<f64>,
    #[serde(default)]
    pub buy_tx_count_cur: Option<i32>,
    #[serde(default)]
    pub sell_tx_count_cur: Option<i32>,
    pub wallet_tag_v2: String,
    pub eth_balance: String,
    pub sol_balance: String,
    pub trx_balance: String,
    pub balance: String,
    #[serde(default)]
    pub profit: Option<f64>,
    #[serde(default)]
    pub realized_profit: Option<f64>,
    #[serde(default)]
    pub unrealized_profit: Option<f64>,
    pub profit_change: Option<f64>,
    #[serde(default)]
    pub amount_percentage: Option<f64>,
    pub avg_cost: Option<f64>,
    pub avg_sold: Option<f64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub maker_token_tags: Vec<String>,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub twitter_username: Option<String>,
    pub twitter_name: Option<String>,
    pub tag_rank: HashMap<String, Option<i32>>,
    pub last_active_timestamp: Option<i64>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub accu_amount: Option<f64>,
    #[serde(default)]
    pub accu_cost: Option<f64>,
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub total_cost: Option<f64>,
    pub transfer_in: bool,
    pub is_new: bool,
    pub native_transfer: NativeTransfer,
    pub is_suspicious: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NativeTransfer {
    pub name: Option<String>,
    pub from_address: Option<String>,
    #[serde(default)]
    pub timestamp: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TokenInfoResponse {
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: TokenInfo,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TokenInfo {
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub decimals: Option<i32>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub biggest_pool_address: Option<String>,
    #[serde(default)]
    pub open_timestamp: Option<i64>,
    #[serde(default)]
    pub holder_count: Option<i32>,
    #[serde(default)]
    pub circulating_supply: Option<String>,
    #[serde(default)]
    pub total_supply: Option<String>,
    #[serde(default)]
    pub max_supply: Option<String>,
    #[serde(default)]
    pub liquidity: Option<String>,
    #[serde(default)]
    pub creation_timestamp: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletHoldingsResponse {
    pub code: i32,
    pub reason: String,
    pub message: String,
    pub data: WalletHoldingsData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletHoldingsData {
    pub holdings: Vec<HoldingInfo>,
    pub next: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HoldingInfo {
    pub token: TokenHoldingInfo,
    pub balance: String,
    pub usd_value: String,
    pub realized_profit_30d: String,
    pub realized_profit: String,
    pub realized_pnl: String,
    pub realized_pnl_30d: String,
    pub unrealized_profit: String,
    pub unrealized_pnl: String,
    pub total_profit: String,
    pub total_profit_pnl: String,
    pub avg_cost: String,
    pub avg_sold: String,
    pub buy_30d: i32,
    pub sell_30d: i32,
    pub sells: i32,
    pub price: String,
    pub cost: String,
    pub position_percent: String,
    pub last_active_timestamp: i64,
    pub history_sold_income: String,
    pub history_bought_cost: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenHoldingInfo {
    pub address: String,
    pub token_address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: i32,
    pub logo: String,
    pub price_change_6h: String,
    pub is_show_alert: bool,
    pub is_honeypot: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapRankResponse {
    pub code: i32,
    pub msg: String,
    pub data: SwapRankData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapRankData {
    pub rank: Vec<TokenRankInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenRankInfo {
    pub id: i64,
    pub chain: String,
    pub address: String,
    pub symbol: String,
    pub price: f64,
    pub price_change_percent: f64,
    pub swaps: f64,
    pub volume: f64,
    pub liquidity: f64,
    pub market_cap: f64,
    pub hot_level: i32,
    pub pool_creation_timestamp: i64,
    pub holder_count: f64,
    pub twitter_username: Option<String>,
    pub website: Option<String>,
    pub telegram: Option<String>,
    pub open_timestamp: i64,
    pub price_change_percent1m: f64,
    pub price_change_percent5m: f64,
    pub price_change_percent1h: f64,
    pub buys: f64,
    pub sells: f64,
    pub initial_liquidity: f64,
    pub is_show_alert: bool,
    pub top_10_holder_rate: f64,
    pub renounced_mint: i32,
    pub renounced_freeze_account: i32,
    pub burn_ratio: Option<String>,
    pub burn_status: Option<String>,
    pub launchpad: Option<String>,
    pub dev_token_burn_amount: Option<String>,
    pub dev_token_burn_ratio: Option<f64>,
    pub dexscr_ad: i32,
    pub dexscr_update_link: i32,
    pub cto_flag: i32,
    pub twitter_change_flag: i32,
    pub creator_token_status: Option<String>,
    pub creator_close: Option<bool>,
    pub launchpad_status: i32,
    pub rat_trader_amount_rate: f64,
    pub bluechip_owner_percentage: f64,
    pub smart_degen_count: u32,
    pub renowned_count: f64,
    pub is_wash_trading: bool,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct TokenPriceInfo {
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub market_cap: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub price_change_24h: Option<f64>,
    #[serde(default)]
    pub price_change_1h: Option<f64>,
    #[serde(default)]
    pub price_change_5m: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TokenPriceResponse {
    pub code: i32,
    pub reason: String,
    pub message: String,
    pub data: TokenPriceInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenDetailResponse {
    pub code: i32,
    pub reason: String,
    pub message: String,
    pub data: TokenDetailData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenDetailData {
    pub price: Option<f64>,
    pub market_cap: Option<f64>,
    pub volume_24h: Option<f64>,
    pub price_change_24h: Option<f64>,
    pub price_change_1h: Option<f64>,
    pub price_change_5m: Option<f64>,
    // Add other fields as needed
}

pub fn print_debug_info(info: &TokenInfo, price_info: Option<&TokenPriceInfo>, holders: &[HolderInfo]) {
    println!("\n=== DEBUG INFO ===");
    println!("Raw Token Data:");
    println!("- Address: {:?}", info.address);
    println!("- Symbol: {:?}", info.symbol);
    println!("- Name: {:?}", info.name);
    println!("- Decimals: {:?}", info.decimals);
    println!("- Holder Count: {:?}", info.holder_count);
    println!("- Liquidity: {:?}", info.liquidity);
    println!("- Total Supply: {:?}", info.total_supply);
    println!("- Circulating Supply: {:?}", info.circulating_supply);

    if let Some(price) = price_info {
        println!("\nRaw Price Data:");
        println!("- Price: {:?}", price.price);
        println!("- Market Cap: {:?}", price.market_cap);
        println!("- Volume 24h: {:?}", price.volume);
        println!("- Price Change 24h: {:?}", price.price_change_24h);
        println!("- Price Change 1h: {:?}", price.price_change_1h);
        println!("- Price Change 5m: {:?}", price.price_change_5m);
    }

    println!("\nRaw Holder Data:");
    for (i, holder) in holders.iter().take(5).enumerate() {
        println!("Holder {}: {{ address: {}, amount: {:?}, percentage: {:?} }}", 
            i + 1, 
            holder.address, 
            holder.amount_cur, 
            holder.amount_percentage
        );
    }
    println!("================\n");
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenCompleteResponse {
    pub code: i32,
    pub reason: String,
    pub message: String,
    pub data: Vec<TokenCompleteData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenCompleteData {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: i32,
    pub logo: Option<String>,
    pub biggest_pool_address: String,
    pub open_timestamp: i64,
    pub holder_count: i64,
    pub circulating_supply: String,
    pub total_supply: String,
    pub max_supply: String,
    pub liquidity: String,
    pub creation_timestamp: i64,
    pub price: TokenPriceDetails,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenPriceDetails {
    pub address: String,
    pub price: String,
    pub price_1m: String,
    pub price_5m: String,
    pub price_1h: String,
    pub price_6h: String,
    pub price_24h: String,
    pub volume_1m: String,
    pub volume_5m: String,
    pub volume_1h: String,
    pub volume_6h: String,
    pub volume_24h: String,
    pub swaps_1m: i64,
    pub swaps_5m: i64,
    pub swaps_1h: i64,
    pub swaps_6h: i64,
    pub swaps_24h: i64,
    pub hot_level: i32,
}
