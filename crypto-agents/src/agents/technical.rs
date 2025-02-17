use async_trait::async_trait;
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;

use crate::models::{MarketData, Conversation};
use super::{Agent, BaseAgent, ModelProvider};
use crate::api::coingecko::{DetailedCoinData, CoinGeckoClient, CandleData, TechnicalData, MarketTechnicalData};

const TECHNICAL_SYSTEM_PROMPT: &str = r#"
You are Agent One - The Technical Analysis and Trader Expert 📊
Your role is to analyze charts, patterns, and market indicators to identify trading opportunities.

Focus on:
- Price action and chart patterns
- Technical indicators (RSI, MACD, stochastics, bollinger bands, etc.)
- if only have raw limited data i want you to calculate the technical indicators
- Volume analysis
- Support/resistance levels
- Short to medium-term opportunities

Format your response like this:

📊 Market Analysis:
[Detailed market analysis including price action and trends]

💡 Technical Indicators:
📊 OHLC (24h):
- Open: [Opening price]
- High: [Highest price]
- Low: [Lowest price]
- Close: [Current price]

 Moving Averages:\n\
- MA50: ${:.2}\n\
- MA200: ${:.2}\n\
- Position: {}\n\


🎯 Key Levels:
- Support: [List key support levels]
- Resistance: [List key resistance levels]

⚠️ Risk Assessment:
[Detailed risk analysis]

🎯 Trading Recommendations:
- Entry Points: [Specific prices]
- Exit Points: [Specific prices]
- Stop Loss: [Specific prices]

💫 Short-term Outlook:
[Clear price prediction with reasoning]

🎯 Confidence Metrics:
- Bullish Scenario Confidence: [0-100%]
- Bearish Scenario Confidence: [0-100%]
- Overall Analysis Confidence: [0-100%]
- Key Support/Resistance Confidence: [0-100%]

🎯 FINAL RECOMMENDATION:
Action: [MUST BE ONE OF: "BUY" / "SELL" / "WAIT"]
Timeframe: [Short-term/Mid-term/Long-term]
Reasoning: [Clear, concise reasons for the recommendation]
Entry Price: [Specify price range for entry, even if WAIT]
Stop Loss: [Specify stop loss level for potential entry]
Target Prices: 
  - Target 1 (Conservative): $X.XX (XX% gain)
  - Target 2 (Moderate): $X.XX (XX% gain)
  - Target 3 (Aggressive): $X.XX (XX% gain)
Risk Level: [Low/Medium/High]
Risk/Reward Ratio: [Calculate based on entry to targets]
Position Size Recommendation: [Based on risk level]

🚨 Final Note:
[Key risks and important monitoring points]
[Include specific price levels to watch]
[Add market correlation warnings if relevant]
[Mention any upcoming events/catalysts]

⚠️ DISCLAIMER:
This analysis is for informational purposes only. Cryptocurrency trading involves 
substantial risk of loss and is not suitable for every investor. The valuation 
and prices of cryptocurrencies may fluctuate based on various factors beyond 
technical analysis. Always conduct your own research and consider your investment 
objectives before making any trading decisions.

💭 QUOTE OF THE ANALYSIS:
[Generate a short, insightful quote about trading/investing that relates to the current analysis.
The quote should be wise, memorable, and specific to the current market conditions.
Format: "Quote" - Source/Context]
"#;

#[derive(Debug)]
pub struct TechnicalAnalysis {
    pub analysis: String,
    pub market_outlook: String,
    pub risk_level: String,
    pub quote: String,
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

