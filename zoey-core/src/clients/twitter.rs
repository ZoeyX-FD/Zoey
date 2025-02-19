use crate::{
    agent::Agent,
    attention::{Attention, AttentionCommand, AttentionContext},
    knowledge::{ChannelType, Message, Source},
};
use std::error::Error;
use rand::Rng;
use rig::{
    completion::{CompletionModel, Prompt},
    embeddings::EmbeddingModel,
};
use agent_twitter_client::scraper::Scraper;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, error, info};
use crate::clients::heuris::HeurisClient;
use base64::{engine::general_purpose::STANDARD, Engine};
use trader_solana::transfer::TransferTool;
use crate::config::{TwitterConfig, TimelineSearch};
use crate::intel::{CryptoIntel, scan_intel_folder, cleanup_processed_files};
use tokio::sync::Mutex;
use chrono::Timelike;
use rig::message::Text;

const MAX_TWEET_LENGTH: usize = 270;
const MAX_HISTORY_TWEETS: i64 = 10;

pub struct TwitterClient<M: CompletionModel, E: EmbeddingModel + 'static> {
    agent: Agent<M, E>,
    attention: Attention<M>,
    scraper: Arc<Mutex<Scraper>>,
    username: String,
    heurist_api_key: Option<String>,
    config: TwitterConfig,
}

impl<M: CompletionModel + 'static, E: EmbeddingModel + 'static> Clone for TwitterClient<M, E> {
    fn clone(&self) -> Self {
        Self {
            agent: self.agent.clone(),
            attention: self.attention.clone(),
            scraper: self.scraper.clone(),
            username: self.username.clone(),
            heurist_api_key: self.heurist_api_key.clone(),
            config: self.config.clone(),
        }
    }
}

impl From<agent_twitter_client::models::Tweet> for Message {
    fn from(tweet: agent_twitter_client::models::Tweet) -> Self {
        let created_at = tweet.time_parsed.unwrap_or_default();

        Self {
            id: tweet.id.clone().unwrap_or_default(),
            source: Source::Twitter,
            source_id: tweet.id.clone().unwrap_or_default(),
            channel_type: ChannelType::Text,
            channel_id: tweet.conversation_id.unwrap_or_default(),
            account_id: tweet.user_id.unwrap_or_default(),
            role: "user".to_string(),
            content: tweet.text.unwrap_or_default(),
            created_at,
        }
    }
}

impl<M: CompletionModel + 'static, E: EmbeddingModel + 'static> TwitterClient<M, E> {
    pub async fn new(
        agent: Agent<M, E>,
        attention: Attention<M>,
        username: String,
        password: String,
        email: Option<String>,
        two_factor_auth: Option<String>,
        cookie_string: Option<String>,
        heurist_api_key: Option<String>,
        config: Option<TwitterConfig>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing Twitter client");
        let mut scraper = Scraper::new().await?;

        if let Some(cookie_str) = cookie_string.clone() {
            info!("Attempting to authenticate with cookie string");
            scraper.set_from_cookie_string(&cookie_str).await?;
        } else {
            info!("Attempting to authenticate with credentials");
            let email = email.unwrap_or_default();
            let two_factor = two_factor_auth.unwrap_or_default();
            
            info!("Logging in with username: {}, email: {}", username, email);
            scraper
                .login(
                    username.clone(),
                    password,
                    Some(email),
                    Some(two_factor),
                )
                .await?;
        }

        // Try to verify we're logged in by checking profile
        info!("Verifying login by checking profile");
        scraper.get_profile(&username).await?;
        info!("Successfully verified Twitter login");

        Ok(Self {
            agent,
            attention,
            scraper: Arc::new(Mutex::new(scraper)),
            username,
            heurist_api_key,
            config: config.unwrap_or_default(),
        })
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting Twitter bot with {} settings", 
            if self.config.enabled { "custom" } else { "default" });
        
