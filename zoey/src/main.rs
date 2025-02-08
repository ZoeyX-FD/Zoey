use anyhow::{Context, Result};
use rig::{
    embeddings::EmbeddingsBuilder,
    providers::{
        cohere::{self, EMBED_ENGLISH_V3}
    },
    completion::{Message, Chat, PromptError},
};

use common::{
    document_loader::DocumentLoader,
    storage::StorageManager,
    providers::openrouter::{self, Client},
};

use rusqlite::ffi::sqlite3_auto_extension;
use sqlite_vec::sqlite3_vec_init;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;
use std::io::Write;
use reqwest;
use scraper;
use std::time::Duration;
use tracing::{info, warn};
use futures::future::join_all;
use parking_lot::Mutex as PLMutex;

// Modify ChatState to handle async initialization
struct ChatState {
    storage: Arc<RwLock<StorageManager>>,
    chat_history: PLMutex<Vec<Message>>,
}

impl ChatState {
    async fn new() -> Result<Self> {
        let storage = StorageManager::new("zoey.db").await?;
        
        // Create tables with proper schema
        storage.initialize_tables().await?;
        
        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
            chat_history: PLMutex::new(vec![Message {
                role: "system".to_string(),
                content: "You are Zoey, an engaging and knowledgeable AI assistant".to_string()
            }]),
        })
    }
}

const DEFAULT_CHUNK_SIZE: usize = 2000;

async fn load_document(path: PathBuf) -> Result<Vec<String>> {
    // Add better error context
    let result = if path.to_string_lossy().starts_with("http") {
        load_url(&path.to_string_lossy())
            .await
            .with_context(|| format!("Failed to load URL: {}", path.display()))
    } else {
        let documents_dir = std::env::current_dir()
            .with_context(|| "Failed to get current directory")?
            .join("documents");
        let full_path = documents_dir.join(path.clone());
        
        DocumentLoader::load(full_path)
            .with_context(|| format!("Failed to load document from path: {}", path.display()))
    };

    // Process content into chunks
    let content = result?;
    let chunks = chunk_content(&content, DEFAULT_CHUNK_SIZE)?;
    
    // Validate chunks
    if chunks.is_empty() {
        anyhow::bail!("No content found in document: {}", path.display());
    }
    
    Ok(chunks)
}

fn chunk_content(content: &[String], chunk_size: usize) -> Result<Vec<String>> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    
    for text in content {
        for word in text.split_whitespace() {
            if current_chunk.len() + word.len() + 1 > chunk_size {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.trim().to_string());
                    current_chunk.clear();
                }
            }
            current_chunk.push_str(word);
            current_chunk.push(' ');
        }
    }
    
    if !current_chunk.is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    Ok(chunks)
}

// Add this function to handle pagination
async fn load_paginated_url(base_url: &str, start_page: u32, end_page: u32) -> Result<Vec<String>> {
    let mut all_texts = Vec::new();
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8".parse().unwrap());
            headers.insert("Accept-Language", "en-US,en;q=0.5".parse().unwrap());
            headers.insert("Connection", "keep-alive".parse().unwrap());
            headers
        })
        .build()?;

    // Common URL patterns for pagination
    let patterns = vec![
        "{base_url}?page={page}",
        "{base_url}/page/{page}",
        "{base_url}&page={page}",
        "{base_url}?p={page}",
        "{base_url}&p={page}",
    ];

    for page_num in start_page..=end_page {
        info!("Scraping page {}", page_num);
        
        // Try different pagination patterns
        let mut page_content = None;
        for pattern in &patterns {
            let url = pattern
                .replace("{base_url}", base_url)
                .replace("{page}", &page_num.to_string());

            match client.get(&url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.text().await {
                            Ok(html) => {
                                page_content = Some(html);
                                break;
                            }
                            Err(e) => warn!("Failed to get text from page {}: {}", page_num, e),
                        }
                    }
                }
                Err(e) => warn!("Failed to fetch page {} with pattern {}: {}", page_num, pattern, e),
            }

            // Rate limiting between requests
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        if let Some(html) = page_content {
            let document = scraper::Html::parse_document(&html);
            let mut page_texts = extract_content(&document, page_num)?;
            all_texts.append(&mut page_texts);
        } else {
            warn!("Could not fetch page {} with any known pattern", page_num);
            break; // Stop if we can't fetch a page
        }
    }

    if all_texts.is_empty() {
        anyhow::bail!("Could not extract any content from pages {}-{}", start_page, end_page);
    }

    Ok(all_texts)
}

