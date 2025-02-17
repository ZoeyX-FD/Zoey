use anyhow::Result;
use dotenv::dotenv;
use std::{env, path::Path};
use serde::{Deserialize, Serialize};
use tokio::fs as tokio_fs;

#[derive(Debug, Serialize)]
struct ExaSearchRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    contents: Contents,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_results: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_domains: Option<Vec<String>>,  // Add specific domains to search
    #[serde(skip_serializing_if = "Option::is_none")]
    exclude_domains: Option<Vec<String>>,  // Exclude specific domains
    #[serde(skip_serializing_if = "Option::is_none")]
    start_published_date: Option<String>,  // Filter by publication date
    #[serde(skip_serializing_if = "Option::is_none")]
    end_published_date: Option<String>,
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

#[derive(Debug, Serialize)]
struct Extras {
    image_links: i32,  // Number of images to return per result
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
    #[serde(default)]
    image: Option<String>,  // URL of the main image
    #[serde(default)]
    extras: Option<ResultExtras>,
}

#[derive(Debug, Deserialize)]
struct ResultExtras {
    #[serde(default)]
    image_links: Vec<String>,  // Additional image URLs
}

async fn download_image(client: &reqwest::Client, image_url: &str, file_name: &str) -> Result<()> {
    let response = client.get(image_url).send().await?;
    if response.status().is_success() {
        let bytes = response.bytes().await?;
        
        // Create images directory if it doesn't exist
        let images_dir = Path::new("crypto_images");
        if !images_dir.exists() {
            tokio_fs::create_dir_all(images_dir).await?;
        }

        // Save the image
        let path = images_dir.join(file_name);
        tokio_fs::write(path, &bytes).await?;
        println!("✅ Saved image: {}", file_name);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();
    let exa_api_key = env::var("EXA_API_KEY").expect("EXA_API_KEY must be set");

    // Create reqwest client
    let client = reqwest::Client::new();

    // Example 1: Recent market analysis with date filtering
    println!("\n🔍 Searching for recent Bitcoin market analysis...");
    let search_request = ExaSearchRequest {
        query: "Bitcoin price analysis forecast market trends last 7 days".to_string(),
        category: Some("news".to_string()),
        contents: Contents {
            text: true,
            highlights: Some(Highlights {
                highlights_per: 3,
            }),
            extras: None,
        },
        num_results: Some(3),
        include_domains: None,
        exclude_domains: None,
        start_published_date: None,
        end_published_date: None,
    };

    let response = client
        .post("https://api.exa.ai/search")
        .header("Authorization", format!("Bearer {}", exa_api_key))
        .header("Content-Type", "application/json")
        .json(&search_request)
        .send()
        .await?;

    match response.status() {
        reqwest::StatusCode::OK => {
            let result: ExaResponse = response.json().await?;
            println!("\n📊 Bitcoin Market Analysis Results:");
            for item in result.results {
                println!("\n📰 {}", item.title);
                println!("URL: {}", item.url);
                
                if let Some(summary) = item.summary {
                    println!("\nSummary: {}", summary);
                }

                println!("\nHighlights:");
                for (highlight, score) in item.highlights.iter().zip(item.highlight_scores.iter()) {
                    println!("• {} (score: {:.2})", highlight, score);
                }

                println!("\nFull Text:\n{}", item.text.unwrap_or_default());
                println!("\n---");
            }
        }
        status => {
            let error_text = response.text().await?;
            println!("Error: {} - {}", status, error_text);
            // Print the full response for debugging
            println!("\nFull response: {}", error_text);
        }
    }

    // Example 2: Technical research with domain filtering
    println!("\n🔍 Searching for Ethereum technical developments...");
    let eth_request = ExaSearchRequest {
        query: "Ethereum 2.0 technical developments consensus layer execution layer".to_string(),
        category: Some("research".to_string()),
        contents: Contents {
            text: true,
            highlights: Some(Highlights {
                highlights_per: 3,
            }),
            extras: None,
        },
        num_results: Some(3),
        include_domains: None,
        exclude_domains: None,
        start_published_date: None,
        end_published_date: None,
    };

    let response = client
        .post("https://api.exa.ai/search")
        .header("Authorization", format!("Bearer {}", exa_api_key))
        .header("Content-Type", "application/json")
        .json(&eth_request)
        .send()
        .await?;

    match response.status() {
        reqwest::StatusCode::OK => {
            let result: ExaResponse = response.json().await?;
            println!("\n🛠 Ethereum Technical Updates:");
            for item in result.results {
                println!("\n📝 {}", item.title);
                println!("URL: {}", item.url);
                
                if let Some(summary) = item.summary {
                    println!("\nSummary: {}", summary);
                }

                println!("\nKey Points:");
                for (highlight, score) in item.highlights.iter().zip(item.highlight_scores.iter()) {
                    println!("• {} (relevance: {:.2})", highlight, score);
                }

                println!("\nContent:\n{}", item.text.unwrap_or_default());
                println!("\n---");
            }
        }
        status => {
            let error_text = response.text().await?;
            println!("Error: {} - {}", status, error_text);
            // Print the full response for debugging
            println!("\nFull response: {}", error_text);
        }
    }

    // Example 3: Specific topic search
    println!("\n🔍 Searching for DeFi security analysis...");
    let defi_request = ExaSearchRequest {
        query: "DeFi protocol security vulnerabilities audit findings".to_string(),
        category: Some("research".to_string()),
        contents: Contents {
            text: true,
            highlights: Some(Highlights {
                highlights_per: 3,
            }),
            extras: None,
        },
        num_results: Some(3),
        include_domains: None,
        exclude_domains: None,
        start_published_date: None,
        end_published_date: None,
    };

    let response = client
        .post("https://api.exa.ai/search")
        .header("Authorization", format!("Bearer {}", exa_api_key))
        .header("Content-Type", "application/json")
        .json(&defi_request)
        .send()
        .await?;

    match response.status() {
        reqwest::StatusCode::OK => {
            let result: ExaResponse = response.json().await?;
            println!("\n🔒 DeFi Security Analysis Results:");
            for item in result.results {
                println!("\n📑 {}", item.title);
                println!("URL: {}", item.url);
                
                if let Some(summary) = item.summary {
                    println!("\nSummary: {}", summary);
                }

                println!("\nKey Findings:");
                for (highlight, score) in item.highlights.iter().zip(item.highlight_scores.iter()) {
                    println!("• {} (relevance: {:.2})", highlight, score);
                }

                println!("\nDetailed Analysis:\n{}", item.text.unwrap_or_default());
                println!("\n---");
            }
        }
        status => {
            let error_text = response.text().await?;
            println!("Error: {} - {}", status, error_text);
            println!("\nFull response: {}", error_text);
        }
    }

    // Example 4: PDF Document Search and Read
    println!("\n🔍 Searching for Cryptocurrency Whitepapers...");
    let pdf_request = ExaSearchRequest {
        query: "cryptocurrency blockchain whitepaper technical documentation filetype:pdf".to_string(),
        category: Some("pdf".to_string()),
        contents: Contents {
            text: true,
            highlights: Some(Highlights {
                highlights_per: 5,
            }),
            extras: None,
        },
        num_results: Some(2),
        include_domains: None,
        exclude_domains: None,
        start_published_date: None,
        end_published_date: None,
    };

    let response = client
        .post("https://api.exa.ai/search")
        .header("Authorization", format!("Bearer {}", exa_api_key))
        .header("Content-Type", "application/json")
        .json(&pdf_request)
        .send()
        .await?;

    match response.status() {
        reqwest::StatusCode::OK => {
            let result: ExaResponse = response.json().await?;
            println!("\n📚 PDF Documents Found:");
            for item in result.results {
                println!("\n📑 {}", item.title);
                println!("PDF URL: {}", item.url);
                
                if let Some(summary) = item.summary {
                    println!("\n📝 Executive Summary:");
                    println!("{}", summary);
                }

                println!("\n🔍 Key Sections:");
                for (highlight, score) in item.highlights.iter().zip(item.highlight_scores.iter()) {
                    println!("• {} (relevance: {:.2})", highlight, score);
                }

                if let Some(text) = item.text {
                    println!("\n📄 Document Content Preview:");
                    // Print first 500 characters of the PDF content
                    let preview: String = text.chars().take(500).collect();
                    println!("{}", preview);
                    println!("\n[... Document continues ...]");
                }
                println!("\n---");
            }
        }
        status => {
            let error_text = response.text().await?;
            println!("Error: {} - {}", status, error_text);
            println!("\nFull response: {}", error_text);
        }
    }

    // Example 5: Image Search and Download
    println!("\n🔍 Searching for NFT artwork and crypto-related images...");
    let image_request = ExaSearchRequest {
        query: "popular NFT artwork crypto art blockchain visualization".to_string(),
        category: None,
        contents: Contents {
            text: true,
            highlights: Some(Highlights {
                highlights_per: 2,
            }),
            extras: Some(Extras {
                image_links: 3,  // Request up to 3 images per result
            }),
        },
        num_results: Some(2),
        include_domains: None,
        exclude_domains: None,
        start_published_date: None,
        end_published_date: None,
    };

    let response = client
        .post("https://api.exa.ai/search")
        .header("Authorization", format!("Bearer {}", exa_api_key))
        .header("Content-Type", "application/json")
        .json(&image_request)
        .send()
        .await?;

    match response.status() {
        reqwest::StatusCode::OK => {
            let result: ExaResponse = response.json().await?;
            println!("\n🎨 NFT and Crypto Art Results:");
            for (idx, item) in result.results.iter().enumerate() {
                println!("\n🖼 {}", item.title);
                println!("Source: {}", item.url);

                // Download main image if available
                if let Some(image_url) = &item.image {
                    let file_name = format!("main_image_{}.jpg", idx);
                    if let Err(e) = download_image(&client, image_url, &file_name).await {
                        println!("❌ Failed to download main image: {}", e);
                    }
                }

                // Download additional images if available
                if let Some(extras) = &item.extras {
                    for (img_idx, img_url) in extras.image_links.iter().enumerate() {
                        let file_name = format!("additional_image_{}_{}.jpg", idx, img_idx);
                        if let Err(e) = download_image(&client, img_url, &file_name).await {
                            println!("❌ Failed to download additional image: {}", e);
                        }
                    }
                }

                if let Some(summary) = &item.summary {
                    println!("\n📝 Description:");
                    println!("{}", summary);
                }

                println!("\nContext:");
                for (highlight, score) in item.highlights.iter().zip(item.highlight_scores.iter()) {
                    println!("• {} (relevance: {:.2})", highlight, score);
                }
                println!("\n---");
            }
        }
        status => {
            let error_text = response.text().await?;
            println!("Error: {} - {}", status, error_text);
            println!("\nFull response: {}", error_text);
        }
    }

    // Example 6: Custom Advanced Search
    println!("\n🔍 Performing custom advanced search...");
    let custom_request = ExaSearchRequest {
        // Complex query with multiple conditions
        query: "Solana (NFT OR DeFi) (development OR ecosystem) -gaming site:decrypt.co OR site:cointelegraph.com".to_string(),
        
        category: Some("news".to_string()),
        
        // Include specific domains for higher quality results
        include_domains: Some(vec![
            "decrypt.co".to_string(),
            "cointelegraph.com".to_string(),
            "coindesk.com".to_string(),
        ]),
        
        // Remove exclude_domains as we can't use both
        exclude_domains: None,
        
        // Set date range (ISO 8601 format)
        start_published_date: Some("2024-01-01T00:00:00Z".to_string()),
        end_published_date: Some("2024-12-31T23:59:59Z".to_string()),
        
        contents: Contents {
            text: true,
            highlights: Some(Highlights {
                highlights_per: 4,
            }),
            extras: None,
        },
        num_results: Some(5),
    };

    let response = client
        .post("https://api.exa.ai/search")
        .header("Authorization", format!("Bearer {}", exa_api_key))
        .header("Content-Type", "application/json")
        .json(&custom_request)
        .send()
        .await?;

    match response.status() {
        reqwest::StatusCode::OK => {
            let result: ExaResponse = response.json().await?;
            println!("\n🎯 Custom Search Results:");
            for item in result.results {
                println!("\n📄 {}", item.title);
                println!("Source: {}", item.url);
                
                if let Some(summary) = item.summary {
                    println!("\n📝 Summary:");
                    println!("{}", summary);
                }

                println!("\n🔍 Relevant Excerpts:");
                for (highlight, score) in item.highlights.iter().zip(item.highlight_scores.iter()) {
                    println!("• {} (relevance: {:.2})", highlight, score);
                }

                if let Some(text) = item.text {
                    println!("\n📃 Content Preview:");
                    let preview: String = text.chars().take(300).collect();
                    println!("{}", preview);
                    println!("\n[... Content continues ...]");
                }
                println!("\n---");
            }
        }
        status => {
            let error_text = response.text().await?;
            println!("Error: {} - {}", status, error_text);
            println!("\nFull response: {}", error_text);
        }
    }

    // Example 7: Pas Normal Studios Product Search
    println!("\n🔍 Searching Pas Normal Studios jerseys collection...");
    let pas_request = ExaSearchRequest {
        query: "site:pasnormalstudios.com/int/collections/jerseys-men cycling jersey collection".to_string(),
        category: None,
        contents: Contents {
            text: true,
            highlights: Some(Highlights {
                highlights_per: 5,
            }),
            extras: Some(Extras {
                image_links: 5,  // Get up to 5 product images per result
            }),
        },
        num_results: Some(20),  // Get more results for products
        include_domains: Some(vec![
            "pasnormalstudios.com".to_string(),
        ]),
        exclude_domains: None,
        start_published_date: None,
        end_published_date: None,
    };

    let response = client
        .post("https://api.exa.ai/search")
        .header("Authorization", format!("Bearer {}", exa_api_key))
        .header("Content-Type", "application/json")
        .json(&pas_request)
        .send()
        .await?;

    match response.status() {
        reqwest::StatusCode::OK => {
            let result: ExaResponse = response.json().await?;
            println!("\n👕 Pas Normal Studios Jersey Collection:");
            
            // Create a specific directory for PAS products
            let pas_dir = Path::new("pas_products");
            if !pas_dir.exists() {
                tokio_fs::create_dir_all(pas_dir).await?;
            }

            for (idx, item) in result.results.iter().enumerate() {
                println!("\n🏷️ Product: {}", item.title);
                println!("🔗 URL: {}", item.url);
                
                if let Some(summary) = &item.summary {
                    println!("\n📝 Product Description:");
                    println!("{}", summary);
                }

                // Download product images
                if let Some(image_url) = &item.image {
                    let file_name = format!("jersey_main_{}.jpg", idx);
                    let path = pas_dir.join(&file_name);
                    if let Err(e) = download_image(&client, image_url, &file_name).await {
                        println!("❌ Failed to download main product image: {}", e);
                    } else {
                        println!("✅ Saved main product image: {}", path.display());
                    }
                }

                // Download additional product images
                if let Some(extras) = &item.extras {
                    println!("\n📸 Product Variants:");
                    for (img_idx, img_url) in extras.image_links.iter().enumerate() {
                        let file_name = format!("jersey_variant_{}_{}.jpg", idx, img_idx);
                        let path = pas_dir.join(&file_name);
                        if let Err(e) = download_image(&client, img_url, &file_name).await {
                            println!("❌ Failed to download variant image: {}", e);
                        } else {
                            println!("✅ Saved variant image: {}", path.display());
                        }
                    }
                }

                println!("\n🏷️ Product Details:");
                for (highlight, score) in item.highlights.iter().zip(item.highlight_scores.iter()) {
                    println!("• {} (relevance: {:.2})", highlight, score);
                }

                if let Some(text) = &item.text {
                    println!("\n📋 Full Product Information:");
                    // Extract and print key product information
                    let preview: String = text.chars().take(500).collect();
                    println!("{}", preview);
                    println!("\n[... More details available ...]");
                }
                println!("\n---");
            }

            // Save collection summary to a file
            let summary_path = pas_dir.join("collection_summary.txt");
            let mut summary = String::new();
            summary.push_str("Pas Normal Studios Jersey Collection Summary\n");
            summary.push_str("=====================================\n\n");
            
            for item in result.results {
                summary.push_str(&format!("Product: {}\n", item.title));
                summary.push_str(&format!("URL: {}\n", item.url));
                if let Some(text) = item.text {
                    summary.push_str("\nDescription:\n");
                    summary.push_str(&text);
                }
                summary.push_str("\n---\n\n");
            }

            tokio_fs::write(summary_path, summary).await?;
            println!("\n✅ Saved collection summary to collection_summary.txt");
        }
        status => {
            let error_text = response.text().await?;
            println!("Error: {} - {}", status, error_text);
            println!("\nFull response: {}", error_text);
        }
    }

    Ok(())
} 