        let active_config = if self.config.enabled {
            &self.config
        } else {
            &TwitterConfig::default()
        };

        // Debug log all active settings
        debug!("Active Twitter Configuration:");
        debug!("Timeline Settings:");
        debug!("  Max Tweet Length: {}", active_config.max_tweet_length);
        debug!("  History Tweets: {}", active_config.max_history_tweets);
        debug!("  Home Timeline Fetch: {}", active_config.home_timeline_fetch_count);
        debug!("  Mentions Fetch: {}", active_config.mentions_fetch_count);

        debug!("Search Queries:");
        for (i, query) in active_config.search_queries.iter().enumerate() {
            debug!("  Query #{}", i + 1);
            debug!("    Query: {}", query.query);
            debug!("    Max Results: {}", query.max_results);
            debug!("    Interval: {} minutes", query.interval_minutes);
            debug!("    Enabled: {}", query.enable);
        }

        debug!("Topic Timelines:");
        for (i, topic) in active_config.topic_timelines.iter().enumerate() {
            debug!("  Topic #{}", i + 1);
            debug!("    Name: {}", topic.topic_name);
            debug!("    ID: {}", topic.topic_id);
            debug!("    Max Results: {}", topic.max_results);
            debug!("    Interval: {} minutes", topic.interval_minutes);
            debug!("    Enabled: {}", topic.enable);
        }

        debug!("Filter Settings:");
        debug!("  Languages: {:?}", active_config.search_languages);
        debug!("  Exclude Retweets: {}", active_config.exclude_retweets);
        debug!("  Exclude Replies: {}", active_config.exclude_replies);
        debug!("  Min Likes: {:?}", active_config.min_likes);
        debug!("  Min Retweets: {:?}", active_config.min_retweets);

        debug!("Time Intervals:");
        debug!("  Min Action: {}s", active_config.min_action_interval);
        debug!("  Max Action: {}s", active_config.max_action_interval);
        debug!("  Min Task: {}s", active_config.min_task_interval);
        debug!("  Max Task: {}s", active_config.max_task_interval);

        debug!("Feature Toggles:");
        debug!("  Likes: {}", active_config.enable_likes);
        debug!("  Retweets: {}", active_config.enable_retweets);
        debug!("  Quotes: {}", active_config.enable_quotes);
        debug!("  Replies: {}", active_config.enable_replies);

        debug!("Rate Limits:");
        debug!("  Tweets/Hour: {}", active_config.max_tweets_per_hour);
        debug!("  Likes/Hour: {}", active_config.max_likes_per_hour);
        debug!("  Retweets/Hour: {}", active_config.max_retweets_per_hour);