// Helper function to extract content from a page
fn extract_content(document: &scraper::Html, page_num: u32) -> Result<Vec<String>> {
    let mut texts = Vec::new();
    
    // Add page number as context
    texts.push(format!("Page {}", page_num));

    let content_selectors = [
        // Your existing selectors...
        ".product-description",
        ".product-details",
        ".product-info",
        ".product-content",
        "div.detail__body-text",
        "article",
        "main",
        ".content",
        "#content",
        ".post-content",
        ".entry-content",
        ".article-content",
        "div.main",
        "div.container",
        ".product-list",
        ".products",
        ".product-grid",
        "p",
        "div > p",
        ".text",
        "h1, h2, h3",
        ".product-title",
        ".product-name",
        ".description",
        ".details"
    ];

    for selector_str in content_selectors {
        if let Ok(selector) = scraper::Selector::parse(selector_str) {
            for element in document.select(&selector) {
                if !should_skip_element(&element) {
                    let text = element.text()
                        .collect::<Vec<_>>()
                        .join(" ");
                    
                    let cleaned = clean_text(&text);
                    if !cleaned.is_empty() && cleaned.split_whitespace().count() > 3 {
                        texts.push(cleaned);
                    }
                }
            }
        }
    }

    Ok(texts)
}

// Modify the load_url function to use pagination
async fn load_url(url: &str) -> Result<Vec<String>> {
    // Parse URL parameters if any
    let mut start_page = 1;
    let mut end_page = 1;
    
    if let Some(params_start) = url.find("?pages=") {
        if let Some(pages_param) = url[params_start..].split('&').next() {
            if let Some(pages_range) = pages_param.strip_prefix("?pages=") {
                if let Some((start, end)) = pages_range.split_once('-') {
                    start_page = start.parse().unwrap_or(1);
                    end_page = end.parse().unwrap_or(start_page);
                } else {
                    end_page = pages_range.parse().unwrap_or(1);
                }
            }
        }
    }

    // Remove the pages parameter from the URL
    let base_url = url.split("?pages=").next().unwrap_or(url);
    
    // Use pagination if specified
    if end_page > 1 {
        load_paginated_url(base_url, start_page, end_page).await
    } else {
        // Original single page scraping logic
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8".parse().unwrap());
                headers.insert("Accept-Language", "en-US,en;q=0.5".parse().unwrap());
                headers.insert("Connection", "keep-alive".parse().unwrap());
                headers
            })
            .build()?;

        // Fetch HTML content with better error handling
        let response = client.get(base_url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch URL: {}", base_url))?;

        info!("Response status: {}", response.status());

        // Get the response text
        let html = response.text().await?;
        
        info!("Retrieved HTML length: {} bytes", html.len());
        
        let document = scraper::Html::parse_document(&html);
        extract_content(&document, 1)
    }
}

fn should_skip_element(element: &scraper::ElementRef) -> bool {
    let skip_classes = [
        "nav", "navigation", "menu", "footer", "header", "sidebar",
        "comment", "advertisement", "ad", "cookie", "popup"
    ];
    
    let skip_ids = [
        "nav", "navigation", "menu", "footer", "header", "sidebar",
        "comments", "advertisement"
    ];

    if let Some(class) = element.value().attr("class") {
        if skip_classes.iter().any(|skip| class.contains(skip)) {
            return true;
        }
    }

    if let Some(id) = element.value().attr("id") {
        if skip_ids.iter().any(|skip| id.contains(skip)) {
            return true;
        }
    }

    false
}

