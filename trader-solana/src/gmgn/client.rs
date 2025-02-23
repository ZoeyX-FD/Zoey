use reqwest;
use std::error::Error;
use serde_json::Value;
use crate::gmgn::types::{
    TopHoldersResponse, 
    HolderInfo, 
    TokenInfoResponse, 
    TokenInfo, 
    WalletHoldingsResponse, 
    WalletHoldingsData, 
    SwapRankResponse, 
    TokenPriceInfo,
};

const BASE_URL: &str = "https://gmgn.mobi";
pub struct GMGNClient {
    client: reqwest::Client,
}

#[allow(dead_code)]
pub struct TokenPriceData {
    price: Option<f64>,
    price_change_percent: Option<f64>,
    volume: Option<f64>,
    liquidity: Option<f64>,
    market_cap: Option<f64>,
    price_change_percent1h: Option<f64>,
    price_change_percent5m: Option<f64>,
}

impl TokenPriceData {
    fn from_alternative_api(data: &serde_json::Value) -> Self {
        if let Some(first_rank) = data.get("data").and_then(|d| d.get("rank")).and_then(|r| r.get(0)) {
            Self {
                price: first_rank.get("price").and_then(|v| v.as_f64()),
                price_change_percent: first_rank.get("price_change_percent").and_then(|v| v.as_f64()),
                volume: first_rank.get("volume").and_then(|v| v.as_f64()),
                liquidity: first_rank.get("liquidity").and_then(|v| v.as_f64()),
                market_cap: first_rank.get("market_cap").and_then(|v| v.as_f64()),
                price_change_percent1h: first_rank.get("price_change_percent1h").and_then(|v| v.as_f64()),
                price_change_percent5m: first_rank.get("price_change_percent5m").and_then(|v| v.as_f64()),
            }
        } else {
            Self {
                price: None,
                price_change_percent: None,
                volume: None,
                liquidity: None,
                market_cap: None,
                price_change_percent1h: None,
                price_change_percent5m: None,
            }
        }
    }
}

impl GMGNClient {
    pub fn new() -> Self {
        let headers = {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("accept", "application/json, text/plain, */*".parse().unwrap());
            headers.insert("host", "gmgn.mobi".parse().unwrap());
            headers.insert("connection", "Keep-Alive".parse().unwrap());
            headers.insert("user-agent", "okhttp/4.9.2".parse().unwrap());
            headers
        };

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        Self { client }
    }

    pub async fn get_top_holders(
        &self, 
        contract_address: &str,
        limit: Option<u32>,
        cost: Option<u32>,
        orderby: Option<&str>,
        direction: Option<&str>
    ) -> Result<Vec<HolderInfo>, reqwest::Error> {
        let limit = limit.unwrap_or(20);
        let cost = cost.unwrap_or(20);
        let orderby = orderby.unwrap_or("amount_percentage");
        let direction = direction.unwrap_or("desc");

        let url = format!(
            "{BASE_URL}/defi/quotation/v1/tokens/top_holders/sol/{contract_address}?limit={limit}&cost={cost}&orderby={orderby}&direction={direction}"
        );
        let response = self.client.get(url).send().await?;
        let top_holders_response: TopHoldersResponse = response.json().await?;
        Ok(top_holders_response.data)
    }

    pub async fn get_token_info(&self, token: &str) -> Result<TokenInfo, reqwest::Error> {
        let url = format!("{BASE_URL}/api/v1/token_info/sol/{token}");
        let response = self.client.get(&url).send().await?;
        
        if std::env::var("DEBUG").is_ok() {
            let text = response.text().await?;
            println!("\nRaw API Response:");
            println!("{}", text);
            
            // We need to create a new request since we consumed the response
            let response = self.client.get(&url).send().await?;
            let token_info: TokenInfoResponse = response.json().await?;
            Ok(token_info.data)
        } else {
            let token_info: TokenInfoResponse = response.json().await?;
            Ok(token_info.data)
        }
    }

    pub async fn get_wallet_holdings(
        &self,
        wallet_address: &str,
        limit: Option<u32>,
        orderby: Option<&str>,
        direction: Option<&str>,
        showsmall: Option<bool>,
        sellout: Option<bool>,
        hide_abnormal: Option<bool>,
    ) -> Result<WalletHoldingsData, reqwest::Error> {
        let limit = limit.unwrap_or(50);
        let orderby = orderby.unwrap_or("last_active_timestamp");
        let direction = direction.unwrap_or("desc");
        let showsmall = showsmall.unwrap_or(false);
        let sellout = sellout.unwrap_or(false);
        let hide_abnormal = hide_abnormal.unwrap_or(false);

        let url = format!(
            "{BASE_URL}/api/v1/wallet_holdings/sol/{wallet_address}?limit={limit}&orderby={orderby}&direction={direction}&showsmall={showsmall}&sellout={sellout}&hide_abnormal={hide_abnormal}"
        );
        let response = self.client.get(url).send().await?;
        let holdings_response: WalletHoldingsResponse = response.json().await?;
        Ok(holdings_response.data)
    }

    pub async fn get_swap_rankings(
        &self, 
        time_period: &str, 
        launchpad: &str, 
        limit: Option<&str>,
    ) -> Result<SwapRankResponse, reqwest::Error> {
        let url = format!(
            "{BASE_URL}/defi/quotation/v1/rank/sol/swaps/{time_period}"
        );
        let params = vec![
            ("device_id", "1212e9167c96f7ee"),
            ("client_id", "gmgn_android_209000"), 
            ("from_app", "gmgn"),
            ("app_ver", "209000"),
            ("os", "android"),
            ("limit", limit.unwrap_or("20")),
            ("orderby", "marketcap"),
            ("direction", "desc"),
            ("filters[]", "renounced"),
            ("filters[]", "frozen")
        ];

        let response = self.client.get(url).query(&params).send().await?;
        let mut swap_rank_response: SwapRankResponse = response.json().await?;
        
        let launchpad = if launchpad.is_empty() { "Pump.fun" } else { launchpad };
        swap_rank_response.data.rank.retain(|token| token.launchpad.as_deref() == Some(launchpad));
        
        Ok(swap_rank_response)
    }