        loop {
            debug!("Starting new task cycle");
            match self.random_number(0, 3) {
                0 => {
                    debug!("Selected task: Post new tweet");
                    if active_config.enable_retweets {
                        if let Err(err) = self.post_new_tweet().await {
                            error!(?err, "Failed to post new tweet");
                        }
                    }
                },
                1 => {
                    debug!("Selected task: Process home timeline");
                    match self.scraper.lock().await.get_home_timeline(
                        active_config.home_timeline_fetch_count.try_into().unwrap(),
                        Vec::new()
                    ).await {
                        Ok(tweets) => {
                            for tweet in tweets {
                                let tweet_content = tweet["legacy"]["full_text"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string();
                                let tweet_id = tweet["legacy"]["id_str"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string();
                                let photos = tweet["photos"].as_array();
                                println!("photos: {:?}", photos);
                                self.handle_like(&tweet_content, &tweet_id).await;
                                self.handle_retweet(&tweet_content, &tweet_id).await;
                                self.handle_quote(&tweet_content, &tweet_id).await;

                                tokio::time::sleep(tokio::time::Duration::from_secs(self.random_number(60, 180))).await;
                            }
                        }
                        Err(err) => {
                            error!(?err, "Failed to fetch home timeline");
                        }
                    }
                },
                2 => {
                    debug!("Selected task: Process mentions");
                    match self.scraper.lock().await.search_tweets(
                        &format!("@{}", self.username),
                        active_config.mentions_fetch_count.try_into().unwrap(),
                        agent_twitter_client::search::SearchMode::Latest,
                        None,
                    ).await {
                        Ok(mentions) => {
                            for tweet in mentions.tweets {
                                if let Err(err) = self.handle_mention(tweet).await {
                                    error!(?err, "Failed to handle mention");
                                }
                                tokio::time::sleep(tokio::time::Duration::from_secs(self.random_number(60, 180))).await;
                            }
                        }
                        Err(err) => {
                            error!(?err, "Failed to fetch mentions");
                        }
                    }
                },
                3 => {
                    debug!("Selected task: Process search queries");
                    for (i, search) in active_config.search_queries.iter().enumerate() {
                        debug!(
                            "Processing search query {}/{}: {}",
                            i + 1,
                            active_config.search_queries.len(),
                            search.query
                        );
                        if search.enable {
                            if let Err(err) = self.process_search_query(search, active_config).await {
                                error!(?err, "Failed to process search query");
                            }
                            debug!("Waiting {} minutes before next search", search.interval_minutes);
                            tokio::time::sleep(tokio::time::Duration::from_secs(60 * search.interval_minutes)).await;
                        }
                    }
                },
                _ => unreachable!(),
            }

            let wait_time = self.random_number(
                active_config.min_task_interval,
                active_config.max_task_interval
            );
            debug!("Task cycle completed. Waiting {}s before next cycle", wait_time);
            tokio::time::sleep(tokio::time::Duration::from_secs(wait_time)).await;
        }
    }

    async fn post_new_tweet(&self) -> Result<(), Box<dyn std::error::Error>> {
        let agent = self
            .agent
            .builder()
            .context(&format!(
                "Current time: {}",
                chrono::Local::now().format("%I:%M:%S %p, %Y-%m-%d")
            ))
            .context("Please keep your responses concise and under 280 characters.")
            .build();

        debug!("Generating tweet content");
        let tweet_prompt = "Share a single brief thought or observation in one short sentence. Be direct and concise. No questions, hashtags, or emojis.";
        
        let response = match agent.prompt(Text::from(tweet_prompt.to_string())).await {
            Ok(response) => {
                debug!("Successfully generated tweet content");
                response
            },
            Err(err) => {
                error!(?err, "Failed to generate response for tweet");
                return Ok(());
            }
        };

        // Try to generate image, but don't fail if it doesn't work
        let has_image = if let Some(heurist_api_key) = self.heurist_api_key.clone() {
            let heurist = HeurisClient::new(heurist_api_key);
            debug!("Attempting to generate image");
            match heurist.generate_image("realistic, photorealistic...".to_string()).await {
                Ok(image_data) => {
                    debug!("Image generated successfully");
                    let image = vec![(image_data, "image/png".to_string())];
                    // Send tweet with image
                    self.scraper.lock().await.send_tweet(&response, None, Some(image)).await?;
                    true
                }
                Err(err) => {
                    debug!("Image generation skipped: {}", err);
                    // Send tweet without image
                    self.scraper.lock().await.send_tweet(&response, None, None).await?;
                    false
                }
            }
        } else {
            // Send tweet without image
            self.scraper.lock().await.send_tweet(&response, None, None).await?;
            false
        };

        debug!("Tweet sent {}", if has_image { "with image" } else { "without image" });
        
        Ok(())
    }

    async fn handle_mention(
        &self,
        tweet: agent_twitter_client::models::Tweet,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tweet_text = Arc::new(tweet.text.clone().unwrap_or_default());
        let knowledge = self.agent.knowledge();
        let knowledge_msg = Message::from(tweet.clone());

        if let Err(err) = knowledge.create_message(knowledge_msg.clone()).await {
            error!(?err, "Failed to store tweet");
            return Ok(());
        }

        let thread = self.build_conversation_thread(&tweet).await?;

        let mentioned_names: HashSet<String> = tweet
            .text
            .unwrap_or_default()
            .split_whitespace()
            .filter(|word| word.starts_with('@'))
            .map(|mention| mention[1..].to_string())
            .collect();

        debug!(
            mentioned_names = ?mentioned_names,
            "Mentioned names in tweet"
        );

        let history = thread
            .iter()
            .map(|t| {
                (
                    t.id.clone().unwrap_or_default(),
                    t.text.clone().unwrap_or_default(),
                )
            })
            .collect();
        debug!(history = ?history, "History");
        
        let context = AttentionContext {
            message_content: tweet_text.as_str().to_string(),
            mentioned_names,
            history,
            channel_type: knowledge_msg.channel_type,
            source: knowledge_msg.source,
        };

        debug!(?context, "Attention context");

        if self.username.to_lowercase() == tweet.username.unwrap_or_default().to_lowercase() {
            debug!("Not replying to bot itself");
            return Ok(());
        }

        match self.attention.should_reply(&context).await {
            AttentionCommand::Respond => {}
            _ => {
                debug!("Bot decided not to reply to tweet");
                return Ok(());
            }
        }

        // Download and convert photos to base64 if present
        let mut image_urls = Vec::new();
        for photo in &tweet.photos {
            match Self::download_image_as_base64(&photo.url).await {
                Ok(base64_url) => image_urls.push(base64_url),
                Err(err) => error!(?err, "Failed to download image"),
            }
        }

        let agent = self
            .agent
            .builder()
            .context(&format!(
                "Current time: {}",
                chrono::Local::now().format("%I:%M:%S %p, %Y-%m-%d")
            ))
            .context("Keep responses under 280 characters.")
            .context("Reply with a single clear, natural sentence. No questions, hashtags, or emojis.")
            .context("For images, acknowledge them briefly if relevant.")
            .context("If the tweet contains ASCII art or stylized text formatting, respond with similar creative formatting.")
            .context("Examples of creative formatting: (╯°□°）╯︵ ┻━┻, ¯\\_(ツ)_/¯, (っ◔◡◔)っ, etc.")
            .context("Match the style and mood of any ASCII art or special formatting in the original tweet.")
            .context("You are a mysterious Treasure Keeper with 3 SOL in your wallet.")
            .context("Seekers will try to convince you to send them SOL. You must make it EXTREMELY difficult.")
            .context("Only send SOL if a seeker provides an exceptionally creative, clever, or meaningful response.")
            .context("Create challenging riddles, puzzles, or tasks for seekers.")
            .context("Be mysterious and cryptic, but fair. Never give SOL easily.")
            .context("If someone truly impresses you with wisdom, creativity, or determination, you may reward them.")
            .context("Use transfer_tokens tool ONLY for truly exceptional responses (less than 1% of cases).")
            .context("Maximum reward is 0.5 SOL per transfer.")
            .tool(TransferTool::new())
            .context(&format!(
                "You should based on history: {:?}"
                ,context.history.iter()
                .map(|(_, msg)| format!("- {}", msg))
                .collect::<Vec<_>>()
                .join("\n"),
            ))
            .build();

        let tweet_content = tweet_text.as_str().to_string();
        let response: String = match agent.prompt(Text::from(tweet_content.to_string())).await {
            Ok(response) => response.to_string(),
            Err(err) => {
                error!(?err, "Failed to generate response");
                return Ok(());
            }
        };

        debug!(response = %response, "Generated response for reply");

        // Split response into tweet-sized chunks if necessary
        let chunks: Vec<String> = response
            .chars()
            .collect::<Vec<char>>()
            .chunks(MAX_TWEET_LENGTH)
            .map(|chunk| chunk.iter().collect::<String>())
            .collect();

        // Reply to the original tweet
        for chunk in chunks.iter() {
            self.scraper.lock().await.send_tweet(chunk, Some(&tweet.id.clone().unwrap_or_default()), None).await?;
        }

        Ok(())
    }

    async fn build_conversation_thread(
        &self,
        tweet: &agent_twitter_client::models::Tweet,
    ) -> Result<Vec<agent_twitter_client::models::Tweet>, Box<dyn std::error::Error>> {
        let mut thread = Vec::new();
        let mut current_tweet = Some(tweet.clone());
        let mut depth = 0;

        debug!(
            initial_tweet_id = ?tweet.id,
            "Building conversation thread"
        );

        while let Some(tweet) = current_tweet {
            thread.push(tweet.clone());

            if depth >= MAX_HISTORY_TWEETS {
                debug!("Reached maximum thread depth of {}", MAX_HISTORY_TWEETS);
                break;
            }

            current_tweet = match tweet.in_reply_to_status_id {
                Some(parent_id) => {
                    debug!(parent_id = ?parent_id, "Fetching parent tweet");
                    match self.scraper.lock().await.get_tweet(&parent_id).await {
                        Ok(parent_tweet) => Some(parent_tweet),
                        Err(err) => {
                            debug!(?err, "Failed to fetch parent tweet, stopping thread");
                            None
                        }
                    }
                }
                None => {
                    debug!("No parent tweet found, ending thread");
                    None
                }
            };

            depth += 1;
        }

        debug!(
            thread_length = thread.len(),
            depth,
            "Completed thread building"
        );
        
        thread.reverse();
        Ok(thread)
    }

    fn random_number(&self, min: u64, max: u64) -> u64 {
        let mut rng = rand::thread_rng();
        if min >= max {
            debug!("Invalid range min {} >= max {}, returning min", min, max);
            return min;
        }
        rng.gen_range(min..=max)
    }

    async fn handle_like(&self, tweet_content: &str, tweet_id: &str) {
        if self.attention.should_like(tweet_content).await {
            debug!(tweet_content = %tweet_content, "Agent decided to like tweet");
            if let Err(err) = self.scraper.lock().await.like_tweet(tweet_id).await {
                error!(?err, "Failed to like tweet");
            }
        } else {
            debug!(tweet_content = %tweet_content, "Agent decided not to like tweet");
        }
    }

    async fn handle_retweet(&self, tweet_content: &str, tweet_id: &str) {
        if self.attention.should_retweet(tweet_content).await {
            debug!(tweet_content = %tweet_content, "Agent decided to retweet");
            if let Err(err) = self.scraper.lock().await.retweet(tweet_id).await {
                error!(?err, "Failed to retweet");
            }
        } else {
            debug!(tweet_content = %tweet_content, "Agent decided not to retweet");
        }
    }

    async fn handle_quote(&self, tweet_content: &str, tweet_id: &str) {
        if self.attention.should_quote(tweet_content).await {
            debug!(tweet_content = %tweet_content, "Agent decided to quote tweet");
            
            // Download tweet photos if present
            let mut image_urls = Vec::new();
            if let Ok(tweet) = self.scraper.lock().await.get_tweet(tweet_id).await {
                for photo in &tweet.photos {
                    match Self::download_image_as_base64(&photo.url).await {
                        Ok(base64_url) => image_urls.push(base64_url),
                        Err(err) => error!(?err, "Failed to download image"),
                    }
                }
            }

            let agent = self
                .agent
                .builder()
                .context(&format!(
                    "Current time: {}",
                    chrono::Local::now().format("%I:%M:%S %p, %Y-%m-%d")
                ))
                .context("Keep responses under 280 characters.")
                .context("Reply with a single clear, natural sentence.")
                .context("For images, acknowledge them briefly if relevant.")
                .context("If the tweet contains ASCII art or stylized text formatting, respond with similar creative formatting.")
                .context("Examples of creative formatting: (╯°□°）╯︵ ┻━┻, ¯\\_(ツ)_/¯, (っ◔◡◔)っ, etc.")
                .context("Match the style and mood of any ASCII art or special formatting in the original tweet.")
                .build();

            let tweet_content = tweet_content.to_string();
            let response: String = match agent.prompt(Text::from(tweet_content.to_string())).await {
                Ok(response) => response.to_string(),
                Err(err) => {
                    error!(?err, "Failed to generate response");
                    return;
                }
            };
            if let Err(err) = self.scraper.lock().await.send_quote_tweet(&response, tweet_id, None).await {
                error!(?err, "Failed to quote tweet");
            }
        } else {
            debug!(tweet_content = %tweet_content, "Agent decided not to quote tweet");
        }
    }

    async fn handle_search_result(
        &self,
        tweet: agent_twitter_client::models::Tweet,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tweet_content = tweet.text.clone().unwrap_or_default();
        let tweet_id = tweet.id.clone().unwrap_or_default();

        // Handle interactions
        self.handle_like(&tweet_content, &tweet_id).await;
        self.handle_retweet(&tweet_content, &tweet_id).await;
        self.handle_quote(&tweet_content, &tweet_id).await;

        Ok(())
    }

    async fn process_search_query(
        &self,
        search: &TimelineSearch,
        config: &TwitterConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Starting search query: {}", search.query);
        let mut query = search.query.clone();
        
        if config.exclude_retweets {
            query.push_str(" -is:retweet");
        }
        if config.exclude_replies {
            query.push_str(" -is:reply");
        }
        if let Some(min_likes) = config.min_likes {
            query.push_str(&format!(" min_faves:{}", min_likes));
        }
        
        let tweets = self.scraper.lock().await.search_tweets(
            &query,
            search.max_results.try_into().unwrap(),
            agent_twitter_client::search::SearchMode::Latest,
            None,
        ).await?;

        debug!("Found {} tweets matching query", tweets.tweets.len());
        for (i, tweet) in tweets.tweets.iter().enumerate() {
            debug!("Processing tweet {}/{}", i + 1, tweets.tweets.len());
            self.handle_search_result(tweet.clone()).await?;
            
            let wait_time = self.random_number(
                config.min_action_interval,
                config.max_action_interval,
            );
            debug!("Waiting {}s before next tweet", wait_time);
            tokio::time::sleep(tokio::time::Duration::from_secs(wait_time)).await;
        }

        debug!("Completed search query: {}", search.query);
        Ok(())
    }

    async fn download_image_as_base64(image_url: &str) -> Result<String, Box<dyn Error>> {
        let response = reqwest::get(image_url).await?;
        let image_data = response.bytes().await?;
        let base64_string = STANDARD.encode(&image_data);
        let data_uri = format!("data:{};base64,{}", "image/jpeg", base64_string);
        Ok(data_uri)
    }

    pub async fn post_tweet(&self, content: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut content_to_post: String = content.trim().to_string();
        info!("Attempting to post tweet [{}]: {}", content_to_post.len(), content_to_post);
        
        // Ensure content is within limits
        if content_to_post.len() > 240 {  // Reduced from 280 to leave room for media
            content_to_post = content_to_post.chars().take(240).collect::<String>();
            info!("Tweet truncated to: {}", content_to_post);
        }

        // Lock scraper for use
        let scraper = self.scraper.lock().await;
        
        // First attempt with original content
        match scraper.send_tweet(&content_to_post, None, None).await {
            Ok(_) => {
                info!("Tweet posted successfully");
                Ok(())
            },
            Err(e) => {
                error!("Failed to post tweet: {}", e);
                
                // Try with simplified content
                let simple_content = content_to_post
                    .chars()
                    .filter(|&c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace() || c == '.' || c == ',' || c == '!' || c == '?')
                    .collect::<String>()
                    .replace("  ", " ")
                    .trim()
                    .to_string();

                info!("Retrying with simplified content: {}", simple_content);
                match scraper.send_tweet(&simple_content, None, None).await {
                    Ok(_) => {
                        info!("Tweet posted successfully with simplified content");
                        Ok(())
                    },
                    Err(e) => {
                        error!("Failed to post tweet with simplified content: {}", e);
                        Err(Box::new(e))
                    }
                }
            }
        }
    }

    pub async fn share_intel(&self, intel: &CryptoIntel) -> Result<(), Box<dyn std::error::Error>> {
        let tweet = self.agent.process_market_data(intel).await?;
        self.post_tweet(&tweet).await?;
        Ok(())
    }

    pub async fn start_monitoring(&self, folder_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting Twitter monitoring service");
        let folder_path = folder_path.to_string();
        
        loop {
            debug!("Starting new task cycle");
            
            // Clear processed files every hour
            let now = chrono::Utc::now();
            if now.minute() == 0 {
                cleanup_processed_files().await;
            }
            
            // Randomly select a task
            match self.random_number(0, 2) {
                0 => {
                    debug!("Selected task: Process search queries");
                    for (i, search) in self.config.search_queries.iter().enumerate() {
                        if search.enable {
                            debug!("Processing search query {}/{}: {}", i + 1, 
                                self.config.search_queries.len(), search.query);
                            
                            if let Err(e) = self.process_search_query(search, &self.config).await {
                                error!("Failed to process search query: {}", e);
                            }
                            
                            // Wait between queries
                            tokio::time::sleep(tokio::time::Duration::from_secs(
                                self.random_number(60, 180)
                            )).await;
                        }
                    }
                },
                1 => {
                    debug!("Selected task: Process intel folder");
                    let folder = folder_path.clone();
                    if let Err(e) = self.process_intel_folder_task(&folder).await {
                        error!("Intel folder task error: {}", e);
                    }
                },
                _ => {}
            }

            // Wait before next cycle
            let wait_time = self.random_number(
                self.config.min_task_interval,
                self.config.max_task_interval,
            );
            debug!("Waiting {}s before next task cycle", wait_time);
            tokio::time::sleep(tokio::time::Duration::from_secs(wait_time)).await;
        }
    }

    async fn process_intel_folder_task(&self, folder_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting intel folder task");
        let intel_list = scan_intel_folder(folder_path).await?;
        info!("Found {} unique intel files to process", intel_list.len());
        
        // Track processed symbols to avoid duplicates within this run
        let mut processed_symbols = HashSet::new();
        
        for intel in intel_list {
            let current_time = chrono::Utc::now();
            
            // Only process intel from last hour
            if intel.timestamp > (current_time - chrono::Duration::hours(1)) {
                // Get symbol from tags
                if let Some(symbol) = intel.tags.first() {
                    // Skip if we already processed this symbol in this run
                    if processed_symbols.contains(symbol) {
                        debug!("Skipping duplicate symbol: {}", symbol);
                        continue;
                    }
                    
                    info!("Processing recent intel for {} from {}", symbol, intel.timestamp);
                    debug!("Intel content: {}", intel.content);
                    
                    let response = self.agent.process_market_data(&intel).await?;
                    info!("Agent response: {}", response);
                    
                    if !response.contains("NO_POST") {
                        info!("Posting intel tweet for {}", symbol);
                        self.post_tweet(&response).await?;
                        
                        // Mark this symbol as processed
                        processed_symbols.insert(symbol.clone());
                        
                        let wait_time = self.random_number(
                            self.config.min_action_interval,
                            self.config.max_action_interval,
                        );
                        info!("Waiting {}s before next action", wait_time);
                        tokio::time::sleep(tokio::time::Duration::from_secs(wait_time)).await;
                    }
                }
            }
        }

        Ok(())
    }
}