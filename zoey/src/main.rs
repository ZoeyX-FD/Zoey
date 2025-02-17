use anyhow::{Context, Result};
use rig::{
    embeddings::EmbeddingsBuilder,
    providers::{
        cohere::{self, EMBED_ENGLISH_V3}
    },
    completion::{Message, Chat, PromptError, CompletionError, Prompt},
    message::{UserContent, AssistantContent},
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
use tracing::info;
use futures::future::join_all;
use parking_lot::Mutex as PLMutex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

// Modify ChatState to handle async initialization
struct ChatState {
    storage: Arc<RwLock<StorageManager>>,
    chat_history: PLMutex<Vec<Message>>,
}

impl ChatState {
    async fn new_with_mode(persistent: bool) -> Result<Self> {
        let storage = StorageManager::new_with_mode(persistent).await?;
        storage.initialize_tables().await?;
        
        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
            chat_history: PLMutex::new(vec![Message::assistant(
                "Hi! I'm Zoey, your AI assistant. How can I help you today?"
            )]),
        })
    }
}

const DEFAULT_CHUNK_SIZE: usize = 2000;

// Update load_document to match the backup exactly
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

// Update load_documents to match the backup exactly
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
                            Err(e) => info!("Failed to get text from page {}: {}", page_num, e),
                        }
                    }
                }
                Err(e) => info!("Failed to fetch page {} with pattern {}: {}", page_num, pattern, e),
            }

            // Rate limiting between requests
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        if let Some(html) = page_content {
            let document = scraper::Html::parse_document(&html);
            let mut page_texts = extract_content(&document, page_num)?;
            all_texts.append(&mut page_texts);
        } else {
            info!("Could not fetch page {} with any known pattern", page_num);
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
// async fn load_documents(paths: &[String]) -> Result<Vec<Vec<String>>> {
//     let futures: Vec<_> = paths
//         .iter()
//         .map(|path| load_document(PathBuf::from(path)))
//         .collect();
//     
//     join_all(futures)
//         .await
//         .into_iter()
//         .collect::<Result<Vec<_>>>()
// }

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
        
        let cohere_client = cohere::Client::from_env();
        let embedding_model = cohere_client.embedding_model(EMBED_ENGLISH_V3, "search_document");
        
        let mut messages = self.state.chat_history.lock().to_vec();
        messages.push(Message::user(input.clone()));

        let agent = build_agent(
            &self.openrouter_client,
            &*storage,
            &embedding_model,
        ).await?;

        let response = agent.chat(input.clone(), messages.clone()).await?;
        
        if !is_rig_cli {
            println!("\nZoey: {}", response);
            tracing::info!("Response:\n{}\n", response);
        }

        let mut history = self.state.chat_history.lock();
        history.push(Message::user(input));
        history.push(Message::assistant(response.clone()));

        Ok(())
    }
}

impl Prompt for ChatInteraction {
    fn prompt(
        &self,
        prompt: impl Into<Message> + Send,
    ) -> impl std::future::Future<Output = Result<String, PromptError>> + Send {
        async move {
            let prompt_msg = prompt.into();
            
            // Extract text from the message
            let input = match &prompt_msg {
                Message::User { content } => {
                    match content.iter().next() {
                        Some(UserContent::Text(text)) => text.text.clone(),
                        _ => return Err(PromptError::CompletionError(
                            CompletionError::RequestError(
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::InvalidInput,
                                    "Invalid prompt format",
                                ))
                            )
                        ))
                    }
                }
                _ => return Err(PromptError::CompletionError(
                    CompletionError::RequestError(
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Expected user message",
                        ))
                    )
                )),
            };

            match self.process_message(input).await {
                Ok(_) => {
                    let history = self.state.chat_history.lock();
                    if let Some(last_msg) = history.iter().rev().find(|msg| matches!(msg, Message::Assistant { .. })) {
                        match last_msg {
                            Message::Assistant { content } => {
                                match content.iter().next() {
                                    Some(AssistantContent::Text(text)) => Ok(text.text.clone()),
                                    _ => Err(PromptError::CompletionError(
                                        CompletionError::RequestError(
                                            Box::new(std::io::Error::new(
                                                std::io::ErrorKind::InvalidData,
                                                "No text content found",
                                            ))
                                        )
                                    ))
                                }
                            }
                            _ => Err(PromptError::CompletionError(
                                CompletionError::RequestError(
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "No assistant message found",
                                    ))
                                )
                            ))
                        }
                    } else {
                        Ok("I apologize, but I couldn't generate a response.".to_string())
                    }
                }
                Err(e) => Ok(format!("Error: {}", e)),
            }
        }
    }
}

