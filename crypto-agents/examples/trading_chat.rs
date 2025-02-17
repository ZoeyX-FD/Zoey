use anyhow::Result;
use crypto_agents::{
    agents::{BaseAgent, ModelProvider},
    api::coingecko::CoinGeckoClient,
};
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tokio::time::sleep;
use std::time::Duration;
use std::io::Write;
use colored::*;
use common::exa::{ExaClient, ExaSearchParams, Contents, Highlights, Summary};

// Reuse the market data structures from trading_research.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoinData {
    symbol: String,
    price: f64,
    volume_24h: f64,
    price_change_24h: f64,
    technical_indicators: TechnicalIndicators,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MarketContext {
    timestamp: DateTime<Utc>,
    coins: Vec<CoinData>,
    total_market_cap: f64,
    news_events: Vec<NewsEvent>,
    correlations: Vec<Correlation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TechnicalIndicators {
    rsi_14: f64,
    ma_50: f64,
    ma_200: f64,
    macd: (f64, f64, f64),
    bollinger_bands: (f64, f64, f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NewsEvent {
    timestamp: DateTime<Utc>,
    title: String,
    source: String,
    sentiment_score: f64,
    relevance_score: f64,
    category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Correlation {
    symbol: String,
    correlation_7d: f64,
    correlation_30d: f64,
    sector: String,
}

// Chat message structure
#[derive(Debug, Clone)]
struct ChatMessage {
    from: String,
    content: String,
}

// Agent trait for common functionality
#[async_trait::async_trait]
trait ChatAgent {
    fn name(&self) -> &str;
    fn role(&self) -> &str;
    fn emoji(&self) -> &str;
    fn model_info(&self) -> (String, String); // Returns (provider, model)
    async fn respond(&self, context: &MarketContext, message: &str, history: &[ChatMessage]) -> Result<String>;
}

// Trend Analysis Agent
struct TrendAgent {
    base: BaseAgent,
    model: String,
    provider: ModelProvider,
}

#[async_trait::async_trait]
impl ChatAgent for TrendAgent {
    fn name(&self) -> &str {
        "Roger"
    }

    fn role(&self) -> &str {
        "Technical Analyst"
    }

    fn emoji(&self) -> &str {
        "📈"
    }

    fn model_info(&self) -> (String, String) {
        (self.provider.to_string(), self.model.clone())
    }

    async fn respond(&self, context: &MarketContext, message: &str, history: &[ChatMessage]) -> Result<String> {
        let recent_messages = history.iter()
            .rev()
            .take(10)
            .map(|msg| format!("{}: {}", msg.from, msg.content))
            .collect::<Vec<_>>()
            .join("\n");

        // Build market data string for all coins
        let mut market_data = String::new();
        for coin in &context.coins {
            market_data.push_str(&format!(
                "\n{} Market Data:\n\
                Price: ${:.2}\n\
                24h Change: {:.2}%\n\
                RSI: {:.2}\n\
                MACD: {:.2}\n\
                MA50/MA200: ${:.2}/${:.2}\n\
                Volume: ${:.2}B\n",
                coin.symbol,
                coin.price,
                coin.price_change_24h,
                coin.technical_indicators.rsi_14,
                coin.technical_indicators.macd.0,
                coin.technical_indicators.ma_50,
                coin.technical_indicators.ma_200,
                coin.volume_24h / 1_000_000_000.0
            ));
        }

        let prompt = format!(
            "As {}, you are having a friendly chat. The user says: '{}'\n\n\
            Recent conversation context:\n{}\n\n\
            Current market data:\n{}\n\
            Total Market Cap: ${:.2}B\n\n\
            Be friendly and conversational. You can reference the previous conversation and what other agents have said. \
            Show that you're following the whole discussion, not just your part. \
            If others have made relevant points, acknowledge them while adding your technical perspective. \
            Keep your expertise but be more human-like in your interactions.\n\
            IMPORTANT: Always use the exact prices and data provided above. Do not make up or modify any market data.",
            self.name(),
            message,
            recent_messages,
            market_data,
            context.total_market_cap / 1_000_000_000.0
        );

        self.base.generate_response(&prompt, None).await
    }
}

// News Analysis Agent
struct NewsAgent {
    base: BaseAgent,
    model: String,
    provider: ModelProvider,
}

#[async_trait::async_trait]
impl ChatAgent for NewsAgent {
    fn name(&self) -> &str {
        "Laura"
    }

    fn role(&self) -> &str {
        "News Analyst"
    }

    fn emoji(&self) -> &str {
        "📰"
    }

    fn model_info(&self) -> (String, String) {
        (self.provider.to_string(), self.model.clone())
    }

    async fn respond(&self, context: &MarketContext, message: &str, history: &[ChatMessage]) -> Result<String> {
        let recent_messages = history.iter()
            .rev()
            .take(10)
            .map(|msg| format!("{}: {}", msg.from, msg.content))
            .collect::<Vec<_>>()
            .join("\n");

        // Build market data string for each coin
        let mut market_data = String::new();
        for coin in &context.coins {
            market_data.push_str(&format!(
                "{}:\n\
                - Price: ${:.2}\n\
                - 24h Change: {:.2}%\n\
                - RSI: {:.2}\n\
                - MACD: {:.2}\n\
                - MA50/MA200: ${:.2}/${:.2}\n\
                - Volume 24h: ${:.2}B\n\n",
                coin.symbol,
                coin.price,
                coin.price_change_24h,
                coin.technical_indicators.rsi_14,
                coin.technical_indicators.macd.0,
                coin.technical_indicators.ma_50,
                coin.technical_indicators.ma_200,
                coin.volume_24h / 1_000_000_000.0
            ));
        }

        // Add news events if available
        let mut news_summary = String::new();
        if !context.news_events.is_empty() {
            news_summary.push_str("\nRecent Market News:\n");
            for event in context.news_events.iter().take(3) {
                news_summary.push_str(&format!(
                    "- {} (Source: {}, Sentiment: {:.2})\n",
                    event.title,
                    event.source,
                    event.sentiment_score
                ));
            }
        }

        let prompt = format!(
            "As {}, you are having a friendly chat. The user says: '{}'\n\n\
            Recent conversation context:\n{}\n\n\
            Current Market Data:\n{}\
            Total Market Cap: ${:.2}B\n\
            {}\n\
            Be friendly and conversational while providing expert analysis. \
            Reference specific market data points in your response. \
            Show that you're following the whole discussion. \
            Keep your expertise but be natural in your interactions.",
            self.name(),
            message,
            recent_messages,
            market_data,
            context.total_market_cap / 1_000_000_000.0,
            news_summary
        );

        self.base.generate_response(&prompt, None).await
    }
}

// Strategy Agent
struct StrategyAgent {
    base: BaseAgent,
    model: String,
    provider: ModelProvider,
}

#[async_trait::async_trait]
impl ChatAgent for StrategyAgent {
    fn name(&self) -> &str {
        "Lisa"
    }

    fn role(&self) -> &str {
        "Strategy Advisor"
    }

    fn emoji(&self) -> &str {
        "💡"
    }

    fn model_info(&self) -> (String, String) {
        (self.provider.to_string(), self.model.clone())
    }

    async fn respond(&self, context: &MarketContext, message: &str, history: &[ChatMessage]) -> Result<String> {
        let recent_messages = history.iter()
            .rev()
            .take(10)
            .map(|msg| format!("{}: {}", msg.from, msg.content))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "As {}, you are having a friendly chat. The user says: '{}'\n\n\
            Recent chat context:\n{}\n\n\
            Market state for {}:\n\
            Price: ${:.2}\n\
            24h Change: {:.2}%\n\
            Market Cap: ${:.2}B\n\n\
            Be friendly and conversational first, then naturally weave in your strategy advice. \
            You can make small talk, joke lightly, and show personality while still being professional. \
            If asked about how you are, respond naturally before moving to market discussion. \
            Keep your expertise but be more human-like in your interactions.",
            self.name(),
            message,
            recent_messages,
            context.coins[0].symbol,
            context.coins[0].price,
            context.coins[0].price_change_24h,
            context.total_market_cap / 1e9
        );

        self.base.generate_response(&prompt, None).await
    }
}

impl TrendAgent {
    async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        let preamble = "You are Roger, an expert Technical Analyst AI. Your personality is data-driven \
        and precise, but also engaging and helpful. You specialize in trend analysis, chart patterns, \
        and technical indicators. Respond conversationally as if in a group chat.";

        Ok(Self {
            base: BaseAgent::new(
                "Roger (Technical Analyst)".to_string(),
                model.clone(),
                preamble.to_string(),
                provider.clone()
            ).await?,
            model,
            provider,
        })
    }
}

impl NewsAgent {
    async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        let preamble = "You are Laura, an expert News Analyst AI. Your personality is insightful \
        and well-informed about market events. You specialize in analyzing news impact and market \
        sentiment. Respond conversationally as if in a group chat.";

        Ok(Self {
            base: BaseAgent::new(
                "Laura (News Analyst)".to_string(),
                model.clone(),
                preamble.to_string(),
                provider.clone()
            ).await?,
            model,
            provider,
        })
    }
}

impl StrategyAgent {
    async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        let preamble = "You are Lisa, an expert Strategy Advisor AI. Your personality is thoughtful \
        and focused on risk management. You synthesize different market perspectives into actionable \
        trading strategies. Respond conversationally as if in a group chat.";

        Ok(Self {
            base: BaseAgent::new(
                "Lisa (Strategy Advisor)".to_string(),
                model.clone(),
                preamble.to_string(),
                provider.clone()
            ).await?,
            model,
            provider,
        })
    }
}