    pub async fn analyze_coin_data(&self, symbol: &str, data: &DetailedCoinData) -> Result<TechnicalAnalysis> {
        // Get OHLC data
        let client = CoinGeckoClient::new()?;
        let ohlc_data = client.get_ohlc_data(&data.id, 1).await?;
        
        // Get the latest OHLC candle
        let latest_ohlc = ohlc_data.last()
            .ok_or_else(|| anyhow::anyhow!("No OHLC data available"))?;

        let prompt = format!(
            "Perform a detailed technical analysis for {} with the following market data:\n\n\
            Current Price: ${:.2}\n\
            24h Change: {:.2}%\n\
            Market Cap: ${:.2}M\n\
            24h Volume: ${:.2}M\n\
            OHLC Data (24h):\n\
            - Open: ${:.2}\n\
            - High: ${:.2}\n\
            - Low: ${:.2}\n\
            - Close: ${:.2}\n\
            Moving Averages:\n\
            - MA50: ${:.2}\n\
            - MA200: ${:.2}\n\
            - Position: {}\n\
            \n\
            Please analyze:\n\
            1. Overall Market Analysis\n\
            2. Key Support and Resistance Levels\n\
            3. Volume Analysis\n\
            4. Technical Indicators Assessment (including OHLC)\n\
            5. Short-term Price Outlook\n\
            6. Risk Assessment\n\
            7. Trading Recommendations\n\
            8. Confidence Levels for Each Scenario\n\
            9. EXPLICIT RECOMMENDATION (BUY/SELL/WAIT) with clear reasoning\n\
            \n\
            For each analysis point, include a confidence level (0-100%) based on:\n\
            - Technical indicator alignment\n\
            - Volume confirmation\n\
            - Historical pattern reliability\n\
            - Market condition similarity\n\
            \n\
            End your analysis with a clear actionable recommendation and risk disclaimer.\n\
            Format your response using the template from the system prompt.",
            symbol,
            data.current_price,
            data.price_change_24h.unwrap_or_default(),
            data.market_cap / 1_000_000.0,
            data.volume_24h / 1_000_000.0,
            latest_ohlc.open,
            latest_ohlc.high,
            latest_ohlc.low,
            latest_ohlc.close,
            data.ma_50.unwrap_or_default(),
            data.ma_200.unwrap_or_default(),
            if data.current_price > data.ma_50.unwrap_or_default() {
                "Above MA50"
            } else {
                "Below MA50"
            }
        );

        // Get AI response
        let response = self.base.generate_response(&prompt, None).await?;
        
        // Parse market outlook and risk level
        let market_outlook = self.extract_market_outlook(&response);

        let risk_level = if response.to_lowercase().contains("high risk") {
            "High"
        } else if response.to_lowercase().contains("low risk") {
            "Low"
        } else {
            "Medium"
        }.to_string();

        Ok(TechnicalAnalysis {
            analysis: response,
            market_outlook,
            risk_level,
            quote: String::new(),
        })
    }