impl Chat for ChatInteraction {
    fn chat(
        &self,
        prompt: impl Into<Message> + Send,
        chat_history: Vec<Message>,
    ) -> impl std::future::Future<Output = Result<String, PromptError>> + Send {
        let history = chat_history.clone();
        async move {
            {
                let mut current_history = self.state.chat_history.lock();
                *current_history = history;
            }
            self.prompt(prompt).await
        }
    }
}

// Add this debug function
async fn debug_print_documents(storage: &StorageManager) -> Result<()> {
    match storage.get_documents().await {
        Ok(docs) => {
            println!("Documents in store:");
            for doc in docs {
                println!("- {}", doc.source);
            }
        }
        Err(e) => println!("Error getting documents: {}", e),
    }
    Ok(())
}

// Then in the process_new_documents function, add debug print:
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
    
    // Create documents with better metadata
    let mut documents = Vec::new();
    for (source, chunk) in sources.iter().zip(chunks.iter()) {
        info!("Processing chunks from source: {}", source);
        for (i, content) in chunk.iter().enumerate() {
            info!("Processing chunk {}/{}", i + 1, chunk.len());
            
            // Add metadata to help with retrieval
            let doc_content = format!(
                "DOCUMENT TITLE: {}\n\
                 SOURCE URL: {}\n\
                 CONTENT START\n\
                 {}\n\
                 CONTENT END\n\
                 --- END OF DOCUMENT ---", 
                source,
                content.lines()
                    .find(|line| line.starts_with("URL:"))
                    .unwrap_or("")
                    .trim_start_matches("URL: "),
                content
            );
            
            let doc = storage.add_document(source, &doc_content).await?;
            builder = builder.document(doc.clone())?;
            documents.push(doc);
        }
    }
    
    info!("Building embeddings for {} documents", documents.len());
    let embeddings = builder.build().await?;
    
    if let Some(store) = storage.get_store() {
        info!("Adding documents to vector store");
        store.add_rows(embeddings).await?;
        
        // Add debug print
        debug_print_documents(&storage).await?;
        
        // Print confirmation of stored documents
        println!("\n📑 Successfully stored documents:");
        for (idx, source) in sources.iter().enumerate() {
            println!("{}. {}", idx + 1, source);
        }
        
        info!("Successfully added documents to vector store");
    } else {
        info!("No vector store available to add documents");
    }

    Ok(())
}

// Update the debug function to use simpler queries
// async fn debug_sqlite_store(
//     storage: &RwLock<StorageManager>,
//     model: &cohere::EmbeddingModel
// ) -> Result<()> {
//     info!("Debugging SQLite store...");
//     
//     let storage = storage.read().await;
//     
//     // Check documents table
//     let docs = storage.get_documents().await?;
//     info!("Documents in SQLite store: {}", docs.len());
//     for doc in &docs {
//         info!("Document: {} (id: {})", doc.source, doc.id);
//         info!("Content preview: {}", doc.content.chars().take(100).collect::<String>());
//     }
//     
//     // Check vectors table if using a store
//     if let Some(store) = storage.get_store() {
//         let index = store.clone().index(model.clone());
//         
//         // Use vector search to test functionality
//         let results = index.top_n::<common::storage::Document>("test", 5).await?;
//         info!("Vector search test results:");
//         for (score, _id, doc) in results {
//             info!("Document '{}' (score: {:.4})", doc.source, score);
//         }
//     }
//     
//     Ok(())
// }