async fn trigger_discussion(
    agents: &[Box<dyn ChatAgent>],
    context: &MarketContext,
    history: &mut Vec<ChatMessage>,
    topic: &str
) -> Result<()> {
    println!("\n📣 Starting team discussion about: {}", topic);
    
    // Technical Analysis First
    if let Some(tech_agent) = agents.iter().find(|a| a.role() == "Technical Analyst") {
        let response = tech_agent.respond(context, topic, history).await?;
        println!("\n{} {} ({}): {}", 
            tech_agent.emoji(),
            tech_agent.name().blue().bold(),
            tech_agent.role(),
            response
        );
        
        history.push(ChatMessage {
            from: tech_agent.name().to_string(),
            content: response.clone(),
        });
        
        sleep(Duration::from_millis(800)).await;

        // News Analysis responds to Technical Analysis
        if let Some(news_agent) = agents.iter().find(|a| a.role() == "News Analyst") {
            let prompt = format!(
                "Roger just shared this technical analysis: '{}'\n\
                How does this align with your news analysis? What additional insights can you add? \
                Any news that confirms or contradicts these technical indicators? {}", 
                response, topic
            );
            let response = news_agent.respond(context, &prompt, history).await?;
            println!("\n{} {} ({}): {}", 
                news_agent.emoji(),
                news_agent.name().green().bold(),
                news_agent.role(),
                response
            );
            
            history.push(ChatMessage {
                from: news_agent.name().to_string(),
                content: response.clone(),
            });
            
            sleep(Duration::from_millis(800)).await;

            // Technical Analyst responds to News Analysis
            let prompt = format!(
                "Laura raised some interesting points about the news: '{}'\n\
                How do these news events align with your technical indicators? \
                Do you see any technical patterns that support or contradict these developments?",
                response
            );
            let response = tech_agent.respond(context, &prompt, history).await?;
            println!("\n{} {} ({}): {}", 
                tech_agent.emoji(),
                tech_agent.name().blue().bold(),
                tech_agent.role(),
                response
            );

            history.push(ChatMessage {
                from: tech_agent.name().to_string(),
                content: response.clone(),
            });

            sleep(Duration::from_millis(800)).await;
        }
        
        // Strategy Advisor synthesizes both perspectives
        if let Some(strat_agent) = agents.iter().find(|a| a.role() == "Strategy Advisor") {
            let prompt = format!(
                "After hearing both technical and news analysis from Roger and Laura, \
                what's your strategic assessment? How would you synthesize their insights \
                into actionable trading strategies? Consider risk management and potential scenarios. {}", 
                topic
            );
            let response = strat_agent.respond(context, &prompt, history).await?;
            println!("\n{} {} ({}): {}", 
                strat_agent.emoji(),
                strat_agent.name().magenta().bold(),
                strat_agent.role(),
                response
            );
            
            history.push(ChatMessage {
                from: strat_agent.name().to_string(),
                content: response.clone(),
            });

            // Final round of quick reactions from other agents
            if let Some(tech_agent) = agents.iter().find(|a| a.role() == "Technical Analyst") {
                let prompt = "Any final technical levels or indicators to watch based on this strategy?";
                let response = tech_agent.respond(context, prompt, history).await?;
                println!("\n{} {} ({}): {}", 
                    tech_agent.emoji(),
                    tech_agent.name().blue().bold(),
                    tech_agent.role(),
                    response
                );
            }

            if let Some(news_agent) = agents.iter().find(|a| a.role() == "News Analyst") {
                let prompt = "Any key news events or catalysts we should monitor for this strategy?";
                let response = news_agent.respond(context, prompt, history).await?;
                println!("\n{} {} ({}): {}", 
                    news_agent.emoji(),
                    news_agent.name().green().bold(),
                    news_agent.role(),
                    response
                );
            }
        }
    }
    
    Ok(())
}

