use anyhow::Result;
use common::providers::openrouter::Client;
use rig::completion::{CompletionModel, CompletionRequest, Chat, ModelChoice};


#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the client from environment variable
    let client = Client::from_env()?;

    // Example 1: Simple prompt with Claude
    let claude = client.completion_model("anthropic/claude-2");
    let request = CompletionRequest {
        prompt: "What is the capital of France?".to_string(),
        preamble: None,
        chat_history: vec![],
        documents: vec![],
        temperature: Some(0.7),
        max_tokens: None,
        tools: vec![],
        additional_params: None,
    };
    let response = claude.completion(request).await?;
    if let ModelChoice::Message(content) = response.choice {
        println!("Claude's response: {}", content);
    }

    // Example 2: Chat completion with GPT-4
    let gpt4 = client.completion_model("openai/gpt-4o-mini");
    let request = CompletionRequest {
        prompt: "Explain quantum computing".to_string(),
        preamble: Some("You are a helpful physics professor.".to_string()),
        chat_history: vec![],
        documents: vec![],
        temperature: Some(0.7),
        max_tokens: None,
        tools: vec![],
        additional_params: None,
    };
    let response = gpt4.completion(request).await?;
    if let ModelChoice::Message(content) = response.choice {
        println!("GPT-4's response: {}", content);
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
            prompt: "Write a haiku about programming".to_string(),
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
                if let ModelChoice::Message(content) = response.choice {
                    println!("{} response:\n{}\n", model_name, content);
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
    let response = agent.chat("Find me recent papers about rust programming language", chat_history).await?;
    println!("Agent response: {}", response);

    Ok(())
} 