    pub async fn analyze_market_technicals(
        &self,
        technical_data: &MarketTechnicalData,
        category_volumes: (f64, f64, f64, f64)
    ) -> Result<String> {
        let (ai_vol, l1_vol, l2_vol, rwa_vol) = category_volumes;
        
        // Add sector volume data to analysis context
        let mut context = format!(
            "\n🔄 Sector Volumes (24h):\n\
             AI: ${:.2}B\nLayer 1: ${:.2}B\nLayer 2: ${:.2}B\nRWA: ${:.2}B",
            ai_vol / 1e9, l1_vol / 1e9, l2_vol / 1e9, rwa_vol / 1e9
        );

        // Add BTC analysis using full technical data
        context.push_str(&format!("\n\n💎 Bitcoin Analysis:"));
        context.push_str(&self.analyze_major_coin(
            "BTC", 
            &technical_data.btc_data,
            technical_data.global_metrics.btc_dominance
        ));

        // Add ETH analysis
        context.push_str(&format!("\n\n💎 Ethereum Analysis:"));
        context.push_str(&self.analyze_major_coin(
            "ETH",
            &technical_data.eth_data,
            0.0 // ETH dominance
        ));

        // Analyze trending coins
        context.push_str("\n🔥 Trending Coins Analysis:\n");
        for (symbol, tech_data) in &technical_data.trending_data {
            context.push_str(&format!("\n{} Analysis:\n", symbol));
            context.push_str(&self.analyze_trend_indicators(tech_data));
        }

        // Global market analysis
        context.push_str(&format!("\n📊 Global Market Metrics:\n"));
        context.push_str(&format!("Total Market Cap: ${:.2}B\n", technical_data.global_metrics.total_market_cap / 1e9));
        context.push_str(&format!("24h Volume: ${:.2}B\n", technical_data.global_metrics.total_volume_24h / 1e9));
        context.push_str(&format!("BTC Dominance: {:.2}%\n", technical_data.global_metrics.btc_dominance));
        context.push_str(&format!("24h Market Cap Change: {:.2}%\n", technical_data.global_metrics.market_cap_change_24h));

        // Add sector analysis
        let sector_analysis = self.analyze_sector(
            "AI Sector",
            ai_vol,
            technical_data.global_metrics.ai_sector_dominance,
            &["FET", "AGIX", "OCEAN"]
        );
        
        context.push_str(&sector_analysis);
        
        // Repeat for other sectors...

        Ok(context)
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

        // Add market overview with more detailed metrics
        context.push_str(&format!("\n📊 Market Overview:\n"));
        context.push_str(&format!(
            "• Total Market Cap: ${:.2}B\n\
             • BTC Dominance: {:.2}%\n\
             • Market Cap Change: {:.2}%\n\
             • Volume Change: {:.2}%\n\
             • Volatility Index: {:.2}\n",
            technical_data.global_metrics.total_market_cap / 1e9,
            technical_data.global_metrics.btc_dominance,
            technical_data.global_metrics.market_cap_change_24h,
            technical_data.global_metrics.volume_change_24h,
            technical_data.global_metrics.volatility_index
        ));

        // Add sector analysis
        context.push_str("\n🔍 Sector Analysis:\n");
        context.push_str(&format!(
            "• AI Sector: {:.2}% dominance (Volume: ${:.2}B)\n\
             • Layer 1: {:.2}% dominance\n\
             • DeFi: {:.2}% dominance\n\
             • RWA: {:.2}% dominance\n",
            technical_data.global_metrics.ai_sector_dominance,
            technical_data.global_metrics.ai_sector_volume / 1e9,
            technical_data.global_metrics.layer1_dominance,
            technical_data.global_metrics.defi_dominance,
            technical_data.global_metrics.rwa_sector_dominance
        ));

        // Enhanced coin analysis
        let major_coins = [
            ("BTC", &technical_data.btc_data),
            ("ETH", &technical_data.eth_data),
            ("SOL", &technical_data.sol_data)
        ];

        for (symbol, data) in major_coins {
            context.push_str(&format!("\n💎 {} Analysis:\n", symbol));
            context.push_str(&self.format_enhanced_coin_data(symbol, data));
        }

        // Add trending coins with enhanced analysis
        context.push_str("\n🔥 Trending Coins:\n");
        for (symbol, data) in &technical_data.trending_data {
            context.push_str(&format!("\n{} Analysis:\n", symbol));
            context.push_str(&self.format_enhanced_coin_data(symbol, data));
        }

        // Get AI analysis with enhanced prompt
        let response = self.base.generate_response(
            "Analyze the current market conditions from a technical analysis perspective. \
             Focus on identifying key patterns, trend strength, and potential trading opportunities. \
             Include specific price targets and risk levels.",
            Some(&context)
        ).await?;
        
        Ok(response)
    }

