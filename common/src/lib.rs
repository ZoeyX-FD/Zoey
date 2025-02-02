pub mod traits;
pub mod error;
pub mod character;
pub mod document_loader;
pub mod providers;

pub use error::AgentError;
pub use document_loader::DocumentLoader;  // Re-export DocumentLoader
pub use providers::*;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use rig::embeddings::{TextEmbedder, EmbedError};
use rig::Embed;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Document {
    pub id: String,
    pub content: String,
}

impl Embed for Document {
    fn embed(&self, embedder: &mut TextEmbedder) -> Result<(), EmbedError> {
        embedder.embed(self.content.clone());
        Ok(())
    }
}