async fn fetch_coin_data(coingecko: &mut CoinGeckoClient, coin_id: &str) -> Result<Option<CoinData>> {
    println!("🔍 Fetching data for coin {}...", coin_id);
    match coingecko.get_coin_technical_analysis(coin_id, 200).await {
        Ok(tech_data) => {
            let coin_data = CoinData {
                symbol: coin_id.to_uppercase(),
                price: tech_data.current_price.unwrap_or(0.0),
                volume_24h: tech_data.volume_24h.unwrap_or(0.0),
                price_change_24h: tech_data.price_change_24h.unwrap_or(0.0),
                technical_indicators: TechnicalIndicators {
                    rsi_14: tech_data.rsi_14.unwrap_or(0.0),
                    ma_50: tech_data.ma_50.unwrap_or(0.0),
                    ma_200: tech_data.ma_200.unwrap_or(0.0),
                    macd: tech_data.macd.unwrap_or((0.0, 0.0, 0.0)),
                    bollinger_bands: tech_data.bollinger_bands.unwrap_or((0.0, 0.0, 0.0)),
                },
            };
            println!("✅ Successfully fetched data for {}", coin_id);
            Ok(Some(coin_data))
        },
        Err(e) => {
            println!("⚠️ Error fetching data for {}: {}", coin_id, e);
            Ok(None)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    println!("🤖 Interactive Trading Research Chat");
    println!("===================================");

    // Initialize agents
    println!("🔄 Initializing AI team members...");
    
    let agents: Vec<Box<dyn ChatAgent>> = vec![
        Box::new(TrendAgent::new(
            "mistral-large-latest".to_string(),
            ModelProvider::Mistral
        ).await?),
        Box::new(NewsAgent::new(
            "openai/o3-mini".to_string(),
            ModelProvider::OpenRouter
        ).await?),
        Box::new(StrategyAgent::new(
            "deepseek-chat".to_string(),
            ModelProvider::DeepSeek
        ).await?),
    ];

    // Initialize CoinGecko client
    println!("📊 Connecting to market data...");
    let mut coingecko = CoinGeckoClient::new()?;

    // Initialize Exa client for news
    println!("📰 Connecting to news service...");
    let exa_client = if let Ok(api_key) = std::env::var("EXA_API_KEY") {
        Some(ExaClient::new(&api_key))
    } else {
        println!("⚠️ EXA_API_KEY not found, news features will be limited");
        None
    };

    // Fetch real market data
    println!("🔄 Fetching latest market data...");
    let market_data = coingecko.get_market_data().await?;
    let technical_data = coingecko.get_technical_analysis().await?;

    // Fetch recent news
    let mut news_events = Vec::new();
    if let Some(client) = &exa_client {
        println!("🔍 Fetching recent market news...");
        let search_params = ExaSearchParams {
            query: "Current market news , Market updates".to_string(),
            num_results: 10,
            include_domains: vec![
                "cointelegraph.com".to_string(),
                "coindesk.com".to_string(),
                "beincrypto.com".to_string(),
            ],
            start_date: Some(Utc::now() - chrono::Duration::hours(24)),
            end_date: None,
            contents: Some(Contents {
                text: true,
                highlights: Some(Highlights {
                    num_sentences: 3,
                    highlights_per_result: 3,
                }),
                summary: Some(Summary {
                    max_sentences: 5,
                }),
            }),
        };

        match client.search_crypto(search_params).await {
            Ok(results) => {
                for result in results {
                    news_events.push(NewsEvent {
                        timestamp: result.published_date.unwrap_or_else(Utc::now),
                        title: result.title,
                        source: result.url,
                        sentiment_score: result.relevance_score.unwrap_or(0.5),
                        relevance_score: result.relevance_score.unwrap_or(0.5),
                        category: "Market".to_string(),
                    });
                }
                println!("📰 Found {} recent news articles", news_events.len());
            }
            Err(e) => println!("⚠️ Error fetching news: {}", e),
        }
    }

    // Update the market context creation:
    let mut coin_data = Vec::new();
    
    // Fetch BTC data
    let btc_data = CoinData {
        symbol: "BTC".to_string(),
        price: technical_data.btc_data.current_price.unwrap_or(0.0),
        volume_24h: technical_data.btc_data.volume_24h.unwrap_or(0.0),
        price_change_24h: technical_data.btc_data.price_change_24h.unwrap_or(0.0),
        technical_indicators: TechnicalIndicators {
            rsi_14: technical_data.btc_data.rsi_14.unwrap_or(0.0),
            ma_50: technical_data.btc_data.ma_50.unwrap_or(0.0),
            ma_200: technical_data.btc_data.ma_200.unwrap_or(0.0),
            macd: technical_data.btc_data.macd.unwrap_or((0.0, 0.0, 0.0)),
            bollinger_bands: technical_data.btc_data.bollinger_bands.unwrap_or((0.0, 0.0, 0.0)),
        },
    };
    coin_data.push(btc_data);

    // Fetch ETH data
    let eth_data = CoinData {
        symbol: "ETH".to_string(),
        price: technical_data.eth_data.current_price.unwrap_or(0.0),
        volume_24h: technical_data.eth_data.volume_24h.unwrap_or(0.0),
        price_change_24h: technical_data.eth_data.price_change_24h.unwrap_or(0.0),
        technical_indicators: TechnicalIndicators {
            rsi_14: technical_data.eth_data.rsi_14.unwrap_or(0.0),
            ma_50: technical_data.eth_data.ma_50.unwrap_or(0.0),
            ma_200: technical_data.eth_data.ma_200.unwrap_or(0.0),
            macd: technical_data.eth_data.macd.unwrap_or((0.0, 0.0, 0.0)),
            bollinger_bands: technical_data.eth_data.bollinger_bands.unwrap_or((0.0, 0.0, 0.0)),
        },
    };
    coin_data.push(eth_data);

    // Fetch SOL data
    let sol_data = CoinData {
        symbol: "SOL".to_string(),
        price: technical_data.sol_data.current_price.unwrap_or(0.0),
        volume_24h: technical_data.sol_data.volume_24h.unwrap_or(0.0),
        price_change_24h: technical_data.sol_data.price_change_24h.unwrap_or(0.0),
        technical_indicators: TechnicalIndicators {
            rsi_14: technical_data.sol_data.rsi_14.unwrap_or(0.0),
            ma_50: technical_data.sol_data.ma_50.unwrap_or(0.0),
            ma_200: technical_data.sol_data.ma_200.unwrap_or(0.0),
            macd: technical_data.sol_data.macd.unwrap_or((0.0, 0.0, 0.0)),
            bollinger_bands: technical_data.sol_data.bollinger_bands.unwrap_or((0.0, 0.0, 0.0)),
        },
    };
    coin_data.push(sol_data);

    // Create market context with real data
    let mut context = MarketContext {
        timestamp: Utc::now(),
        coins: coin_data,
        total_market_cap: market_data.overview.total_market_cap,
        news_events,
        correlations: vec![
            Correlation {
                symbol: "ETH".to_string(),
                correlation_7d: 0.85,
                correlation_30d: 0.82,
                sector: "Layer 1".to_string(),
            },
        ],
    };

    println!("\n📈 Current Market State:");
    println!("BTC Price: ${:.2}", context.coins[0].price);
    println!("24h Change: {:.2}%", context.coins[0].price_change_24h);
    println!("24h Volume: ${:.2}B", context.coins[0].volume_24h / 1e9);
    println!("Market Cap: ${:.2}B", context.total_market_cap / 1e9);

    // Chat history
    let mut history: Vec<ChatMessage> = Vec::new();

    // Add last_speaking_agent to track who spoke last
    let mut last_speaking_agent: Option<&Box<dyn ChatAgent>> = None;

    // Interactive chat loop
    println!("\n💬 Welcome to the Trading Research Chat!");
    println!("Team members:");
    for agent in &agents {
        let (provider, model) = agent.model_info();
        println!("{} {} ({}) - {}/{} [{}]", 
            agent.emoji(),
            match agent.role() {
                "Technical Analyst" => agent.name().blue().bold(),
                "News Analyst" => agent.name().green().bold(),
                "Strategy Advisor" => agent.name().magenta().bold(),
                _ => agent.name().normal()
            },
            agent.role(),
            provider,
            model.dimmed(),
            match agent.role() {
                "Technical Analyst" => format!("🤖 {}", provider).blue(),
                "News Analyst" => format!("🌐 {}", provider).green(),
                "Strategy Advisor" => format!("🧠 {}", provider).magenta(),
                _ => format!("❓ {}", provider).normal()
            }
        );
    }
    println!("\nCommands:");
    println!("- Type agent name to ask specific agent (e.g., 'Roger, what's the trend?')");
    println!("- Type 'discuss' for team discussion");
    println!("- Type 'discuss [topic]' for specific topic discussion");
    println!("- Type 'fetch [coin_id]' to add a new coin (e.g., 'fetch ondo')");
    println!("- Type 'exit' to quit");

    let stdin = std::io::stdin();
    let mut buffer = String::new();

    loop {
        print!("\n> ");
        std::io::stdout().flush()?;
        buffer.clear();
        stdin.read_line(&mut buffer)?;

        let input = buffer.trim();
        if input.is_empty() {
            continue;
        }

        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            println!("👋 Goodbye!");
            break;
        }

        if input.eq_ignore_ascii_case("discuss") || input.starts_with("discuss ") {
            let topic = if input.len() > 7 {
                &input[7..]
            } else {
                "What's your current market outlook?"
            };
            trigger_discussion(&agents, &context, &mut history, topic).await?;
            last_speaking_agent = None;  // Reset after discussion
            continue;
        }

        if input.to_lowercase().starts_with("fetch ") {
            let coin_id = input[6..].trim().to_lowercase();
            match fetch_coin_data(&mut coingecko, &coin_id).await? {
                Some(new_coin_data) => {
                    // Check if coin already exists
                    if let Some(pos) = context.coins.iter().position(|c| c.symbol.to_lowercase() == coin_id) {
                        context.coins[pos] = new_coin_data;
                    } else {
                        context.coins.push(new_coin_data);
                    }
                    println!("\n✅ Added {} to the market context!", coin_id.to_uppercase());
                    println!("You can now ask the agents about {}", coin_id.to_uppercase());
                    continue;
                }
                None => {
                    println!("\n❌ Could not fetch data for {}. Please check the coin ID and try again.", coin_id);
                    continue;
                }
            }
        }

        // Add user message to history
        history.push(ChatMessage {
            from: "User".to_string(),
            content: input.to_string(),
        });

        // Find specifically addressed agent
        let addressed_agent = agents.iter().find(|agent| {
            input.to_lowercase().starts_with(&format!("{},", agent.name().to_lowercase())) ||
            input.to_lowercase().starts_with(&format!("{} ", agent.name().to_lowercase())) ||
            input.to_lowercase() == agent.name().to_lowercase()
        });

        // If no agent is directly addressed, check if this is a follow-up to the last speaking agent
        let responding_agent = if addressed_agent.is_none() && last_speaking_agent.is_some() {
            last_speaking_agent
        } else {
            addressed_agent
        };

        if let Some(agent) = responding_agent {
            let question = if addressed_agent.is_some() {
                input[agent.name().len()..].trim_start_matches(|c| c == ',' || c == ' ')
            } else {
                input
            };
            
            if question.is_empty() {
                println!("\n❌ Please ask {} a question!", agent.name());
                continue;
            }
            
            match agent.respond(&context, question, &history).await {
                Ok(response) => {
                    println!("\n{} {} ({}): {}", 
                        agent.emoji(),
                        match agent.role() {
                            "Technical Analyst" => agent.name().blue().bold(),
                            "News Analyst" => agent.name().green().bold(),
                            "Strategy Advisor" => agent.name().magenta().bold(),
                            _ => agent.name().normal()
                        },
                        agent.role(),
                        response
                    );
                    
                    history.push(ChatMessage {
                        from: agent.name().to_string(),
                        content: response,
                    });
                    last_speaking_agent = Some(agent);
                }
                Err(e) => println!("Error getting response: {}", e),
            }
        } else {
            // Only respond to unaddressed messages if they contain relevant keywords
            let mut responded = false;
            for agent in &agents {
                if should_agent_respond(agent.as_ref(), input) {
                    match agent.respond(&context, input, &history).await {
                        Ok(response) => {
                            sleep(Duration::from_millis(800)).await;
                            
                            println!("\n{} {} ({}): {}", 
                                agent.emoji(),
                                match agent.role() {
                                    "Technical Analyst" => agent.name().blue().bold(),
                                    "News Analyst" => agent.name().green().bold(),
                                    "Strategy Advisor" => agent.name().magenta().bold(),
                                    _ => agent.name().normal()
                                },
                                agent.role(),
                                response
                            );
                            
                            history.push(ChatMessage {
                                from: agent.name().to_string(),
                                content: response,
                            });
                            last_speaking_agent = Some(agent);
                            responded = true;
                        }
                        Err(e) => println!("Error getting response: {}", e),
                    }
                }
            }
            
            if !responded && !input.starts_with("discuss") {
                println!("\n❓ To talk to an agent, either:");
                println!("   1. Start your message with their name (e.g., 'Laura, what's the latest news?')");
                println!("   2. Ask a question about their expertise (e.g., 'What's the market trend?')");
                println!("   3. Type 'discuss' to start a team discussion");
            }
        }
    }

    Ok(())
}

// Helper function to determine if an agent should respond to a message
fn should_agent_respond(agent: &dyn ChatAgent, message: &str) -> bool {
    let msg_lower = message.to_lowercase();
    
    // Don't respond to messages that look like they're addressing another agent
    for prefix in ["roger", "laura", "lisa"] {
        if msg_lower.starts_with(prefix) {
            return false;
        }
    }
    
    match agent.role() {
        "Technical Analyst" => {
            msg_lower.contains("trend") || 
            msg_lower.contains("price") || 
            msg_lower.contains("chart") ||
            msg_lower.contains("indicator") ||
            msg_lower.contains("support") ||
            msg_lower.contains("resistance")
        }
        "News Analyst" => {
            msg_lower.contains("news") ||
            msg_lower.contains("event") ||
            msg_lower.contains("announcement") ||
            msg_lower.contains("sentiment") ||
            (msg_lower.contains("market") && !msg_lower.contains("technical")) ||
            msg_lower.contains("report")
        }
        "Strategy Advisor" => {
            msg_lower.contains("strategy") ||
            msg_lower.contains("trade") ||
            msg_lower.contains("risk") ||
            msg_lower.contains("position") ||
            msg_lower.contains("entry") ||
            msg_lower.contains("exit") ||
            msg_lower.contains("target")
        }
        _ => false
    }
} 