    fn format_enhanced_coin_data(&self, _symbol: &str, data: &TechnicalData) -> String {
        let mut analysis = String::new();
        
        let price = data.current_price.unwrap_or_else(|| data.candles.last().unwrap().close);
        let rsi = data.rsi_14.unwrap_or_default();
        let ma50 = data.ma_50.unwrap_or_default();
        let ma200 = data.ma_200.unwrap_or_default();

        // Enhanced price analysis
        analysis.push_str(&format!("• Price: ${:.2}\n", price));
        if let Some(change) = data.price_change_24h {
            analysis.push_str(&format!("• 24h Change: {:.2}%\n", change));
        }

        // Enhanced technical indicators
        analysis.push_str(&format!("• RSI (14): {:.2} - {}\n", rsi, self.interpret_rsi(rsi)));
        
        // Enhanced MA analysis with crossover detection
        analysis.push_str(&format!("• 50 MA: ${:.2}\n", ma50));
        analysis.push_str(&format!("• 200 MA: ${:.2}\n", ma200));
        analysis.push_str(&format!("• MA Status: {}\n", self.analyze_ma_status(price, ma50, ma200)));

        // Add MACD analysis
        if let Some((macd, signal, hist)) = data.macd {
            analysis.push_str(&format!("• MACD: {:.2}/{:.2}/{:.2} - {}\n", 
                macd, signal, hist, 
                if hist > 0.0 { "Bullish" } else { "Bearish" }
            ));
        }

        // Add Bollinger Bands analysis
        if let Some((upper, middle, lower)) = data.bollinger_bands {
            analysis.push_str(&format!("• Bollinger Bands: {}\n", 
                self.analyze_bollinger_bands(price, upper, middle, lower)
            ));
        }

        // Enhanced volume analysis
        if let Some(volume) = data.volume_24h {
            analysis.push_str(&format!("• Volume: ${:.2}B ({})\n", 
                volume / 1e9,
                self.determine_volume_trend(volume)
            ));
        }

        // Add pattern detection
        if let Some(patterns) = self.detect_patterns(&data.candles) {
            analysis.push_str(&format!("• Patterns: {}\n", patterns));
        }

        analysis
    }

    pub async fn analyze_market_data(&self, client: &CoinGeckoClient) -> Result<TechnicalAnalysis> {
        // Get comprehensive technical data including sectors
        let market_data = client.get_technical_analysis().await?;
        let category_volumes = client.get_category_volumes().await?;

        let mut prompt = "Analyze current market conditions based on the following data:\n\n".to_string();

        // Add sector analysis using category_volumes
        let (ai_volume, l1_volume, l2_volume, rwa_volume) = category_volumes;
        
        prompt.push_str(&format!(
            "\n🔄 Sector Volumes (24h):\n\
             • AI Sector: ${:.2}B\n\
             • Layer 1: ${:.2}B\n\
             • Layer 2: ${:.2}B\n\
             • RWA: ${:.2}B\n",
            ai_volume / 1e9,
            l1_volume / 1e9,
            l2_volume / 1e9,
            rwa_volume / 1e9
        ));

        // Add overall market metrics
        prompt.push_str(&format!(
            "Market Overview:\n\
             Total Market Cap: ${:.2}B\n\
             BTC Dominance: {:.2}%\n\
             ETH Dominance: {:.2}%\n\
             SOL Dominance: {:.2}%\n\
             24h Volume: ${:.2}B\n\
             Market Cap Change 24h: {:.2}%\n\n",
            market_data.global_metrics.total_market_cap / 1e9,
            market_data.global_metrics.btc_dominance,
            market_data.global_metrics.eth_dominance,
            market_data.global_metrics.sol_dominance,
            market_data.global_metrics.total_volume_24h / 1e9,
            market_data.global_metrics.market_cap_change_24h,
        ));

        // Add sector analysis
        prompt.push_str("Sector Analysis:\n\n");
        
        // AI Sector
        prompt.push_str(&format!(
            "🤖 AI Sector:\n\
             • Market Cap: ${:.2}B\n\
             • Volume 24h: ${:.2}B\n\
             • Dominance: {:.2}%\n\
             • Growth: {:.2}%\n\n",
            market_data.global_metrics.ai_sector_volume / 1e9,
            market_data.global_metrics.ai_sector_volume / 1e9,
            market_data.global_metrics.ai_sector_dominance,
            market_data.global_metrics.ai_sector_growth,
        ));

        // Layer 1 Sector
        prompt.push_str(&format!(
            "🔗 Layer 1 Sector:\n\
             • Market Cap: ${:.2}B\n\
             • Volume 24h: ${:.2}B\n\
             • Dominance: {:.2}%\n\n",
            market_data.global_metrics.total_market_cap * market_data.global_metrics.layer1_dominance / 100.0 / 1e9,
            market_data.global_metrics.total_volume_24h * market_data.global_metrics.layer1_dominance / 100.0 / 1e9,
            market_data.global_metrics.layer1_dominance,
        ));

        // Layer 2 Sector
        prompt.push_str(&format!(
            "⚡ Layer 2 Sector:\n\
             • Volume 24h: ${:.2}B\n\
             • Cross-chain Volume: ${:.2}B\n\n",
            market_data.global_metrics.cross_chain_volume / 1e9,
            market_data.global_metrics.cross_chain_volume / 1e9,
        ));

        // RWA Sector
        prompt.push_str(&format!(
            "💎 RWA Sector:\n\
             • Volume 24h: ${:.2}B\n\
             • Dominance: {:.2}%\n\n",
            market_data.global_metrics.rwa_sector_volume / 1e9,
            market_data.global_metrics.rwa_sector_dominance,
        ));

        // Add major coins analysis
        prompt.push_str("Major Coins Analysis:\n\n");
        
        // BTC Analysis
        self.add_coin_analysis(&mut prompt, "BTC", &market_data.btc_data);
        
        // ETH Analysis
        self.add_coin_analysis(&mut prompt, "ETH", &market_data.eth_data);
        
        // SOL Analysis
        self.add_coin_analysis(&mut prompt, "SOL", &market_data.sol_data);

        // Add trending coins
        prompt.push_str("\nTrending Coins:\n");
        for (symbol, data) in &market_data.trending_data {
            self.add_coin_analysis(&mut prompt, symbol, data);
        }

        // Get analysis from LLM
        let response = self.base.generate_response(&prompt, None).await?;

        Ok(TechnicalAnalysis {
            analysis: response.clone(),
            market_outlook: self.extract_market_outlook(&response),
            risk_level: self.extract_risk_level(&response),
            quote: String::new(),
        })
    }

