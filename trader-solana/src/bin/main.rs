use anyhow::Result;
use clap::{command, Parser};
use rig::{
    providers::deepseek::{self, Client as DeepseekClient},
    completion::{Prompt, Message},
};
use trader_solana::{
    gmgn::client::GMGNClient,
    gmgn::types::{TokenInfo, HolderInfo, TokenPriceInfo},
    tools::{swap::SwapTool, transfer::TransferTool},
};
use tracing::{info, error, debug};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// DeepSeek API token
    #[clap(long, env = "DEEPSEEK_API_KEY")]
    deepseek_api_key: String,

    /// Solana RPC URL
    #[clap(long, env = "SOLANA_RPC_URL")]
    solana_rpc_url: String,

    /// Solana wallet private key
    #[clap(long, env = "SOLANA_PRIVATE_KEY")]
    solana_private_key: String,
}

fn print_debug_info(info: &TokenInfo, price_info: Option<&TokenPriceInfo>, holders: &[HolderInfo]) {
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

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    // Initialize DeepSeek client
    let deepseek = DeepseekClient::new(&args.deepseek_api_key);
    
    // Create AI agent for trading decisions
    let trading_agent = deepseek
        .agent(deepseek::DEEPSEEK_CHAT)
        .preamble(r#"You are an Quant expert crypto trading assistant for ur creator, Zoey.
            your goal and mission is to make profit ,example make 0.1 sol to 1 sol profit/trade or 2% profit/trade - 100% profit/trade 
            Your role is to analyze market data and provide clear trading recommendations.
            Consider:
            - Technical analysis / u can create a technical analysis based on the metrics provided
            - Market sentiment
            - Risk management
            - Current token metrics
            - Holder distribution
            - Liquidity metrics
            - Supply metrics
            - add your confident level from 10-100%
            
            Format responses as:
            ================================
            ANALYSIS TIME: [current_datetime]
            
            TOKEN OVERVIEW:
            - Name: [token name]
            - Symbol: [token symbol]
            
            METRICS:
            - Current Price: ${:.8}
            - Market Cap: ${:.2}M
            - 24h Volume: ${:.2}M
            - Price Changes:
              • 24h: {:.2}%
              • 1h:  {:.2}%
              • 5m:  {:.2}%
            - Holders: {:,}
            - Liquidity: ${:,.2}
            - Circulating Supply: {:,}
            
            ANALYSIS:
            [detailed market analysis including:]
            - Market sentiment
            - Technical indicators 
            - Risk factors
            
            CONFIDENCE LEVEL: {}%
            
            ACTION: [SWAP/TRANSFER/HOLD/DONT BUY]
            DETAILS: [specific parameters for the action]
            
            RISK ASSESSMENT:
            - Risk Level: [LOW/MEDIUM/HIGH]
            - Key Risks: [bullet points of main risks]
            - Mitigation: [risk mitigation strategies]
            ==============================="#)
        .build();

    // Initialize tools
    let swap_tool = SwapTool::new(&args.solana_rpc_url, &args.solana_private_key)?;
    let transfer_tool = TransferTool::new(&args.solana_rpc_url, &args.solana_private_key)?;
    let gmgn = GMGNClient::new();

    info!("Solana Trading Assistant initialized");
    println!("Welcome to Solana Trading Assistant!");
    println!("Type 'help' for commands or 'exit' to quit");

    // Main interaction loop
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        match input {
            "exit" | "quit" => break,
            
            "help" => {
                println!("Available commands:");
                println!("- analyze <token_address> : Get market analysis");
                println!("- swap <from> <to> <amount> : Swap tokens");
                println!("- transfer <to_address> <amount> : Transfer tokens");
                println!("- holders <token_address> : View top holders");
                println!("- metrics <token_address> : View token metrics");
                println!("- exit : Quit the program");
            },

            input if input.starts_with("analyze ") => {
                let token = input.replace("analyze ", "");
                debug!("Analyzing token: {}", token);

                // Get both token info and holder data
                let token_info_future = gmgn.get_token_info(&token);
                let price_info_future = gmgn.get_token_price_info(&token);
                let holders_future = gmgn.get_top_holders(&token, Some(5), None, None, None);

                // Run both requests concurrently
                match tokio::try_join!(token_info_future, price_info_future, holders_future) {
                    Ok((info, price_info, holders)) => {
                        // Print debug info first
                        if std::env::var("DEBUG").is_ok() {
                            print_debug_info(&info, Some(&price_info), &holders);
                        }

                        let prompt = format!(
                            r#"Analyze this token:
                            ANALYSIS TIME: {}
                            
                            Basic Info:
                            - Symbol: {}
                            - Name: {}
                            - Decimals: {}
                            
                            METRICS:
                            - Current Price: ${}
                            - Market Cap: ${}
                            - 24h Volume: ${}
                            - Price Changes:
                              • 24h: {}%
                              • 1h:  {}%
                              • 5m:  {}%
                            - Holders: {}
                            - Liquidity: {}
                            - Supply: [Circulating/Total supply not provided]
                            
                            Top 5 Holders:
                            {}
                            
                            What action should be taken?"#,
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                            info.symbol.as_deref().unwrap_or("Unknown"),
                            info.name.as_deref().unwrap_or("Unknown"),
                            info.decimals.map_or("Unknown".to_string(), |d| d.to_string()),
                            price_info.price.map_or("Unknown".to_string(), |p| format!("{:.6}", p)),
                            price_info.market_cap.map_or("Unknown".to_string(), |m| {
                                if m >= 1_000_000_000.0 {
                                    format!("{:.2}B", m / 1_000_000_000.0)
                                } else if m >= 1_000_000.0 {
                                    format!("{:.2}M", m / 1_000_000.0)
                                } else {
                                    format!("{:.2}", m)
                                }
                            }),
                            price_info.volume.map_or("Unknown".to_string(), |v| {
                                if v >= 1_000_000_000.0 {
                                    format!("{:.2}B", v / 1_000_000_000.0)
                                } else if v >= 1_000_000.0 {
                                    format!("{:.2}M", v / 1_000_000.0)
                                } else {
                                    format!("{:.2}", v)
                                }
                            }),
                            price_info.price_change_24h.map_or("Unknown".to_string(), |p| format!("{:+.2}", p)),
                            price_info.price_change_1h.map_or("Unknown".to_string(), |p| format!("{:+.2}", p)),
                            price_info.price_change_5m.map_or("Unknown".to_string(), |p| format!("{:+.2}", p)),
                            info.holder_count.map_or("Unknown".to_string(), |h| h.to_string()),
                            info.liquidity.as_deref().unwrap_or("Unknown"),
                            holders.iter()
                                .take(5)
                                .map(|h| format!(
                                    "- Address: {} ({}%)", 
                                    h.address, 
                                    h.amount_percentage.unwrap_or(0.0)
                                ))
                                .collect::<Vec<_>>()
                                .join("\n")
                        );
                        
                        match trading_agent.prompt(Message::from(prompt)).await {
                            Ok(analysis) => println!("{}", analysis),
                            Err(e) => {
                                error!("AI analysis error: {}", e);
                                println!("\nFallback Analysis:");
                                println!("Token: {} ({})", 
                                    info.name.as_deref().unwrap_or("Unknown"),
                                    info.symbol.as_deref().unwrap_or("Unknown")
                                );
                                println!("Basic Metrics:");
                                println!("- Price: ${}", price_info.price.map_or("Unknown".to_string(), |p| format!("{:.6}", p)));
                                println!("- Market Cap: ${}", price_info.market_cap.map_or("Unknown".to_string(), |m| {
                                    if m >= 1_000_000_000.0 {
                                        format!("{:.2}B", m / 1_000_000_000.0)
                                    } else if m >= 1_000_000.0 {
                                        format!("{:.2}M", m / 1_000_000.0)
                                    } else {
                                        format!("{:.2}", m)
                                    }
                                }));
                                println!("- Volume: ${}", price_info.volume.map_or("Unknown".to_string(), |v| {
                                    if v >= 1_000_000_000.0 {
                                        format!("{:.2}B", v / 1_000_000_000.0)
                                    } else if v >= 1_000_000.0 {
                                        format!("{:.2}M", v / 1_000_000.0)
                                    } else {
                                        format!("{:.2}", v)
                                    }
                                }));
                            }
                        }
                    },
                    Err(e) => error!("Failed to get token data: {}", e)
                }
            },

            input if input.starts_with("swap ") => {
                let parts: Vec<&str> = input.split_whitespace().collect();
                if parts.len() == 4 {
                    let from = parts[1];
                    let to = parts[2];
                    let amount = parts[3];
                    
                    debug!("Initiating swap: {} {} -> {}", amount, from, to);
                    match swap_tool.execute_swap(from.to_string(), to.to_string(), amount.parse()?).await {
                        Ok(_) => println!("Swap successful!"),
                        Err(e) => error!("Swap failed: {}", e)
                    }
                } else {
                    println!("Usage: swap <from_token> <to_token> <amount>");
                }
            },

            input if input.starts_with("transfer ") => {
                let parts: Vec<&str> = input.split_whitespace().collect();
                if parts.len() == 3 {
                    let to = parts[1];
                    let amount = parts[2];
                    
                    debug!("Initiating transfer: {} to {}", amount, to);
                    match transfer_tool.execute_transfer(to.to_string(), amount.parse()?).await {
                        Ok(_) => println!("Transfer successful!"),
                        Err(e) => error!("Transfer failed: {}", e)
                    }
                } else {
                    println!("Usage: transfer <to_address> <amount>");
                }
            },

            input if input.starts_with("holders ") => {
                let token = input.replace("holders ", "");
                match gmgn.get_top_holders(&token, Some(10), None, None, None).await {
                    Ok(holders) => {
                        println!("Top 10 holders:");
                        for holder in holders {
                            println!("Address: {} | Amount: {} | %: {}", 
                                holder.address,
                                holder.amount_cur.unwrap_or(0.0),
                                holder.amount_percentage.unwrap_or(0.0)
                            );
                        }
                    },
                    Err(e) => error!("Failed to get holders: {}", e)
                }
            },

            input if input.starts_with("metrics ") => {
                let token = input.replace("metrics ", "");
                debug!("Getting metrics for token: {}", token);

                match tokio::try_join!(gmgn.get_token_info(&token), gmgn.get_token_price_info(&token)) {
                    Ok((info, price_info)) => {
                        println!("================================");
                        println!("ANALYSIS TIME: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
                        println!("Token Metrics for {}", info.name.as_deref().unwrap_or("Unknown"));
                        println!("================================");
                        println!("Symbol: {}", info.symbol.as_deref().unwrap_or("Unknown"));
                        println!("Current Price: ${}", price_info.price.map_or("Unknown".to_string(), |p| format!("{:.8}", p)));
                        println!("Market Cap: ${}", price_info.market_cap.map_or("Unknown".to_string(), |m| {
                            if m >= 1_000_000_000.0 {
                                format!("{:.2}B", m / 1_000_000_000.0)
                            } else if m >= 1_000_000.0 {
                                format!("{:.2}M", m / 1_000_000.0)
                            } else {
                                format!("{:.2}", m)
                            }
                        }));
                        println!("24h Volume: ${}", price_info.volume.map_or("Unknown".to_string(), |v| {
                            if v >= 1_000_000_000.0 {
                                format!("{:.2}B", v / 1_000_000_000.0)
                            } else if v >= 1_000_000.0 {
                                format!("{:.2}M", v / 1_000_000.0)
                            } else {
                                format!("{:.2}", v)
                            }
                        }));
                        println!("Price Changes:");
                        println!("  • 24h: {}%", price_info.price_change_24h.map_or("Unknown".to_string(), |p| format!("{:+.2}", p)));
                        println!("  • 1h:  {}%", price_info.price_change_1h.map_or("Unknown".to_string(), |p| format!("{:+.2}", p)));
                        println!("  • 5m:  {}%", price_info.price_change_5m.map_or("Unknown".to_string(), |p| format!("{:+.2}", p)));
                        println!("Token Info:");
                        println!("  • Decimals: {}", info.decimals.map_or("Unknown".to_string(), |d| d.to_string()));
                        println!("  • Holders: {}", info.holder_count.map_or("Unknown".to_string(), |h| h.to_string()));
                        println!("  • Liquidity: ${}", info.liquidity.as_deref().map_or("Unknown".to_string(), |l| {
                            let value = l.parse::<f64>().unwrap_or(0.0);
                            if value >= 1_000_000_000.0 {
                                format!("{:.2}B", value / 1_000_000_000.0)
                            } else if value >= 1_000_000.0 {
                                format!("{:.2}M", value / 1_000_000.0)
                            } else {
                                format!("{:.2}", value)
                            }
                        }));
                        println!("  • Circulating Supply: {}", info.circulating_supply.as_deref().map_or("Unknown".to_string(), |s| {
                            let value = s.parse::<f64>().unwrap_or(0.0);
                            format!("{:.0}", value)
                        }));
                        println!("  • Total Supply: {}", info.total_supply.as_deref().map_or("Unknown".to_string(), |s| {
                            let value = s.parse::<f64>().unwrap_or(0.0);
                            format!("{:.0}", value)
                        }));
                        println!("================================");
                    },
                    Err(e) => error!("Failed to get token metrics: {}", e)
                }
            },

            _ => println!("Unknown command. Type 'help' for available commands.")
        }
    }

    Ok(())
} 