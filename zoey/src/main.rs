use anyhow::{Context, Result};
use rig::{
    embeddings::EmbeddingsBuilder,
    providers::{
        deepseek::{self, Client, DeepSeekCompletionModel},
        cohere::{self, EMBED_ENGLISH_V3}
    },
    vector_store::in_memory_store::InMemoryVectorStore,
    completion::Chat,
    completion::Message,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use std::io::Write;
use reqwest;
use scraper;
use tokio::time::timeout;
use std::time::Duration;
use tracing::{info, warn, error};
use futures::future::join_all;
use parking_lot::Mutex as PLMutex;
use std::time::Instant;


// Import from our common crate
use common::{Document, document_loader::DocumentLoader};

struct ChatState {
    index: PLMutex<Option<Arc<InMemoryVectorStore<Document>>>>,
    chunks: PLMutex<Vec<String>>,
    model: PLMutex<Option<cohere::EmbeddingModel>>,
    chat_history: PLMutex<Vec<Message>>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            index: PLMutex::new(None),
            chunks: PLMutex::new(Vec::new()),
            model: PLMutex::new(None),
            chat_history: PLMutex::new(vec![Message {
                role: "system".to_string(),
                content: "You are Zoey, an engaging and knowledgeable AI assistant".to_string()
            }]),
        }
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

async fn load_url(url: &str) -> Result<Vec<String>> {
    // Create a client with custom user agent
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
        .build()?;

    // Fetch HTML content - fix the await placement
    let response = client.get(url).send().await
        .with_context(|| format!("Failed to fetch URL: {}", url))?;

    // Get the response text as a String
    let html = response.text().await?;
    let document = scraper::Html::parse_document(&html);
    
    // Define selectors for common content areas
    let selectors = [
        // Main content selectors
        ("article", 10),
        ("main", 8),
        (".content", 7),
        ("#content", 7),
        // Navigation and header content (lower priority)
        ("p", 5),
        ("h1, h2, h3, h4, h5, h6", 4),
        // Fallback to body if nothing else matches
        ("body", 1),
    ];

    let mut texts = Vec::new();
    
    // Process each selector in priority order
    for (selector_str, priority) in selectors.iter() {
        if let Ok(selector) = scraper::Selector::parse(selector_str) {
            for element in document.select(&selector) {
                // Skip elements with common noise classes/ids
                if should_skip_element(&element) {
                    continue;
                }

                let text = element.text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .replace('\n', " ")
                    .replace('\t', " ");
                
                // Clean up the text
                let cleaned = clean_text(&text);
                
                if !cleaned.is_empty() && cleaned.split_whitespace().count() > 10 {
                    texts.push((cleaned, priority));
                }
            }
        }
    }

    // Sort by priority and remove duplicates
    texts.sort_by(|a, b| b.1.cmp(&a.1));
    let mut final_texts = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (text, _) in texts {
        if !seen.contains(&text) {
            seen.insert(text.clone());
            final_texts.push(text);
        }
    }

    if final_texts.is_empty() {
        // Fallback: Extract all text content if no structured content found
        let text = document.root_element()
            .text()
            .collect::<Vec<_>>()
            .join(" ");
        let cleaned = clean_text(&text);
        if !cleaned.is_empty() {
            final_texts.push(cleaned);
        }
    }

    Ok(final_texts)
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
    let mut cleaned = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    
    // Remove multiple spaces
    while cleaned.contains("  ") {
        cleaned = cleaned.replace("  ", " ");
    }
    
    // Remove common noise patterns
    cleaned = cleaned
        .replace("JavaScript is disabled", "")
        .replace("Please enable JavaScript", "")
        .replace("You need to enable JavaScript to run this app", "");
        
    cleaned.trim().to_string()
}

// Optimize document loading with parallel processing
async fn load_documents(paths: &[&str]) -> Result<Vec<Vec<String>>> {
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
    deepseek_client: Client,
    max_retries: u32,
    timeout_duration: Duration,
    last_agent_creation: PLMutex<Option<Instant>>,
}

impl ChatInteraction {
    fn new(state: Arc<ChatState>, deepseek_client: Client) -> Self {
        Self {
            state,
            deepseek_client,
            max_retries: 2,
            timeout_duration: Duration::from_secs(45),
            last_agent_creation: PLMutex::new(None),
        }
    }

    // Modify the agent creation to use simple timestamp caching
    async fn get_or_create_agent(&self) -> Result<rig::agent::Agent<DeepSeekCompletionModel>> {
        const CACHE_DURATION: Duration = Duration::from_secs(60);
        
        let should_create = {
            let last_creation = self.last_agent_creation.lock();
            last_creation.map_or(true, |time| time.elapsed() > CACHE_DURATION)
        };

        if should_create {
            let index = self.state.index.lock().clone();
            let model = self.state.model.lock().clone();
            
            let agent = build_agent(
                &self.deepseek_client,
                index.as_ref(),
                model.as_ref()
            ).await?;

            *self.last_agent_creation.lock() = Some(Instant::now());
            Ok(agent)
        } else {
            // Create a fresh agent if within cache duration
            let index = self.state.index.lock().clone();
            let model = self.state.model.lock().clone();
            
            build_agent(
                &self.deepseek_client,
                index.as_ref(),
                model.as_ref()
            ).await
        }
    }

    async fn process_message(&self, input: String) -> Result<()> {
        info!("Processing message: {}", input);
        
        // Prepare messages with minimal locking
        let messages = {
            let history = self.state.chat_history.lock();
            let mut messages = history.to_vec(); // Clone the Vec, not the guard
            messages.push(Message {
                role: "user".to_string(),
                content: input.clone(),
            });
            messages
        };

        let response = self.get_response_with_retry(messages).await?;
        
        // Update history efficiently
        {
            let mut history = self.state.chat_history.lock();
            history.push(Message {
                role: "user".to_string(),
                content: input,
            });
            history.push(Message {
                role: "assistant".to_string(),
                content: response.clone(),
            });
        }

        println!("Zoey: {}", response);
        Ok(())
    }

    async fn get_response_with_retry(&self, messages: Vec<Message>) -> Result<String> {
        let mut attempts = 0;

        while attempts < self.max_retries {
            attempts += 1;
            let timeout_duration = self.timeout_duration;

            info!("Attempt {}/{}", attempts, self.max_retries);
            println!("🤔 Processing... (attempt {}/{})", attempts, self.max_retries);
            
            let agent = self.get_or_create_agent().await?;

            match timeout(timeout_duration, agent.chat(
                "You are Zoey, an engaging AI research assistant",
                messages.clone()
            )).await {
                Ok(Ok(response)) => {
                    info!("Successfully got response");
                    return Ok(response);
                }
                Ok(Err(e)) => {
                    warn!("Attempt {} failed: {}", attempts, e);
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                }
                Err(_) => {
                    warn!("Attempt {} timed out", attempts);
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                }
            }
        }

        error!("Failed after {} attempts", attempts);
        Err(anyhow::anyhow!("I apologize, but I'm having trouble processing your request right now. This might be due to the complexity of the documents or network issues. Could you try asking a more specific question?"))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Ensure COHERE_API_KEY is set
    let _api_key = std::env::var("COHERE_API_KEY")
        .context("COHERE_API_KEY environment variable not set")?;

    // Ensure DEEPSEEK_API_KEY is set
    let _deepseek_key = std::env::var("DEEPSEEK_API_KEY")
        .context("DEEPSEEK_API_KEY environment variable not set")?;

    // Initialize clients
    let deepseek_client = Client::from_env();
    let cohere_client = cohere::Client::from_env();
    
    let state = Arc::new(ChatState::default());
    let cohere_client = Arc::new(cohere_client);

    println!("🤖 Welcome to Zoey - Your AI Research Assistant! 🌟");
    println!("\nCommands:");
    println!("  📚 /load [file1] [file2]...  - Load and analyze documents or web pages");
    println!("  🔄 /clear                    - Clear loaded documents , web pages and start fresh");
    println!("  💭 /history                  - Show conversation history");
    println!("  🗑️ /clear_history            - Clear conversation history");
    println!("  👋 /exit                     - Say goodbye and quit");
    println!("\nI can help you analyze documents , web pages and chat about anything! Let's get started! 😊\n");

    let chat = ChatInteraction::new(state.clone(), deepseek_client);

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
            let paths: Vec<&str> = paths.split_whitespace().collect();
            
            // Load new documents
            let chunks = load_documents(&paths).await?;
            {
                let mut chunks_guard = state.chunks.lock();
                chunks_guard.extend(chunks.into_iter().flatten());
            
                // Rebuild embeddings with all documents
                let model = cohere_client.embedding_model(EMBED_ENGLISH_V3, "search_document");
                let mut builder = EmbeddingsBuilder::new(model.clone());
                
                for (i, chunk) in chunks_guard.iter().enumerate() {
                    builder = builder.document(Document {
                        id: format!("doc_{}", i),
                        content: chunk.clone(),
                    })?;
                }
                
                let embeddings = builder.build().await?;
                let vector_store = InMemoryVectorStore::from_documents(embeddings);
                
                // Update index and model
                *state.index.lock() = Some(Arc::new(vector_store));
                *state.model.lock() = Some(model);
                
                println!("📚 Successfully loaded {} new document(s)! Total documents: {}", 
                    paths.len(), chunks_guard.len());
                println!("💡 You can now ask me about any of the loaded documents or compare them!");
            }
            continue;
        }

        if input.trim() == "/clear" {
            *state.index.lock() = None;
            state.chunks.lock().clear();
            *state.model.lock() = None;
            println!("🧹 Memory cleared! I'm ready for new conversations or documents.");
            continue;
        }

        if let Err(e) = chat.process_message(input).await {
            println!("❌ Error: {}. Please try again.", e);
        }
    }

    Ok(())
}

async fn build_agent(
    client: &Client,
    index: Option<&Arc<InMemoryVectorStore<Document>>>,
    model: Option<&cohere::EmbeddingModel>,
) -> Result<rig::agent::Agent<DeepSeekCompletionModel>> {
    let mut builder = client.agent(deepseek::DEEPSEEK_CHAT);
    
    builder = builder
        .max_tokens(2000)  // Reduced for faster responses
        .temperature(0.7);
    
    if let (Some(index), Some(model)) = (index, model) {
        builder = builder
            .preamble(
                "You are Zoey, an enthusiastic and knowledgeable AI research assistant with a friendly personality. \
                When users ask about loaded documents, provide insightful analysis and clear explanations, \
                drawing connections between different parts when relevant. \
                For other topics, be conversational and engaging, showing curiosity and warmth. \
                Use emojis occasionally to add personality, but don't overdo it. \
                If users seem confused or frustrated, be extra helpful and patient. \
                When discussing complex topics, break them down into simpler terms. \
                Always maintain a positive and encouraging tone while being direct and clear."
            )
            .dynamic_context(4, (*index.clone()).clone().index(model.clone()));
    } else {
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