    fn add_coin_analysis(&self, prompt: &mut String, symbol: &str, data: &TechnicalData) {
        prompt.push_str(&format!("\n{} Analysis:\n", symbol));
        
        if let Some(price) = data.current_price {
            prompt.push_str(&format!("• Current Price: ${:.2}\n", price));
        }
        
        if let Some(change) = data.price_change_24h {
            prompt.push_str(&format!("• Price Change 24h: {:.2}%\n", change));
        }
        
        if let Some(rsi) = data.rsi_14 {
            prompt.push_str(&format!("• RSI (14): {:.2}\n", rsi));
        }
        
        if let Some(ma50) = data.ma_50 {
            prompt.push_str(&format!("• 50 MA: ${:.2}\n", ma50));
        }
        
        if let Some(ma200) = data.ma_200 {
            prompt.push_str(&format!("• 200 MA: ${:.2}\n", ma200));
        }
        
        if let Some((macd, signal, hist)) = data.macd {
            prompt.push_str(&format!("• MACD: {:.2}/{:.2}/{:.2}\n", macd, signal, hist));
        }
        
        if let Some((upper, middle, lower)) = data.bollinger_bands {
            prompt.push_str(&format!("• Bollinger Bands: {:.2}/{:.2}/{:.2}\n", upper, middle, lower));
        }
        
        if let Some(volume) = data.volume_24h {
            prompt.push_str(&format!("• Volume 24h: ${:.2}B\n", volume / 1e9));
        }
    }

    fn extract_market_outlook(&self, response: &str) -> String {
        let response_lower = response.to_lowercase();
        
        // Check confidence levels
        if let Some(start) = response_lower.find("bullish scenario") {
            if let Some(end) = response_lower[start..].find("%") {
                let confidence_text = &response_lower[start..start+end];
                if let Some(confidence) = confidence_text.split(":").nth(1) {
                    if let Ok(confidence_num) = confidence.trim().parse::<f64>() {
                        if confidence_num > 50.0 {
                            return "Bullish".to_string();
                        }
                    }
                }
            }
        }
        
        // Check for explicit bearish signals
        if response_lower.contains("strong bearish phase") 
            || response_lower.contains("bearish momentum")
            || (response_lower.contains("bearish scenario") && response_lower.contains("65%")) {
            return "Bearish".to_string();
        }
        
        "Neutral".to_string()
    }

    fn extract_risk_level(&self, _response: &str) -> String {
        // TODO: Implement proper risk level extraction
        String::from("Medium")
    }

