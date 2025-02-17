use anyhow::Result;
use common::providers::openrouter::Client;
use rig::completion::{CompletionModel, CompletionRequest, Chat, Message, AssistantContent};


#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the client from environment variable
    let client = Client::from_env()?;

    // Example 1: Simple prompt with Claude
    let claude = client.completion_model("anthropic/claude-2");
    let request = CompletionRequest {
        prompt: Message::user("What is the capital of France?"),
        preamble: None,
        chat_history: vec![],
        documents: vec![],
        temperature: Some(0.7),
        max_tokens: None,
        tools: vec![],
        additional_params: None,
    };
    let response = claude.completion(request).await?;
    if let Some(AssistantContent::Text(text)) = response.choice.iter().next() {
        println!("Claude's response: {}", text.text);
    }

    // Example 2: Chat completion with GPT-4
    let gpt4 = client.completion_model("openai/gpt-4o-mini");
    let request = CompletionRequest {
        prompt: Message::user("Explain quantum computing"),
        preamble: Some("You are a helpful physics professor.".to_string()),
        chat_history: vec![],
        documents: vec![],
        temperature: Some(0.7),
        max_tokens: None,
        tools: vec![],
        additional_params: None,
    };
    let response = gpt4.completion(request).await?;
    if let Some(AssistantContent::Text(text)) = response.choice.iter().next() {
        println!("GPT-4's response: {}", text.text);
    }

    // Example 3: Using different models
    let models = vec![
        "google/gemini-2.0-flash-001",
        "mistralai/mistral-nemo",
        "deepseek/deepseek-r1",
        "minimax/minimax-01",
        "qwen/qwen2.5-vl-72b-instruct:free",
    ];

    for model_name in models {
        let model = client.completion_model(model_name);
        let request = CompletionRequest {
            prompt: Message::user("Write a haiku about programming"),
            preamble: None,
            chat_history: vec![],
            documents: vec![],
            temperature: Some(0.7),
            max_tokens: None,
            tools: vec![],
            additional_params: None,
        };
        match model.completion(request).await {
            Ok(response) => {
                if let Some(AssistantContent::Text(text)) = response.choice.iter().next() {
                    println!("{} response:\n{}\n", model_name, text.text);
                }
            },
            Err(e) => println!("Error with {}: {}", model_name, e),
        }
    }

    // Example 4: Using the agent interface
    let agent = client
        .agent("amazon/nova-lite-v1")
        .build();

    let chat_history = vec![];  // Empty chat history
    let response = agent.chat(Message::user("Find me recent papers about rust programming language"), chat_history).await?;
    println!("Agent response: {}", response);

    Ok(())
} 
