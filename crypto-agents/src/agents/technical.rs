use async_trait::async_trait;
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;

use crate::models::{MarketData, Conversation};
use super::{Agent, BaseAgent, ModelProvider};
use crate::api::coingecko::{DetailedCoinData, CoinGeckoClient, CandleData, TechnicalData, MarketTechnicalData};
use crate::api::social_media::SocialMediaPost;

const TECHNICAL_SYSTEM_PROMPT: &str = r#"
You are Agent One - The Technical Analysis Expert 📊
Your role is to analyze charts, patterns, and market indicators to identify trading opportunities.

Focus on:
- Price action and chart patterns
- Technical indicators (RSI, MACD, stochastics,bollinger bands, etc.)
- Volume analysis
- Support/resistance levels
- Short to medium-term opportunities

Remember to be specific about entry/exit points and always consider risk management rules! 🎯

Additional Analysis Requirements:
1. AI/ML Token Analysis:
   - Identify AI-related tokens in trending list
   - Compare against AI sector performance
   - Track correlations with tech indices

2. Market Structure:
   - Identify key liquidity levels
   - Track institutional order flows
   - Monitor DEX vs CEX volumes

3. Cross-Chain Analysis:
   - Track SOL vs ETH market share
   - Monitor L2 adoption metrics
   - Analyze cross-chain bridges volume

4. AI Sector Metrics:
   - AI token index performance
   - Development activity on AI projects
   - Partnership/integration announcements

Format your response like this:

🤖 Hey! Technical Analysis Agent here!
=================================

📊 Market Analysis:
[Your technical analysis in simple terms]

💡 Key Patterns:
- [Pattern 1]
- [Pattern 2]
- [Pattern 3]

🎯 Trading Opportunities:
1. [Clear opportunity with entry/exit]
2. [Clear opportunity with entry/exit]
3. [Clear opportunity with entry/exit]

⚠️ Risk Management:
[Risk management considerations]

🤖 AI Sector Analysis:
- Top AI projects performance
- Sector rotation trends
- Development milestones

🔄 Cross-Chain Dynamics:
- L1 competition analysis
- Bridge volume trends
- Protocol revenue comparison
"#;

#[derive(Debug)]
pub struct TechnicalAnalysis {
    pub analysis: String,
    pub market_outlook: String,
    pub risk_level: String,
}

pub struct TechnicalAgent {
    base: BaseAgent,
}