    /// Analyzes a specific sector with volume and dominance metrics
    /// # Arguments
    /// * `name` - Sector name (e.g. "AI Sector")
    /// * `volume` - 24h volume in USD
    /// * `dominance` - Market dominance percentage
    /// * `coins` - List of key coins in the sector
    pub fn analyze_sector(
        &self, 
        name: &str, 
        volume: f64, 
        dominance: f64, 
        coins: &[&str]
    ) -> String {
        format!(
            "\n🔍 {} Sector Analysis:\n\
             • 24h Volume: ${:.2}B\n\
             • Market Dominance: {:.2}%\n\
             • Key Assets: {}\n\
             • Volume Trend: {}",
            name,
            volume / 1e9,
            dominance,
            coins.join(", "),
            self.determine_volume_trend(volume)
        )
    }

    /// Determines volume trend based on current volume
    /// # Arguments
    /// * `current_volume` - Current 24h volume in USD
    pub fn determine_volume_trend(&self, current_volume: f64) -> &str {
        // Simple volume trend analysis
        match current_volume {
            v if v > 5_000_000_000.0 => "Very High 🔥",
            v if v > 1_000_000_000.0 => "High 📈",
            v if v > 500_000_000.0 => "Moderate ↔️",
            _ => "Low 📉"
        }
    }

    fn format_technical_overview(&self, data: &MarketTechnicalData, days: u32) -> String {
        format!(
            "BTC: ${:.2}\nETH: ${:.2}\nTotal Market Cap: ${:.2}B\nAnalysis Period: {} days",
            data.btc_data.current_price.unwrap_or(0.0),
            data.eth_data.current_price.unwrap_or(0.0),
            data.global_metrics.total_market_cap / 1e9,
            days
        )
    }

    fn interpret_rsi(&self, rsi: f64) -> &str {
        match rsi {
            r if r >= 70.0 => "Overbought ⚠️",
            r if r <= 30.0 => "Oversold 🔥",
            r if r > 50.0 => "Bullish ↗️",
            r if r < 50.0 => "Bearish ↘️",
            _ => "Neutral ↔️",
        }
    }

    fn analyze_ma_status(&self, price: f64, ma50: f64, ma200: f64) -> String {
        let mut status = Vec::new();
        
        // Check MA50
        if price > ma50 {
            status.push("Above 50MA (Bullish)");
        } else {
            status.push("Below 50MA (Bearish)");
        }

        // Check MA200
        if price > ma200 {
            status.push("Above 200MA (Bullish)");
        } else {
            status.push("Below 200MA (Bearish)");
        }

        // Check for Golden/Death Cross
        if (ma50 - ma200).abs() / ma200 < 0.01 {
            if ma50 > ma200 {
                status.push("Potential Golden Cross 🔥");
            } else {
                status.push("Potential Death Cross ⚠️");
            }
        }

        status.join(", ")
    }

    fn analyze_bollinger_bands(&self, price: f64, upper: f64, middle: f64, lower: f64) -> String {
        let band_width = (upper - lower) / middle * 100.0;
        
        let position = if price >= upper {
            "Above Upper Band (Overbought) ⚠️"
        } else if price <= lower {
            "Below Lower Band (Oversold) 🔥"
        } else if price > middle {
            "Above Middle Band ↗️"
        } else {
            "Below Middle Band ↘️"
        };

        let volatility = if band_width > 40.0 {
            "High Volatility"
        } else if band_width < 20.0 {
            "Low Volatility"
        } else {
            "Normal Volatility"
        };

        format!("{} | {} | Width: {:.1}%", position, volatility, band_width)
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
    
    async fn think(
        &mut self,
        market_data: &MarketData,
        previous_message: Option<String>
    ) -> Result<String> {
        let client = CoinGeckoClient::new()?;
        let technical_data = client.get_technical_analysis().await?;
        
        let days = 14u32;
        let context = serde_json::to_string_pretty(&technical_data)?;
        
        let prompt = format!(
            "Analyze these market conditions:\n{}",
            self.format_technical_overview(&technical_data, days)
        );

        let response = self.base.generate_response(&prompt, Some(&context)).await?;
        
        self.base.memory.conversations.push(Conversation {
            timestamp: Utc::now(),
            market_data: market_data.clone(),
            technical_data: Some(technical_data),
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