use anyhow::Result;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::error::AgentError;

const OLLAMA_API_BASE: &str = "http://localhost:11434/api";

#[derive(Debug, Clone)]
pub struct GraniteEmbedding {
    client: Client,
    model: String,
}

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

impl GraniteEmbedding {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            model: "granite-embedding:278m".to_string(),
        }
    }

    pub async fn get_embedding(&self, text: &str) -> Result<Vec<f32>, AgentError> {
        let request = EmbeddingRequest {
            model: self.model.clone(),
            prompt: text.to_string(),
        };

        let response = self.client
            .post(&format!("{}/embeddings", OLLAMA_API_BASE))
            .json(&request)
            .send()
            .await
            .map_err(|e| AgentError::ExternalApiError(e.to_string()))?;

        let embedding_response = response
            .json::<EmbeddingResponse>()
            .await
            .map_err(|e| AgentError::ParseError(e.to_string()))?;

        Ok(embedding_response.embedding)
    }

    pub async fn get_batch_embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AgentError> {
        let mut embeddings = Vec::with_capacity(texts.len());
        
        for text in texts {
            let embedding = self.get_embedding(text).await?;
            embeddings.push(embedding);
        }
        
        Ok(embeddings)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraniteVector(Vec<f32>);

impl GraniteVector {
    pub fn new(vector: Vec<f32>) -> Self {
        Self(vector)
    }

    pub fn cosine_similarity(&self, other: &Self) -> f32 {
        let dot_product: f32 = self.0.iter()
            .zip(other.0.iter())
            .map(|(a, b)| a * b)
            .sum();

        let norm_a: f32 = self.0.iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();

        let norm_b: f32 = other.0.iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();

        dot_product / (norm_a * norm_b)
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }
} 