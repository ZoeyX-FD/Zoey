pub mod traits;
pub mod error;
pub mod character;
pub mod document_loader;
pub mod providers;
pub mod storage;
pub mod exa;

pub use error::AgentError;
pub use document_loader::DocumentLoader;  // Re-export DocumentLoader
pub use providers::*;

// Re-export providers for easier access
pub use providers::mistral;
pub use providers::openrouter;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use rig::embeddings::{TextEmbedder, EmbedError};
use rig::Embed;
use rig_sqlite::{Column, ColumnValue, SqliteVectorStoreTable};

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

impl SqliteVectorStoreTable for Document {
    fn name() -> &'static str {
        "documents"
    }

    fn schema() -> Vec<Column> {
        vec![
            Column::new("id", "TEXT PRIMARY KEY"),
            Column::new("content", "TEXT"),
        ]
    }

    fn id(&self) -> String {
        self.id.clone()
    }

    fn column_values(&self) -> Vec<(&'static str, Box<dyn ColumnValue>)> {
        vec![
            ("id", Box::new(self.id.clone())),
            ("content", Box::new(self.content.clone())),
        ]
    }
}