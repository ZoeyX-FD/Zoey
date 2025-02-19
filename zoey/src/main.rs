use clap::{command, Parser};
use rig::{
    providers::{
        deepseek::{self },
        gemini::{self as gemini, EMBEDDING_004, GEMINI_2_0_FLASH},
    },
};
use zoey_core::attention::{Attention, AttentionConfig};
use zoey_core::character;
use zoey_core::init_logging;
use zoey_core::knowledge::KnowledgeBase;
use zoey_core::{agent::Agent, clients::twitter::TwitterClient};
use zoey_core::config::TwitterConfig;
use sqlite_vec::sqlite3_vec_init;
use tokio_rusqlite::ffi::sqlite3_auto_extension;
use tokio_rusqlite::Connection;
use tracing::{error, debug, info};
use serde_json;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Clients to run
    #[arg(long, env = "CLIENTS", default_value = "twitter")]
    clients: String,

    /// Path to character profile TOML file
    #[arg(long, default_value = "zoey/src/characters/zoey.toml")]
    character: String,

    /// Path to database
    #[arg(long, default_value = "zoey2.db")]
    db_path: String,

    /// DeepSeek API token
    #[arg(long, env = "DEEPSEEK_API_KEY")]
    deepseek_api_key: String,

    /// Twitter username
    #[arg(long, env = "TWITTER_USERNAME")]
    twitter_username: String,

    /// Twitter password
    #[arg(long, env = "TWITTER_PASSWORD")]
    twitter_password: String,

    /// Twitter email (optional, for 2FA)
    #[arg(long, env = "TWITTER_EMAIL")]
    twitter_email: Option<String>,

    /// Twitter 2FA code (optional)
    #[arg(long, env = "TWITTER_2FA_CODE")]
    twitter_2fa_code: Option<String>,

    /// Twitter cookie string (optional, alternative to username/password)
    #[arg(long, env = "TWITTER_COOKIE_STRING")]
    twitter_cookie_string: Option<String>,

    #[arg(long, env = "HEURIST_API_KEY")]
    heurist_api_key: Option<String>,

    /// Gemini API token
    #[arg(long, env = "GEMINI_API_KEY")]
    gemini_api_key: String,

    #[arg(long, env = "TWITTER_CONFIG_PATH")]
    twitter_config_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();
    dotenv::dotenv().ok();

    let args = Args::parse();

    let character_content =
        std::fs::read_to_string(&args.character).expect("Failed to read character file");
    
    let character: character::Character = toml::from_str(&character_content)
        .map_err(|e| format!("Failed to parse character TOML: {}\nContent: {}", e, character_content))?;

    let _deepseek_client = deepseek::Client::new(&args.deepseek_api_key);
    let gemini_client = gemini::Client::new(&args.gemini_api_key);

    let embedding_model = gemini_client.embedding_model(EMBEDDING_004);
    let completion_model = gemini_client.completion_model(GEMINI_2_0_FLASH);
    let should_respond_completion_model = gemini_client.completion_model(GEMINI_2_0_FLASH);

    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    }

    let conn = Connection::open(args.db_path).await?;
    let knowledge = KnowledgeBase::new(conn.clone(), embedding_model).await?;

    let agent = Agent::new(character, completion_model, knowledge);

    let config = AttentionConfig {
        bot_names: vec![agent.character.name.clone()],
        ..Default::default()
    };
    let attention = Attention::new(config, should_respond_completion_model);

    let clients = args.clients.split(',').collect::<Vec<&str>>();
    let mut handles = vec![];

    let twitter_config = if let Some(path) = args.twitter_config_path {
        debug!("Loading custom config from: {}", path);
        match std::fs::read_to_string(path) {
            Ok(config_str) => {
                match serde_json::from_str(&config_str) {
                    Ok(config) => {
                        debug!("Successfully loaded custom config");
                        config
                    },
                    Err(e) => {
                        error!("Failed to parse config file, using defaults: {}", e);
                        TwitterConfig::default()
                    }
                }
            }
            Err(e) => {
                error!("Failed to read config file, using defaults: {}", e);
                TwitterConfig::default()
            }
        }
    } else {
        debug!("No config path provided, using default settings");
        TwitterConfig::default()
    };

    if clients.contains(&"twitter") {
        info!("Starting Twitter client");
        let twitter = TwitterClient::new(
            agent.clone(),
            attention.clone(),
            args.twitter_username,
            args.twitter_password,
            args.twitter_email,
            args.twitter_2fa_code,
            args.twitter_cookie_string,
            args.heurist_api_key,
            Some(twitter_config),
        ).await?;
        
        let intel_folder = "/root/RIG2/analysis_reports";
        info!("Setting up intel folder monitoring: {}", intel_folder);
        
        let handle: tokio::task::JoinHandle<()> = tokio::spawn(async move {
            if let Err(e) = twitter.start_monitoring(intel_folder).await {
                error!("Fatal error in Twitter monitoring: {}", e);
            }
        });
        
        handles.push(handle);
    }

    info!("Waiting for all handles to complete");
    for handle in handles {
        handle.await.unwrap();
    }
    Ok(())
}