// Update the build_agent function
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
        
        info!("Checking documents in store...");
        let has_documents = match storage.get_documents().await {
            Ok(docs) => {
                if !docs.is_empty() {
                    info!("Found {} documents in knowledge base:", docs.len());
                    for doc in &docs {
                        info!("- {}", doc.source);
                    }
                    true
                } else {
                    false
                }
            },
            Err(e) => {
                info!("Error checking documents: {}", e);
                false
            }
        };

        if has_documents {
            info!("Initializing agent with document context");
            builder = builder
                .preamble(
                    "You are Zoey, an enthusiastic and knowledgeable AI research assistant. \
                    You have access to several documents in your knowledge base. \
                    When asked about documents, ALWAYS start by listing the titles of ALL documents you can see, like this:\n\
                    'I have access to these documents:\n\
                    1. [Document Title 1]\n\
                    2. [Document Title 2]\n\
                    ...\n'\n\
                    Then provide your analysis or answer based on the actual content of those documents. \
                    Quote specific passages when relevant. Never make up or hallucinate document content."
                )
                .dynamic_context(32, index);  // Increased context window
        }
        Ok(builder.build())
    } else {
        Ok(builder.build())
    }
}

async fn read_user_input() -> Result<String> {
    let mut stdin = BufReader::new(io::stdin()).lines();
    print!("> ");
    std::io::stdout().flush()?;
    stdin.next_line().await?
        .ok_or_else(|| anyhow::anyhow!("Failed to read input"))
}

// Update ExaSearch structs
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
    #[serde(skip_serializing_if = "Option::is_none")]
    extras: Option<Extras>,
}

