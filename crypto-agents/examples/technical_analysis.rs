use anyhow::Result;
use crypto_agents::{
    api::coingecko::CoinGeckoClient,
    agents::technical::TechnicalAgent,
};
use dotenv::dotenv;
use std::{env, fs};
use chrono::prelude::*;
use serde::{Serialize, Deserialize};
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct AnalysisReport {
    timestamp: String,
    symbol: String,
    current_price: f64,
    price_change_24h: f64,
    market_cap: f64,
    volume_24h: f64,
    ma_50: Option<f64>,
    ma_200: Option<f64>,
    market_outlook: String,
    risk_level: String,
    analysis: String,
    model: String,
    provider: String,
    quote: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    
    // Get current timestamp
    let now: DateTime<Local> = Local::now();
    let timestamp = now.format("%Y-%m-%d %H:%M:%S %Z").to_string();
    
    // Get coin symbol from command line
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        println!("Usage: cargo run --example technical_analysis -- <COIN_SYMBOL> <PROVIDER> <MODEL>");
        println!("Example: cargo run --example technical_analysis -- sol gemini gemini-1.5-flash");
        println!("Available providers/models:");
        println!("- gemini: gemini-1.5-pro, gemini-1.5-flash");
        println!("- mistral: mistral-large-latest, mistral-small-latest");
        println!("- openai: gpt-4o-mini, gpt-3.5-turbo");
        println!("- openrouter: google/gemini-2.0-flash-001, deepseek/deepseek-r1");
        println!("- deepseek: deepseek-chat, deepseek-reasoner");
        println!("- ollama: deepseek-r1:1.5b-qwen-distill-q8_0");
        return Ok(());
    }
    let symbol = &args[1].to_uppercase();
    let coin_id = symbol.to_lowercase();
    let provider = match args[2].to_lowercase().as_str() {
        "gemini" => crypto_agents::agents::ModelProvider::Gemini,
        "mistral" => crypto_agents::agents::ModelProvider::Mistral,
        "openai" => crypto_agents::agents::ModelProvider::OpenAI,
        "openrouter" => crypto_agents::agents::ModelProvider::OpenRouter,
        "deepseek" => crypto_agents::agents::ModelProvider::DeepSeek,
        "ollama" => crypto_agents::agents::ModelProvider::Ollama,
        _ => {
            println!("⚠️ Invalid provider. Available options: gemini, mistral, openai, openrouter, deepseek, ollama");
            return Ok(());
        }
    };
    let model_name = args[3].clone();
    
    println!("🚀 Starting {} Technical Analysis Example", symbol);

    // Initialize CoinGecko client
    let client = CoinGeckoClient::new()?;
    println!("✅ CoinGecko client initialized");

    // Fetch coin technical data
    println!("🔍 Fetching {} technical data...", symbol);
    let coin_data = match client.get_detailed_coin_data(&coin_id).await {
        Ok(data) => data,
        Err(e) => {
            println!("⚠️ Error fetching data for {}: {}", symbol, e);
            return Ok(());
        }
    };

    // Initialize Technical Agent
    let technical_agent = TechnicalAgent::new(
        model_name.clone(),
        provider.clone()
    ).await?;
    println!("✅ Technical Agent initialized with {:?}:{}", provider, model_name);

    // Perform technical analysis
    println!("📈 Analyzing {}...", symbol);
    let analysis = technical_agent.analyze_coin_data(
        symbol,
        &coin_data
    ).await?;

    // Create analysis report struct
    let report = AnalysisReport {
        timestamp: timestamp.clone(),
        symbol: symbol.clone(),
        current_price: coin_data.current_price,
        price_change_24h: coin_data.price_change_24h.unwrap_or_default(),
        market_cap: coin_data.market_cap,
        volume_24h: coin_data.volume_24h,
        ma_50: coin_data.ma_50,
        ma_200: coin_data.ma_200,
        market_outlook: analysis.market_outlook.clone(),
        risk_level: analysis.risk_level.clone(),
        analysis: analysis.analysis.clone(),
        model: model_name.clone(),
        provider: format!("{:?}", provider),
        quote: analysis.quote.clone(),
    };

    // Save as JSON
    let json_path = format!("analysis_reports/{}_{}.json", 
        symbol.to_lowercase(),
        now.format("%Y%m%d_%H%M%S")
    );
    save_as_json(&report, &json_path)?;
    println!("💾 Report saved as JSON: {}", json_path);

    // Save as CSV
    let csv_path = format!("analysis_reports/{}_{}.csv", 
        symbol.to_lowercase(),
        now.format("%Y%m%d_%H%M%S")
    );
    save_as_csv(&report, &csv_path)?;
    println!("💾 Report saved as CSV: {}", csv_path);

    // Display key technical indicators
    println!("\n🔢 Key Technical Indicators :");
    println!("📅 Analysis Time: {}", timestamp);
    println!("- Current Price: ${:.2}", coin_data.current_price);
    println!("- 24h Change: {:.2}%", coin_data.price_change_24h.unwrap_or(0.0));
    println!("- Market Cap: ${:.2}M", coin_data.market_cap / 1_000_000.0);
    println!("- 24h Volume: ${:.2}M", coin_data.volume_24h / 1_000_000.0);

    // Display results
    println!("\n📊 {} Technical Analysis Report by ZOEY Research Crypto trading:", symbol);
    println!("Generated: {}", timestamp);
    println!("=================================");
    println!("{}", analysis.analysis);
    println!("---------------------------------");
    println!("Market Outlook: {}", analysis.market_outlook);
    println!("Risk Level: {}", analysis.risk_level);
    println!("---------------------------------");
    println!("💭 {}", analysis.quote);
    println!("\n⏰ Report End Time: {}", Local::now().format("%Y-%m-%d %H:%M:%S %Z"));
    
    Ok(())
}

fn save_as_json(report: &AnalysisReport, path: &str) -> Result<()> {
    // Create directory if it doesn't exist
    if let Some(dir) = Path::new(path).parent() {
        fs::create_dir_all(dir)?;
    }
    
    let json = serde_json::to_string_pretty(report)?;
    fs::write(path, json)?;
    Ok(())
}

fn save_as_csv(report: &AnalysisReport, path: &str) -> Result<()> {
    // Create directory if it doesn't exist
    if let Some(dir) = Path::new(path).parent() {
        fs::create_dir_all(dir)?;
    }
    
    let mut wtr = csv::Writer::from_path(path)?;
    
    // Write headers and data
    wtr.serialize(report)?;
    wtr.flush()?;
    Ok(())
} 