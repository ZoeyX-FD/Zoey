use common::providers::{GraniteEmbedding, GraniteVector};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting Granite Embedding example...");

    // Initialize the Granite embedding client
    let granite = GraniteEmbedding::new();

    // Example 1: Single text embedding
    let text = "This is a sample text for embedding generation";
    println!("\nGenerating embedding for text: '{}'", text);
    
    let embedding = granite.get_embedding(text).await?;
    let vector = GraniteVector::new(embedding);
    println!("Generated embedding with {} dimensions", vector.as_slice().len());

    // Example 2: Batch text embedding
    let texts = vec![
        "The quick brown fox jumps over the lazy dog".to_string(),
        "Machine learning is fascinating".to_string(),
        "Rust is a systems programming language".to_string(),
    ];
    println!("\nGenerating embeddings for {} texts", texts.len());

    let embeddings = granite.get_batch_embeddings(&texts).await?;
    println!("Generated {} embeddings", embeddings.len());

    // Example 3: Computing similarity between texts
    let vectors: Vec<GraniteVector> = embeddings.into_iter()
        .map(GraniteVector::new)
        .collect();

    println!("\nComputing similarities between texts:");
    for i in 0..vectors.len() {
        for j in i+1..vectors.len() {
            let similarity = vectors[i].cosine_similarity(&vectors[j]);
            println!(
                "Similarity between '{}' and '{}': {:.4}",
                texts[i], texts[j], similarity
            );
        }
    }

    // Example 4: Document comparison
    let doc1 = "Artificial intelligence is transforming technology";
    let doc2 = "AI and machine learning are changing the tech landscape";
    let doc3 = "The weather is nice today";

    println!("\nComparing document similarities:");
    let emb1 = GraniteVector::new(granite.get_embedding(doc1).await?);
    let emb2 = GraniteVector::new(granite.get_embedding(doc2).await?);
    let emb3 = GraniteVector::new(granite.get_embedding(doc3).await?);

    println!(
        "Similarity between related docs (1-2): {:.4}",
        emb1.cosine_similarity(&emb2)
    );
    println!(
        "Similarity between unrelated docs (1-3): {:.4}",
        emb1.cosine_similarity(&emb3)
    );

    Ok(())
} 