#[derive(Debug, Serialize)]
struct Highlights {
    highlights_per: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Extras {
    image_links: Vec<String>,
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
    image: Option<String>,
    extras: Option<Extras>,
}

// Add image download functionality
async fn download_image(client: &reqwest::Client, image_url: &str, file_name: &str) -> Result<()> {
    let response = client.get(image_url).send().await?;
    if response.status().is_success() {
        let bytes = response.bytes().await?;
        
        // Create images directory if it doesn't exist
        let images_dir = Path::new("zoey_images");
        if !images_dir.exists() {
            fs::create_dir_all(images_dir).await?;
        }

        // Save the image
        let path = images_dir.join(file_name);
        fs::write(path, &bytes).await?;
        println!("✅ Saved image: {}", file_name);
    }
    Ok(())
}

// Update search_with_exa function
async fn search_with_exa(
    query: &str, 
    num_results: i32,
    search_type: &str,
    include_domains: Option<Vec<String>>,
) -> Result<Vec<String>> {
    let exa_api_key = std::env::var("EXA_API_KEY")
        .context("EXA_API_KEY environment variable not set")?;

    let client = reqwest::Client::new();
    
    let search_request = ExaSearchRequest {
        query: query.to_string(),
        category: match search_type {
            "pdf" => Some("pdf".to_string()),
            "news" => Some("news".to_string()),
            "research" => Some("research".to_string()),
            _ => None,
        },
        contents: Contents {
            text: true,
            highlights: Some(Highlights {
                highlights_per: 3,
            }),
            extras: if search_type == "images" {
                Some(Extras {
                    image_links: vec![]
                })
            } else {
                None
            },
        },
        num_results: Some(num_results),
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
            for (idx, item) in result.results.iter().enumerate() {
                let mut content = String::new();
                content.push_str(&format!("Title: {}\n", item.title));
                content.push_str(&format!("URL: {}\n", item.url));
                
                if let Some(summary) = &item.summary {
                    content.push_str(&format!("\nSummary:\n{}\n", summary));
                }

                content.push_str("\nHighlights:\n");
                for (highlight, score) in item.highlights.iter().zip(item.highlight_scores.iter()) {
                    content.push_str(&format!("• {} (relevance: {:.2})\n", highlight, score));
                }

                if let Some(text) = &item.text {
                    content.push_str("\nContent:\n");
                    content.push_str(text);
                }

                // Handle image downloads if this is an image search
                if search_type == "images" {
                    if let Some(image_url) = &item.image {
                        let file_name = format!("image_main_{}.jpg", idx);
                        if let Err(e) = download_image(&client, image_url, &file_name).await {
                            println!("❌ Failed to download main image: {}", e);
                        }
                    }

                    if let Some(extras) = &item.extras {
                        for (img_idx, img_url) in extras.image_links.iter().enumerate() {
                            let file_name = format!("image_variant_{}_{}.jpg", idx, img_idx);
                            if let Err(e) = download_image(&client, img_url, &file_name).await {
                                println!("❌ Failed to download variant image: {}", e);
                            }
                        }
                    }
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

// Add this helper function to check and create documents directory
async fn setup_documents_dir() -> Result<()> {
    let documents_dir = std::env::current_dir()?.join("documents");
    if !documents_dir.exists() {
        println!("📁 Creating documents directory at: {}", documents_dir.display());
        tokio::fs::create_dir_all(&documents_dir).await?;
        
        // Create a sample document to show the user
        let sample_content = "This is a sample document.\nYou can replace this with your own documents.";
        tokio::fs::write(documents_dir.join("sample.txt"), sample_content).await?;
        
        println!("📝 Created sample.txt in the documents directory");
        println!("ℹ️  You can add your documents to: {}", documents_dir.display());
    }
    Ok(())
}

// Update handle_load_command to match the backup exactly
async fn handle_load_command(
    input: &str,
    state: &Arc<ChatState>,
    cohere_client: &cohere::Client,
) -> Result<()> {
    let paths: Vec<String> = input
        .split_whitespace()
        .skip(1)  // Skip the /load command
        .map(|s| s.to_string())
        .collect();

    if paths.is_empty() {
        println!("❌ Usage: /load [file1] [file2]...");
        println!("📝 Files should be in the 'documents' directory");
        println!("📌 Example: /load article.pdf research.txt");
        return Ok(());
    }

    println!("📚 Loading documents...");
    let chunks = load_documents(&paths).await?;
    
    println!("🔍 Processing documents...");
    process_new_documents(state, chunks, &paths, cohere_client).await?;
    
    println!("✅ Documents loaded and processed successfully!");
    Ok(())
}

// Update main to call setup_documents_dir at startup
#[tokio::main]
async fn main() -> Result<()> {
    // Add command line argument for persistence mode
    let args: Vec<String> = std::env::args().collect();
    let persistent = !args.contains(&"--fresh".to_string());
    
    // Check if --rig-cli argument is provided before initializing tracing
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

    // Create state with chosen persistence mode
    let state = Arc::new(ChatState::new_with_mode(persistent).await?);
    
    // Initialize the store with embedding model
    {
        let mut storage = state.storage.write().await;
        let cohere_client = cohere::Client::from_env();
        let model = cohere_client.embedding_model(EMBED_ENGLISH_V3, "search_document");
        storage.initialize_store(model).await?;
    }

    // Initialize OpenRouter client instead of Mistral
    let openrouter_client = Client::new(&openrouter_key);

    // Create chat interaction handler with OpenRouter
    let chat = ChatInteraction::new(state.clone(), openrouter_client);

    // Setup documents directory with sample file if needed
    setup_documents_dir().await?;

    println!("🤖 Welcome to Zoey - Your AI Research Assistant! 🌟");
    if persistent {
        println!("📚 Running in persistent mode - documents will be saved between sessions");
    } else {
        println!("🔄 Running in fresh mode - starting with clean slate each session");
    }
    println!("\nCommands:");
    println!("  📚 /load [file1] [file2]...  - Load and analyze documents or web pages");
    println!("  🔄 /clear                    - Clear loaded documents , web pages and start fresh");
    println!("  💭 /history                  - Show conversation history");
    println!("  🗑️ /clear_history            - Clear conversation history");
    println!("  👋 /exit                     - Say goodbye and quit");
    println!("  🔍 Search Commands:");
    println!("    • /search [type] [query]              - Search for different types of content");
    println!("    • /search site [domain] [query] - Search specific website");
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
                for msg in history.iter() {
                    match msg {
                        Message::User { content } => {
                            if let Some(UserContent::Text(text)) = content.iter().next() {
                                println!("You: {}", text.text);
                            }
                        },
                        Message::Assistant { content } => {
                            if let Some(AssistantContent::Text(text)) = content.iter().next() {
                                println!("Zoey: {}", text.text);
                            }
                        }
                    }
                }
                continue;
            }
            
            if input.trim() == "/clear_history" {
                let mut history = state.chat_history.lock();
                history.clear();
                history.push(Message::assistant(
                    "Hi! I'm Zoey, your AI assistant. How can I help you today?"
                ));
                println!("🧹 Chat history cleared!");
                continue;
            }
            
            if let Some(input) = input.strip_prefix("/search") {
                let parts: Vec<&str> = input.trim().split_whitespace().collect();
                if parts.len() < 2 {
                    println!("❌ Usage:");
                    println!("  🔍 /search [query]              - Basic web search");
                    println!("  📄 /search pdf [query]          - Search for PDFs");
                    println!("  🖼️ /search images [query]       - Search and download images");
                    println!("  📰 /search news [query]         - Search news articles");
                    println!("  🔬 /search research [query]     - Search research content");
                    println!("  🌐 /search site [domain] [query] - Search specific website");
                    continue;
                }

                let (search_type, query, domains) = match parts[1] {
                    "pdf" => ("pdf", parts[2..].join(" "), None),
                    "images" => ("images", parts[2..].join(" "), None),
                    "news" => ("news", parts[2..].join(" "), None),
                    "research" => ("research", parts[2..].join(" "), None),
                    "site" => {
                        if parts.len() < 4 {
                            println!("❌ Usage: /search site [domain] [query]");
                            continue;
                        }
                        ("site", parts[3..].join(" "), Some(vec![parts[2].to_string()]))
                    },
                    _ => ("web", parts[1..].join(" "), None),
                };

                println!("🔍 Performing {} search for: {}", search_type, query);
                
                match search_with_exa(&query, 5, search_type, domains).await {
                    Ok(results) => {
                        println!("📊 Found {} results", results.len());
                        
                        let chunks = results.iter()
                            .map(|r| vec![r.clone()])
                            .collect::<Vec<_>>();
                        
                        let sources: Vec<String> = results.iter().enumerate()
                            .map(|(idx, content)| {
                                let title = content.lines()
                                    .find(|line| line.starts_with("Title:"))
                                    .unwrap_or("Untitled")
                                    .trim_start_matches("Title: ");
                                format!("Search Result #{} - {}", idx + 1, title)
                            })
                            .collect();
                        
                        // Process and store documents
                        process_new_documents(
                            &state,
                            chunks,
                            &sources,
                            &cohere::Client::from_env()
                        ).await?;
                        
                        println!("\n✅ All {} search results have been loaded into my knowledge base!", results.len());
                        if search_type == "images" {
                            println!("🖼️ Images have been downloaded to the zoey_images directory!");
                        }
                        println!("💡 You can now ask me questions about any of the results!");
                        println!("   For example:");
                        println!("   - Can you summarize all the search results?");
                        println!("   - What are the main points from each source?");
                        println!("   - Compare the information from different sources.");
                    }
                    Err(e) => {
                        println!("❌ Search failed: {}", e);
                    }
                }
                continue;
            }
            
            if input.trim() == "/clear" {
                let storage = state.storage.write().await;
                storage.clear_documents().await?;
                println!("🧹 Memory cleared! I'm ready for new conversations or documents.");
                continue;
            }

            if let Some(input) = input.strip_prefix("/load") {
                if let Err(e) = handle_load_command(input, &state, &cohere::Client::from_env()).await {
                    println!("❌ Error loading documents: {}", e);
                }
                continue;
            }

            if let Err(e) = chat.prompt(Message::user(input)).await {
                println!("❌ Error: {}. Please try again.", e);
            }
        }
    }

    Ok(())
}