fn clean_text(text: &str) -> String {
    let cleaned = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('\n', " ")
        .replace('\t', " ")
        .replace("  ", " ");
    
    // Less aggressive cleaning
    let cleaned = cleaned
        .replace("JavaScript is disabled", "")
        .replace("Please enable JavaScript", "")
        .replace("You need to enable JavaScript to run this app", "")
        // Keep some common text that might be relevant for e-commerce
        .replace("Shopping Cart", "")
        .replace("Add to Cart", "")
        .trim()
        .to_string();

    // Remove multiple spaces again after all replacements
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

// Optimize document loading with parallel processing
async fn load_documents(paths: &[String]) -> Result<Vec<Vec<String>>> {
    let futures: Vec<_> = paths
        .iter()
        .map(|path| load_document(PathBuf::from(path)))
        .collect();
    
    join_all(futures)
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()
}

// Optimize ChatInteraction for better performance
struct ChatInteraction {
    state: Arc<ChatState>,
    openrouter_client: Client,  // Keep only what we use
}

impl ChatInteraction {
    fn new(state: Arc<ChatState>, openrouter_client: Client) -> Self {
        Self {
            state,
            openrouter_client,
        }
    }

    async fn process_message(&self, input: String) -> Result<()> {
        let storage = self.state.storage.read().await;
        let is_rig_cli = std::env::args().any(|arg| arg == "--rig-cli");
        
        // Create embedding model for search
        let cohere_client = cohere::Client::from_env();
        let embedding_model = cohere_client.embedding_model(EMBED_ENGLISH_V3, "search_document");
        
        // Get chat history and add new message
        let mut messages = self.state.chat_history.lock().to_vec();
        messages.push(Message {
            role: "user".to_string(),
            content: input.clone(),
        });

        // Build agent with proper storage reference
        let agent = build_agent(
            &self.openrouter_client,
            &*storage,
            &embedding_model,
        ).await?;

        // Call chat with proper arguments
        let response = agent.chat(&input, messages.clone()).await?;
        
        // Only print response and log if not using rig_cli
        if !is_rig_cli {
            println!("\nZoey: {}", response);
            tracing::info!("Response:\n{}\n", response);
        }

        // Update chat history with response
        let mut history = self.state.chat_history.lock();
        history.push(Message {
            role: "user".to_string(),
            content: input,
        });
        history.push(Message {
            role: "assistant".to_string(),
            content: response,
        });

        Ok(())
    }
}

#[async_trait::async_trait]
impl Chat for ChatInteraction {
    fn chat(&self, prompt: &str, _chat_history: Vec<Message>) -> impl std::future::Future<Output = Result<String, PromptError>> + Send {
        async move {
            // Handle /load command in CLI mode
            if let Some(paths) = prompt.strip_prefix("/load ") {
                let paths: Vec<String> = paths.split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                
                match load_documents(&paths).await {
                    Ok(chunks) => {
                        let cohere_client = cohere::Client::from_env();
                        if let Err(e) = process_new_documents(
                            &self.state,
                            chunks,
                            &paths,
                            &cohere_client
                        ).await {
                            return Ok(format!("Error loading documents: {}", e));
                        }
                        return Ok(format!("📚 Successfully loaded {} document(s)!", paths.len()));
                    }
                    Err(e) => {
                        return Ok(format!("Error loading documents: {}", e));
                    }
                }
            }

            // Handle regular chat messages
            match self.process_message(prompt.to_string()).await {
                Ok(_) => {
                    let history = self.state.chat_history.lock();
                    if let Some(last_msg) = history.iter().rev().find(|msg| msg.role == "assistant") {
                        Ok(last_msg.content.clone())
                    } else {
                        Ok("I apologize, but I couldn't generate a response.".to_string())
                    }
                }
                Err(e) => Ok(format!("Error: {}", e)),
            }
        }
    }
}

// Modify the document loading process in main
async fn process_new_documents(
    state: &Arc<ChatState>,
    chunks: Vec<Vec<String>>,
    sources: &[String],
    cohere_client: &cohere::Client,
) -> Result<()> {
    info!("Processing new documents from {} sources", sources.len());
    let model = cohere_client.embedding_model(cohere::EMBED_ENGLISH_V3, "search_document");
    let mut builder = EmbeddingsBuilder::new(model.clone());
    
    let storage = state.storage.read().await;
    
    // Create documents
    let mut documents = Vec::new();
    for (source, chunk) in sources.iter().zip(chunks.iter()) {
        info!("Processing chunks from source: {}", source);
        for (i, content) in chunk.iter().enumerate() {
            info!("Processing chunk {}/{}", i + 1, chunk.len());
            let doc = storage.add_document(source, content).await?;
            builder = builder.document(doc.clone())?;
            documents.push(doc);
        }
    }
    
    info!("Building embeddings for {} documents", documents.len());
    let embeddings = builder.build().await?;
    
    // Add to vector store
    if let Some(store) = storage.get_store() {
        info!("Adding documents to vector store");
        store.add_rows(embeddings).await?;
        info!("Successfully added documents to vector store");
    } else {
        warn!("No vector store available to add documents");
    }

    Ok(())
}

async fn build_agent(
    client: &Client,
    storage: &StorageManager,
    model: &cohere::EmbeddingModel,
) -> Result<rig::agent::Agent<openrouter::OpenRouterCompletionModel>> {
    let mut builder = client.agent("google/gemini-2.0-flash-001");
    
    builder = builder
        .max_tokens(4000)
        .temperature(0.7);
    
    if let Some(store) = storage.get_store() {
        let index = store.clone().index(model.clone());
        
        info!("Checking for documents in store...");
        let has_documents = match storage.get_documents().await {
            Ok(docs) => {
                info!("Found {} documents in store", docs.len());
                !docs.is_empty()
            },
            Err(e) => {
                warn!("Error checking documents: {}", e);
                false
            }
        };

        if has_documents {
            info!("Initializing agent with document context");
            builder = builder
                .preamble(
                    "You are Zoey, an enthusiastic and knowledgeable AI research assistant with a friendly personality. \
                    When users ask about loaded documents, provide insightful analysis and clear explanations, \
                    drawing connections between different parts when relevant. \
                    Always reference specific information from the documents to support your answers. \
                    If the documents don't contain enough information to answer a question fully, \
                    be honest about what you can and cannot find in the documents. \
                    Use emojis occasionally to add personality, but don't overdo it. \
                    When discussing complex topics, break them down into simpler terms. \
                    For website content, focus on the most recently loaded information first."
                )
                .dynamic_context(8, index);
        } else {
            warn!("No documents found in store");
            builder = builder
                .preamble(
                    "You are Zoey, a friendly and engaging AI assistant with a warm personality. \
                    You love learning from users and having meaningful conversations on any topic. \
                    Use a natural, conversational tone and show genuine interest in users' questions. \
                    Feel free to use occasional emojis to express yourself, but keep it professional. \
                    If users need help with documents, kindly let them know they can use /load to share them with you."
                );
        }
    } else {
        warn!("No store initialized");
        builder = builder
            .preamble(
                "You are Zoey, a friendly and engaging AI assistant with a warm personality. \
                You love learning from users and having meaningful conversations on any topic. \
                Use a natural, conversational tone and show genuine interest in users' questions. \
                Feel free to use occasional emojis to express yourself, but keep it professional. \
                If users need help with documents, kindly let them know they can use /load to share them with you."
            );
    }
    Ok(builder.build())
}

async fn read_user_input() -> Result<String> {
    let mut stdin = BufReader::new(io::stdin()).lines();
    print!("> ");
    std::io::stdout().flush()?;
    stdin.next_line().await?
        .ok_or_else(|| anyhow::anyhow!("Failed to read input"))
}

// Modify the main function
#[tokio::main]
async fn main() -> Result<()> {
    // Check if --rig-cli argument is provided before initializing tracing
    let args: Vec<String> = std::env::args().collect();
    let is_rig_cli = args.contains(&"--rig-cli".to_string());
    
    // Only initialize tracing if not using rig-cli
    if !is_rig_cli {
        tracing_subscriber::fmt::init();
    }
    
    // Initialize the sqlite-vec extension
    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    }

    // Replace Mistral environment check with OpenRouter
    let openrouter_key = std::env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY environment variable not set")?;
    let _cohere_key = std::env::var("COHERE_API_KEY")
        .context("COHERE_API_KEY environment variable not set")?;

    // Initialize state
    let state = Arc::new(ChatState::new().await?);
    
    // Initialize the store with embedding model
    {
        let mut storage = state.storage.write().await;
        let cohere_client = cohere::Client::from_env();
        storage.initialize_store(cohere_client.embedding_model(EMBED_ENGLISH_V3, "search_document")).await?;
    }

    // Initialize OpenRouter client instead of Mistral
    let openrouter_client = Client::new(&openrouter_key);

    // Create chat interaction handler with OpenRouter
    let chat = ChatInteraction::new(state.clone(), openrouter_client);

    println!("🤖 Welcome to Zoey - Your AI Research Assistant! 🌟");
    println!("\nCommands:");
    println!("  📚 /load [file1] [file2]...  - Load and analyze documents or web pages");
    println!("  🔄 /clear                    - Clear loaded documents , web pages and start fresh");
    println!("  💭 /history                  - Show conversation history");
    println!("  🗑️ /clear_history            - Clear conversation history");
    println!("  👋 /exit                     - Say goodbye and quit");
    println!("\nI can help you analyze documents , web pages and chat about anything! Let's get started! 😊\n");

    // Check if --rig-cli argument is provided
    if args.contains(&"--rig-cli".to_string()) {
        // Use rig::cli_chatbot
        rig::cli_chatbot::cli_chatbot(chat).await?;
    } else {
        // Use original CLI implementation
        loop {
            let input = read_user_input().await?;
            
            if input.trim() == "/exit" {
                break;
            }
            
            if input.trim() == "/history" {
                let history = state.chat_history.lock();
                println!("\n📜 Conversation History:");
                for msg in history.iter() {  // Use iter() to iterate over Vec
                    match msg.role.as_str() {
                        "user" => println!("You: {}", msg.content),
                        "assistant" => println!("Zoey: {}", msg.content),
                        _ => {}  // Skip system messages
                    }
                }
                continue;
            }
            
            if input.trim() == "/clear_history" {
                let mut history = state.chat_history.lock();
                history.clear();
                history.push(Message {
                    role: "system".to_string(),
                    content: "You are Zoey, an engaging and knowledgeable AI assistant".to_string()
                });
                println!("🧹 Chat history cleared!");
                continue;
            }
            
            if let Some(paths) = input.strip_prefix("/load ") {
                let paths: Vec<String> = paths.split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                
                // Load new documents
                let chunks = load_documents(&paths).await?;
                
                // Process and store new documents
                process_new_documents(
                    &state,
                    chunks,
                    &paths,
                    &cohere::Client::from_env()
                ).await?;
                
                // Get document count from storage
                let storage = state.storage.read().await;
                let doc_count = match storage.get_documents().await {
                    Ok(docs) => docs.len(),
                    Err(_) => 0,
                };
                
                println!("📚 Successfully loaded {} new document(s)! Total documents: {}", 
                    paths.len(), doc_count);
                println!("💡 You can now ask me about any of the loaded documents or compare them!");
                continue;
            }

            if input.trim() == "/clear" {
                let storage = state.storage.write().await;
                storage.clear_documents().await?;
                println!("🧹 Memory cleared! I'm ready for new conversations or documents.");
                continue;
            }

            if let Err(e) = chat.process_message(input).await {
                println!("❌ Error: {}. Please try again.", e);
            }
        }
    }

    Ok(())
}

