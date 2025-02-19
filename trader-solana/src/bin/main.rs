use anyhow::Result;
use clap::{command, Parser};
use rig::{
    providers::deepseek::{self, Client as DeepseekClient},
    completion::{CompletionModel, Prompt},
};
use rina_solana::{
    solana::{swap::JupiterSwap, transfer::SolanaTransfer},
    gmgn::client::GMGNClient,
    tools::{swap::SwapTool, transfer::TransferTool},
};
use tracing::{info, error, debug};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// DeepSeek API token
    #[arg(long, env = "DEEPSEEK_API_KEY")]
    deepseek_api_key: String,

    /// Solana RPC URL
    #[arg(long, env = "SOLANA_RPC_URL")]
    solana_rpc_url: String,

    /// Solana wallet private key
    #[arg(long, env = "SOLANA_PRIVATE_KEY")]
    solana_private_key: String,
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
        .preamble(r#"You are an expert crypto trading assistant. 
            Your role is to analyze market data and provide clear trading recommendations.
            Consider:
            - Technical analysis
            - Market sentiment
            - Risk management
            - Current token metrics
            
            Format responses as:
            ANALYSIS: [brief market analysis]
            ACTION: [SWAP/TRANSFER/HOLD]
            DETAILS: [specific parameters for the action]
            RISK: [risk assessment]"#)
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
                println!("- exit : Quit the program");
            },

            input if input.starts_with("analyze ") => {
                let token = input.replace("analyze ", "");
                debug!("Analyzing token: {}", token);

                // Get token info from GMGN
                match gmgn.get_token_info(&token).await {
                    Ok(info) => {
                        // Get market analysis from AI
                        let prompt = format!(
                            "Analyze this token:\nPrice: {}\nLiquidity: {}\nHolders: {}\nWhat action should be taken?",
                            info.price, info.liquidity, info.holder_count
                        );
                        
                        match trading_agent.prompt(&prompt).await {
                            Ok(analysis) => println!("{}", analysis),
                            Err(e) => error!("AI analysis error: {}", e)
                        }
                    },
                    Err(e) => error!("Failed to get token info: {}", e)
                }
            },

            input if input.starts_with("swap ") => {
                let parts: Vec<&str> = input.split_whitespace().collect();
                if parts.len() == 4 {
                    let from = parts[1];
                    let to = parts[2];
                    let amount = parts[3];
                    
                    debug!("Initiating swap: {} {} -> {}", amount, from, to);
                    match swap_tool.swap_tokens(from, to, amount.parse()?).await {
                        Ok(tx) => println!("Swap successful! Tx: {}", tx),
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
                    match transfer_tool.transfer_sol(to, amount.parse()?).await {
                        Ok(tx) => println!("Transfer successful! Tx: {}", tx),
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
                                holder.address, holder.amount_cur, holder.amount_percentage);
                        }
                    },
                    Err(e) => error!("Failed to get holders: {}", e)
                }
            },

            _ => println!("Unknown command. Type 'help' for available commands.")
        }
    }

    Ok(())
} 