use anyhow::{Result, Context};
use dotenv::dotenv;
use std::env;
use serde::{Deserialize, Serialize};
use crypto_agents::{
    agents::{
        Agent, TechnicalAgent, FundamentalAgent, 
        TopicAgent, SentimentAgent, SynopsisAgent,
        ModelProvider
    },
    models::MarketData
};

#[derive(Debug, Serialize)]
struct ExaSearchRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    contents: Contents,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_results: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_domains: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct Contents {
    text: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    highlights: Option<Highlights>,
}

#[derive(Debug, Serialize)]
struct Highlights {
    highlights_per: i32,
}

#[derive(Debug, Deserialize)]
struct ExaResponse {
    results: Vec<ExaResult>,
}

#[derive(Debug, Deserialize)]
struct ExaResult {
    title: String,
    url: String,
    text: Option<String>,
    #[serde(default)]
    highlights: Vec<String>,
    #[serde(default)]
    highlight_scores: Vec<f64>,
    #[serde(default)]
    summary: Option<String>,
}

async fn search_crypto_intel(
    client: &reqwest::Client,
    symbol: &str,
    query_type: &str,
) -> Result<Vec<String>> {
    let exa_api_key = env::var("EXA_API_KEY")
        .context("EXA_API_KEY environment variable not set")?;

    // Construct query based on type
    let query = match query_type {
        "price" => format!("{} price analysis market trends current price prediction", symbol),
        "technical" => format!("{} technical analysis trading patterns indicators", symbol),
        "fundamental" => format!("{} fundamental analysis blockchain metrics development", symbol),
        "sentiment" => format!("{} market sentiment social media community reaction", symbol),
        "news" => format!("{} latest news developments announcements updates", symbol),
        _ => format!("{} {}", symbol, query_type),
    };

    // High-quality crypto news and analysis sites
    let include_domains = Some(vec![
        "cointelegraph.com".to_string(),
        "coindesk.com".to_string(),
        "decrypt.co".to_string(),
        "theblock.co".to_string(),
        "cryptoslate.com".to_string(),
        "messari.io".to_string(),
        "glassnode.com".to_string(),
    ]);

    let search_request = ExaSearchRequest {
        query,
        category: Some("news".to_string()),
        contents: Contents {
            text: true,
            highlights: Some(Highlights {
                highlights_per: 5,
            }),
        },
        num_results: Some(5),
        include_domains,
    };

    let response = client
        .post("https://api.exa.ai/search")
        .header("Authorization", format!("Bearer {}", exa_api_key))
        .header("Content-Type", "application/json")
        .json(&search_request)
        .send()
        .await?;

    let mut results = Vec::new();
    
    match response.status() {
        reqwest::StatusCode::OK => {
            let result: ExaResponse = response.json().await?;
            for item in result.results {
                let mut content = String::new();
                content.push_str(&format!("Title: {}\n", item.title));
                content.push_str(&format!("URL: {}\n", item.url));
                
                if let Some(summary) = item.summary {
                    content.push_str(&format!("\nSummary:\n{}\n", summary));
                }

                content.push_str("\nKey Points:\n");
                for (highlight, score) in item.highlights.iter().zip(item.highlight_scores.iter()) {
                    content.push_str(&format!("• {} (relevance: {:.2})\n", highlight, score));
                }

                if let Some(text) = item.text {
                    content.push_str("\nDetailed Analysis:\n");
                    content.push_str(&text);
                }

                results.push(content);
            }
        }
        status => {
            let error_text = response.text().await?;
            anyhow::bail!("Search failed: {} - {}", status, error_text);
        }
    }

    Ok(results)
}

async fn analyze_with_agents(
    market_data: &MarketData,
    search_results: &[String],
    model: &str,
    provider: ModelProvider,
) -> Result<String> {
    // Initialize agents
    let mut technical = TechnicalAgent::new(model.to_string(), provider.clone()).await?;
    let mut fundamental = FundamentalAgent::new(model.to_string(), provider.clone()).await?;
    let mut sentiment = SentimentAgent::new(model.to_string(), provider.clone()).await?;
    let mut topic = TopicAgent::new(model.to_string(), provider.clone()).await?;
    let mut synopsis = SynopsisAgent::new(model.to_string(), provider).await?;

    // Format search results for context
    let search_context = search_results.join("\n\n");

    // Get analysis from each agent with search context
    let technical_analysis = technical.think(market_data, Some(search_context.clone())).await?;
    let fundamental_analysis = fundamental.think(market_data, Some(search_context.clone())).await?;
    let sentiment_analysis = sentiment.think(market_data, Some(search_context.clone())).await?;
    let topic_analysis = topic.think(market_data, Some(search_context.clone())).await?;

    // Generate final synopsis
    synopsis.generate_synopsis(
        &technical_analysis,
        &fundamental_analysis,
        Some(&sentiment_analysis),
        Some(&topic_analysis),
    ).await
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();

    // Initialize HTTP client
    let client = reqwest::Client::new();

    // Get command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Usage:");
        println!("  Predefined searches:");
        println!("    cargo run -p crypto-agents --example crypto_search [symbol] price");
        println!("    cargo run -p crypto-agents --example crypto_search [symbol] technical");
        println!("    cargo run -p crypto-agents --example crypto_search [symbol] fundamental");
        println!("    cargo run -p crypto-agents --example crypto_search [symbol] sentiment");
        println!("    cargo run -p crypto-agents --example crypto_search [symbol] news");
        println!("  Custom search:");
        println!("    cargo run -p crypto-agents --example crypto_search [symbol] \"your custom query\"");
        return Ok(());
    }

    let symbol = &args[1].to_uppercase();
    let query_type = &args[2];

    println!("🔍 Searching for {} information about {}", query_type, symbol);
    
    // Perform search
    let results = search_crypto_intel(&client, symbol, query_type).await?;
    
    println!("\n📊 Found {} relevant results", results.len());
    
    // Print search results overview
    println!("\n📑 Search Results Overview:");
    for (idx, result) in results.iter().enumerate() {
        let title = result.lines()
            .find(|line| line.starts_with("Title:"))
            .unwrap_or("Untitled")
            .trim_start_matches("Title: ");
        let url = result.lines()
            .find(|line| line.starts_with("URL:"))
            .unwrap_or("No URL")
            .trim_start_matches("URL: ");
        println!("{}. {} - {}", idx + 1, title, url);
    }

    // Create market data for analysis
    let market_data = MarketData {
        symbol: symbol.clone(),
        name: symbol.clone(), // You might want to add proper name resolution
        price: 0.0,          // Add real price data if available
        volume: 0.0,         // Add real volume data if available
        market_cap: 0.0,     // Add real market cap if available
        change_24h: 0.0,     // Add real 24h change if available
    };

    // Analyze with agents
    println!("\n🤖 Analyzing with crypto agents...");
    let analysis = analyze_with_agents(
        &market_data,
        &results,
        "gpt-4",  // You might want to make this configurable
        ModelProvider::OpenRouter,
    ).await?;

    println!("\n📈 Analysis Results:\n{}", analysis);

    Ok(())
} 