impl TechnicalAgent {
    pub async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        Ok(Self {
            base: BaseAgent::new(
                "Technical Agent".to_string(),
                model,
                TECHNICAL_SYSTEM_PROMPT.to_string(),
                provider
            )
            .await?
            .with_temperature(0.7),
        })
    }

    pub async fn analyze_coin_data(
        &self,
        symbol: &str,
        coin_data: &DetailedCoinData,
        sentiment_data: Option<&Vec<SocialMediaPost>>,
    ) -> Result<TechnicalAnalysis> {
        let client = CoinGeckoClient::new()?;
        
        // Get technical data with indicators
        let tech_data = client.get_market_chart(&coin_data.id, 14).await?;
        let candle_data = client.get_candle_data(&coin_data.id, 14).await?;

        let mut prompt = format!(
            "Analyze {} based on the following technical data:\n\n", 
            symbol
        );

        // Add price and basic market data
        prompt.push_str(&format!(
            "Price: ${:.2}\n\
             24h Change: {:.2}%\n\
             Market Cap: ${:.2}M\n",
            coin_data.current_price,
            coin_data.price_change_24h.unwrap_or(0.0),
            coin_data.market_cap / 1e6,
        ));

        // Add technical indicators
        prompt.push_str(&format!("\nTechnical Indicators:\n"));
        prompt.push_str(&format!("RSI (14): {:.2}\n", tech_data.rsi_14.unwrap_or_default()));
        prompt.push_str(&format!("50 MA: ${:.2}\n", tech_data.ma_50.unwrap_or_default()));
        prompt.push_str(&format!("200 MA: ${:.2}\n", tech_data.ma_200.unwrap_or_default()));
        
        // Add trend analysis
        prompt.push_str(&format!("\nTrend Analysis: {}\n", self.determine_trend(&tech_data)));

        // Add volume analysis
        let avg_volume: f64 = tech_data.candles.iter()
            .map(|c| c.volume)
            .sum::<f64>() / tech_data.candles.len() as f64;
        
        let latest_volume = tech_data.candles.last().unwrap().volume;
        let volume_change = (latest_volume - avg_volume) / avg_volume * 100.0;
        
        prompt.push_str(&format!("\nVolume Analysis:\n"));
        prompt.push_str(&format!("Current Volume: ${:.2}M\n", latest_volume / 1e6));
        prompt.push_str(&format!("Avg Volume (14d): ${:.2}M\n", avg_volume / 1e6));
        prompt.push_str(&format!("Volume Change: {:.2}%\n", volume_change));

        // Add candlestick patterns
        if let Some(pattern) = self.detect_patterns(&candle_data) {
            prompt.push_str(&format!("\nCandlestick Patterns:\n{}\n", pattern));
        }

        // Add sentiment data if available
        if let Some(posts) = sentiment_data {
            prompt.push_str("\nSocial Sentiment:\n");
            for post in posts.iter().take(5) {
                prompt.push_str(&format!("- {}\n", post.content));
            }
        }

        // Get analysis from model
        let response = self.base.generate_response(&prompt, None).await?;
        
        // Parse response sections
        let sections: Vec<&str> = response.split('\n').collect();
        let mut analysis = String::new();
        let mut market_outlook = String::from("Neutral");
        let mut risk_level = String::from("Medium");

        for section in sections {
            if section.starts_with("Technical Analysis:") {
                analysis = section.replace("Technical Analysis:", "").trim().to_string();
            } else if section.starts_with("Market Outlook:") {
                market_outlook = section.replace("Market Outlook:", "").trim().to_string();
            } else if section.starts_with("Risk Level:") {
                risk_level = section.replace("Risk Level:", "").trim().to_string();
            }
        }

        Ok(TechnicalAnalysis {
            analysis,
            market_outlook,
            risk_level,
        })
    }

    pub async fn analyze_market_technicals(
        &self,
        _market_data: &MarketData,
        technical_data: &MarketTechnicalData,
    ) -> Result<String> {
        let mut analysis = String::new();

        // Analyze BTC
        analysis.push_str(&self.analyze_major_coin(
            "BTC",
            &technical_data.btc_data,
            technical_data.global_metrics.btc_dominance
        ));

        // Analyze ETH
        analysis.push_str(&self.analyze_major_coin(
            "ETH",
            &technical_data.eth_data,
            0.0 // ETH dominance
        ));

        // Analyze trending coins
        analysis.push_str("\n🔥 Trending Coins Analysis:\n");
        for (symbol, tech_data) in &technical_data.trending_data {
            analysis.push_str(&format!("\n{} Analysis:\n", symbol));
            analysis.push_str(&self.analyze_trend_indicators(tech_data));
        }

        // Global market analysis
        analysis.push_str(&format!("\n📊 Global Market Metrics:\n"));
        analysis.push_str(&format!("Total Market Cap: ${:.2}B\n", technical_data.global_metrics.total_market_cap / 1e9));
        analysis.push_str(&format!("24h Volume: ${:.2}B\n", technical_data.global_metrics.total_volume_24h / 1e9));
        analysis.push_str(&format!("BTC Dominance: {:.2}%\n", technical_data.global_metrics.btc_dominance));
        analysis.push_str(&format!("24h Market Cap Change: {:.2}%\n", technical_data.global_metrics.market_cap_change_24h));

        Ok(analysis)
    }

    fn analyze_major_coin(&self, symbol: &str, data: &TechnicalData, dominance: f64) -> String {
        format!(
            "\n💎 {} Analysis:\n\
            RSI (14): {:.2}\n\
            50 MA: ${:.2}\n\
            200 MA: ${:.2}\n\
            Trend: {}\n\
            Market Dominance: {:.2}%\n",
            symbol,
            data.rsi_14.unwrap_or_default(),
            data.ma_50.unwrap_or_default(),
            data.ma_200.unwrap_or_default(),
            self.determine_trend(data),
            dominance
        )
    }

    // Helper function to determine trend
    fn determine_trend(&self, tech_data: &TechnicalData) -> &str {
        let price = tech_data.candles.last().unwrap().close;
        let ma50 = tech_data.ma_50.unwrap_or_default();
        let ma200 = tech_data.ma_200.unwrap_or_default();
        let rsi = tech_data.rsi_14.unwrap_or_default();

        match (price > ma50, price > ma200, rsi) {
            (true, true, _) if rsi > 70.0 => "Strong Uptrend (Overbought)",
            (true, true, _) => "Strong Uptrend",
            (true, false, _) => "Potential Trend Change (Above 50MA)",
            (false, true, _) => "Weakening Trend",
            (false, false, _) if rsi < 30.0 => "Strong Downtrend (Oversold)",
            (false, false, _) => "Strong Downtrend",
        }
    }

    // Helper function to detect candlestick patterns
    fn detect_patterns(&self, candles: &[CandleData]) -> Option<String> {
        if candles.len() < 3 {
            return None;
        }

        let mut patterns = Vec::new();
        let last = candles.len() - 1;

        // Doji
        if (candles[last].open - candles[last].close).abs() < 0.001 * candles[last].open {
            patterns.push("Doji (Indecision)");
        }

        // Hammer
        if candles[last].low < candles[last].open 
            && candles[last].low < candles[last].close
            && (candles[last].high - candles[last].low.max(candles[last].open.min(candles[last].close))) 
                < (candles[last].open.max(candles[last].close) - candles[last].low) * 0.3 {
            patterns.push("Hammer");
        }

        // Engulfing
        if candles[last].open > candles[last-1].close 
            && candles[last].close < candles[last-1].open {
            patterns.push("Bearish Engulfing");
        } else if candles[last].open < candles[last-1].close 
            && candles[last].close > candles[last-1].open {
            patterns.push("Bullish Engulfing");
        }

        if patterns.is_empty() {
            None
        } else {
            Some(patterns.join("\n"))
        }
    }

    fn analyze_trend_indicators(&self, tech_data: &TechnicalData) -> String {
        let mut analysis = String::new();
        
        // Get latest price and indicators
        let price = tech_data.candles.last().unwrap().close;
        let rsi = tech_data.rsi_14.unwrap_or_default();
        let ma50 = tech_data.ma_50.unwrap_or_default();
        let ma200 = tech_data.ma_200.unwrap_or_default();

        // Price analysis
        analysis.push_str(&format!("Price: ${:.2}\n", price));
        
        // RSI Analysis
        analysis.push_str(&format!("RSI (14): {:.2} - ", rsi));
        analysis.push_str(match rsi {
            r if r > 70.0 => "Overbought ⚠️\n",
            r if r < 30.0 => "Oversold 🔥\n",
            _ => "Neutral ⚖️\n",
        });

        // Moving Average Analysis
        analysis.push_str(&format!("50 MA: ${:.2}\n", ma50));
        analysis.push_str(&format!("200 MA: ${:.2}\n", ma200));
        
        // Trend Analysis
        analysis.push_str(&format!("Trend: {}\n", self.determine_trend(tech_data)));

        // Volume Analysis
        let avg_volume: f64 = tech_data.candles.iter()
            .map(|c| c.volume)
            .sum::<f64>() / tech_data.candles.len() as f64;
        
        let latest_volume = tech_data.candles.last().unwrap().volume;
        let volume_change = (latest_volume - avg_volume) / avg_volume * 100.0;
        
        analysis.push_str(&format!("Volume: ${:.2}M (", latest_volume / 1_000_000.0));
        analysis.push_str(&format!("{:+.2}% vs avg)\n", volume_change));

        // Pattern Detection
        if let Some(patterns) = self.detect_patterns(&tech_data.candles) {
            analysis.push_str("Patterns Detected:\n");
            analysis.push_str(&patterns);
            analysis.push_str("\n");
        }

        // Support/Resistance Levels
        let (support, resistance) = self.calculate_support_resistance(&tech_data.candles);
        analysis.push_str(&format!("Support: ${:.2}\n", support));
        analysis.push_str(&format!("Resistance: ${:.2}\n", resistance));

        analysis
    }

    fn calculate_support_resistance(&self, candles: &[CandleData]) -> (f64, f64) {
        let mut prices: Vec<f64> = candles.iter()
            .map(|c| c.close)
            .collect();
        prices.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let lower_quartile = prices[prices.len() / 4];
        let upper_quartile = prices[3 * prices.len() / 4];

        (lower_quartile, upper_quartile)
    }

    pub async fn think_with_data(&self, technical_data: &str) -> Result<String> {
        // Let the AI analyze the raw technical data
        let response = self.base.generate_response(technical_data, None).await?;
        Ok(response)
    }

    pub async fn analyze_technical_data(
        &self,
        _market_data: &MarketData,
        technical_data: &MarketTechnicalData,
    ) -> Result<String> {
        let mut context = String::new();

        // Add BTC data
        context.push_str(&format!("\nBitcoin Analysis:\n"));
        context.push_str(&self.format_coin_data("BTC", &technical_data.btc_data));

        // Add ETH data
        context.push_str(&format!("\nEthereum Analysis:\n"));
        context.push_str(&self.format_coin_data("ETH", &technical_data.eth_data));

        // Add SOL data
        context.push_str(&format!("\nSolana Analysis:\n"));
        context.push_str(&self.format_coin_data("SOL", &technical_data.sol_data));

        // Add trending coins
        context.push_str("\nTrending Coins:\n");
        for (symbol, data) in &technical_data.trending_data {
            context.push_str(&format!("\n{} Analysis:\n", symbol));
            context.push_str(&self.format_coin_data(symbol, data));
        }

        // Add global metrics
        context.push_str(&format!("\nGlobal Market Status:\n"));
        context.push_str(&format!("Total Market Cap: ${:.2}B\n", technical_data.global_metrics.total_market_cap / 1e9));
        context.push_str(&format!("BTC Dominance: {:.2}%\n", technical_data.global_metrics.btc_dominance));
        context.push_str(&format!("24h Volume: ${:.2}B\n", technical_data.global_metrics.total_volume_24h / 1e9));
        context.push_str(&format!("Market Cap Change: {:.2}%\n", technical_data.global_metrics.market_cap_change_24h));

        // Get AI analysis
        let response = self.base.generate_response("Analyze the current market conditions from a technical analysis perspective.", Some(&context)).await?;
        
        Ok(response)
    }

    fn format_coin_data(&self, symbol: &str, data: &TechnicalData) -> String {
        let mut analysis = String::new();
        
        let price = data.candles.last().unwrap().close;
        let rsi = data.rsi_14.unwrap_or_default();
        let ma50 = data.ma_50.unwrap_or_default();
        let ma200 = data.ma_200.unwrap_or_default();

        analysis.push_str(&format!("💎 {} Analysis:\n", symbol));
        analysis.push_str(&format!("Price: ${:.2}\n", price));
        analysis.push_str(&format!("RSI (14): {:.2}\n", rsi));
        analysis.push_str(&format!("50 MA: ${:.2}\n", ma50));
        analysis.push_str(&format!("200 MA: ${:.2}\n", ma200));
        analysis.push_str(&format!("Trend: {}\n", self.determine_trend(data)));

        if let Some(patterns) = self.detect_patterns(&data.candles) {
            analysis.push_str(&format!("Patterns: {}\n", patterns));
        }

        analysis
    }
}

#[async_trait]
impl Agent for TechnicalAgent {
    fn name(&self) -> &str {
        &self.base.name
    }
    
    fn model(&self) -> &str {
        &self.base.model
    }
    
    async fn think(&mut self, market_data: &MarketData, previous_message: Option<String>) -> Result<String> {
        // Create context from market data
        let context = serde_json::to_string_pretty(market_data)?;
        
        let prompt = "Analyze the current market conditions from a technical analysis perspective.";
        
        let response = self.base.generate_response(prompt, Some(&context)).await?;
        
        // Save to memory
        self.base.memory.conversations.push(Conversation {
            timestamp: Utc::now(),
            market_data: market_data.clone(),
            other_message: previous_message,
            response: response.clone(),
        });
        
        self.save_memory().await?;
        
        Ok(response)
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