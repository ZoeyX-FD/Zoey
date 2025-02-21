use reqwest;
use crate::gmgn::types::{
    TopHoldersResponse, 
    HolderInfo, 
    TokenInfoResponse, 
    TokenInfo, 
    WalletHoldingsResponse, 
    WalletHoldingsData, 
    SwapRankResponse, 
    TokenPriceInfo, 
    TokenDetailResponse
};

const BASE_URL: &str = "https://gmgn.mobi";
pub struct GMGNClient {
    client: reqwest::Client,
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
        // Try multiple time periods to find the token
        let time_periods = ["1h", "24h", "7d"];
        let mut token_data = TokenPriceInfo::default();
        let mut raw_response = String::new();
        let debug_mode = std::env::var("DEBUG").is_ok();

        for period in time_periods {
            let url = format!("{BASE_URL}/defi/quotation/v1/rank/sol/swaps/{period}");
            
            let params = vec![
                ("device_id", "ede3a881-1043-49aa-b645-b19080cb07da"),
                ("client_id", "gmgn_web_2025.0220.193826"), 
                ("from_app", "gmgn"),
                ("app_ver", "2025.0220.193826"),
                ("tz_name", "Asia/Bangkok"),
                ("tz_offset", "25200"),
                ("app_lang", "en"),
                ("orderby", "swaps"),
                ("direction", "desc"),
                ("filters[]", "renounced"),
                ("filters[]", "frozen"),
                ("limit", "100")
            ];

            let response = self.client.get(&url)
                .query(&params)
                .send()
                .await?;

            let text = response.text().await?;
            if debug_mode {
                raw_response = text.clone();
            }

            if let Ok(rank_response) = serde_json::from_str::<SwapRankResponse>(&text) {
                if let Some(t) = rank_response.data.rank.into_iter().find(|t| t.address == token) {
                    token_data = TokenPriceInfo {
                        price: Some(t.price),
                        market_cap: Some(t.market_cap as f64),
                        volume: Some(t.volume as f64),
                        price_change_24h: None,
                        price_change_1h: Some(t.price_change_percent),
                        price_change_5m: Some(t.price_change_percent5m),
                    };
                    break;
                }
            }
        }

        // Try alternative endpoint if token not found
        if token_data == TokenPriceInfo::default() {
            let alt_url = format!("{BASE_URL}/defi/quotation/v1/tokens/detail/sol/{token}");
            let response = self.client.get(&alt_url)
                .query(&[
                    ("device_id", "ede3a881-1043-49aa-b645-b19080cb07da"),
                    ("client_id", "gmgn_web_2025.0220.193826")
                ])
                .send()
                .await?;

            let text = response.text().await?;
            if debug_mode {
                raw_response = text.clone();
            }

            if let Ok(detail_response) = serde_json::from_str::<TokenDetailResponse>(&text) {
                token_data = TokenPriceInfo {
                    price: detail_response.data.price,
                    market_cap: detail_response.data.market_cap,
                    volume: detail_response.data.volume_24h,
                    price_change_24h: detail_response.data.price_change_24h,
                    price_change_1h: detail_response.data.price_change_1h,
                    price_change_5m: detail_response.data.price_change_5m,
                };
            }
        }

        // Only print raw response in debug mode
        if debug_mode {
            println!("\nRaw Price API Response:");
            println!("{}", raw_response);
        }
        
        Ok(token_data)
    }
}