    pub async fn get_token_price_info(&self, token: &str) -> Result<TokenPriceInfo, reqwest::Error> {
        let url = format!("{BASE_URL}/api/v1/token_stats/sol/{token}");
        
        let params = vec![
            ("device_id", "ede3a881-1043-49aa-b645-b19080cb07da"),
            ("client_id", "gmgn_web_2025.0221.110436"),
            ("from_app", "gmgn"),
            ("app_ver", "2025.0221.110436"),
            ("tz_name", "Asia/Bangkok"),
            ("tz_offset", "25200"),
            ("app_lang", "en"),
        ];

        let response = self.client.get(&url)
            .query(&params)
            .header("accept", "*/*")
            .header("accept-language", "en-US,en;q=0.9")
            .header("content-type", "application/json")
            .header("sec-fetch-dest", "empty")
            .header("sec-fetch-mode", "cors")
            .header("sec-fetch-site", "same-origin")
            .header("x-requested-with", "XMLHttpRequest")
            .send()
            .await?;

        let text = response.text().await?;
        
        if std::env::var("DEBUG").is_ok() {
            println!("\nRaw Price API Response:");
            println!("{}", text);
        }

        // Try alternative endpoint if first one fails
        let alt_url = format!("{BASE_URL}/defi/quotation/v1/rank/sol/swaps/1h");
        let alt_params = vec![
            ("device_id", "ede3a881-1043-49aa-b645-b19080cb07da"),
            ("client_id", "gmgn_web_2025.0221.110436"),
            ("from_app", "gmgn"),
            ("app_ver", "2025.0221.110436"),
            ("tz_name", "Asia/Bangkok"),
            ("tz_offset", "25200"),
            ("app_lang", "en"),
            ("orderby", "swaps"),
            ("direction", "desc"),
            ("filters[]", "renounced"),
            ("filters[]", "frozen"),
            ("limit", "100")
        ];

        let alt_response = self.client.get(&alt_url)
            .query(&alt_params)
            .send()
            .await?;

        let alt_text = alt_response.text().await?;
        if std::env::var("DEBUG").is_ok() {
            println!("\nAlternative Price API Response:");
            println!("{}", alt_text);
        }

        if let Ok(rank_response) = serde_json::from_str::<SwapRankResponse>(&alt_text) {
            for t in rank_response.data.rank {
                if t.address == token {
                    return Ok(TokenPriceInfo {
                        price: Some(t.price),
                        market_cap: Some(t.market_cap),
                        volume: Some(t.volume),
                        price_change_24h: None,
                        price_change_1h: Some(t.price_change_percent1h),
                        price_change_5m: Some(t.price_change_percent5m),
                    });
                }
            }
        }

        Ok(TokenPriceInfo::default())
    }

    pub async fn analyze_token(&self, address: &str) -> Result<(), Box<dyn Error>> {
        // Get token info
        let _token_info = self.get_token_info(address).await?;
        
        // Get price info using the alternative endpoint
        let alt_url = format!("{BASE_URL}/defi/quotation/v1/rank/sol/swaps/1h");
        let alt_params = vec![
            ("device_id", "ede3a881-1043-49aa-b645-b19080cb07da"),
            ("client_id", "gmgn_web_2025.0221.110436"),
            ("from_app", "gmgn"),
            ("app_ver", "2025.0221.110436"),
            ("tz_name", "Asia/Bangkok"),
            ("tz_offset", "25200"),
            ("app_lang", "en"),
            ("orderby", "swaps"),
            ("direction", "desc"),
            ("filters[]", "renounced"),
            ("filters[]", "frozen"),
            ("limit", "100")
        ];

        let alt_response = self.client.get(&alt_url)
            .query(&alt_params)
            .send()
            .await?;

        let alt_text = alt_response.text().await?;
        let alternative_price_response: Value = serde_json::from_str(&alt_text)?;

        let price_data = TokenPriceData::from_alternative_api(&alternative_price_response);

        if cfg!(debug_assertions) {
            println!("\nRaw Price Data:");
            println!("- Price: {}", price_data.price.map_or("None".to_string(), |p| format!("{:.8}", p)));
            println!("- Market Cap: {}", price_data.market_cap.map_or("None".to_string(), |m| format!("${:.2}", m)));
            println!("- Volume 24h: {}", price_data.volume.map_or("None".to_string(), |v| format!("${:.2}", v)));
            println!("- Price Change 24h: {}", price_data.price_change_percent.map_or("None".to_string(), |c| format!("{:.2}%", c)));
            println!("- Price Change 1h: {}", price_data.price_change_percent1h.map_or("None".to_string(), |c| format!("{:.2}%", c)));
            println!("- Price Change 5m: {}", price_data.price_change_percent5m.map_or("None".to_string(), |c| format!("{:.2}%", c)));
        }

        // Get top holders
        let holders = self.get_top_holders(address, Some(5), None, None, None).await?;
        
        if cfg!(debug_assertions) {
            println!("\nRaw Holder Data:");
            for (i, holder) in holders.iter().enumerate() {
                println!("Holder {}: {{ address: {}, amount: {:?}, usd_value: {:?} }}", 
                    i + 1, 
                    holder.address, 
                    holder.amount_cur,
                    holder.usd_value
                );
            }
        }

        Ok(())
    }
}
