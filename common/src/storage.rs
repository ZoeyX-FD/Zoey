use anyhow::Result;
use chrono;
use rig::{
    providers::cohere,
    Embed,
};
use rig_sqlite::{Column, ColumnValue, SqliteVectorStore, SqliteVectorStoreTable};
use tokio_rusqlite::Connection;
use uuid;
use tracing::info;
use serde::{Serialize, Deserialize};

// Document struct for storing loaded content
#[derive(Debug, Clone, Embed, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub source: String,
    pub timestamp: String,
    #[embed]
    pub content: String,
}

// Implement SqliteVectorStoreTable for Document to match database schema
impl SqliteVectorStoreTable for Document {
    fn name() -> &'static str {
        "documents"
    }

    fn schema() -> Vec<Column> {
        vec![
            Column::new("id", "TEXT PRIMARY KEY"),
            Column::new("source", "TEXT NOT NULL"),
            Column::new("timestamp", "TEXT NOT NULL"), 
            Column::new("content", "TEXT NOT NULL")
        ]
    }

    fn id(&self) -> String {
        self.id.clone()
    }

    fn column_values(&self) -> Vec<(&'static str, Box<dyn ColumnValue>)> {
        vec![
            ("id", Box::new(self.id.clone())),
            ("source", Box::new(self.source.clone())),
            ("timestamp", Box::new(self.timestamp.clone())),
            ("content", Box::new(self.content.clone()))
        ]
    }
}

// Storage manager struct
pub struct StorageManager {
    conn: Connection,
    store: Option<SqliteVectorStore<cohere::EmbeddingModel, Document>>,
    model: Option<cohere::EmbeddingModel>,
}

impl StorageManager {
    pub async fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path).await?;
        Ok(Self {
            conn,
            store: None,
            model: None,
        })
    }

    pub fn get_store(&self) -> Option<&SqliteVectorStore<cohere::EmbeddingModel, Document>> {
        self.store.as_ref()
    }

    pub async fn initialize_store(&mut self, embedding_model: cohere::EmbeddingModel) -> Result<()> {
        let store = SqliteVectorStore::new(self.conn.clone(), &embedding_model).await?;
        self.store = Some(store);
        self.model = Some(embedding_model.clone());
        
        // Load existing documents into vector store
        let docs = self.get_documents().await?;
        if !docs.is_empty() {
            if let Some(store) = &self.store {
                let docs_count = docs.len(); // Store count before moving docs
                let embeddings = rig::embeddings::EmbeddingsBuilder::new(embedding_model)
                    .documents(docs)?
                    .build()
                    .await?;
                store.add_rows(embeddings).await?;
                info!("Loaded {} existing documents into vector store", docs_count);
            }
        }
        
        Ok(())
    }

    pub async fn add_document(&self, source: &str, content: &str) -> Result<Document> {
        let now = chrono::Local::now();
        let uuid = uuid::Uuid::new_v4().to_string();
        let doc = Document {
            id: format!("doc_{}_{}", now.timestamp(), uuid),
            source: source.to_string(),
            timestamp: now.to_rfc3339(),
            content: content.to_string(),
        };

        // Clone values for the closure
        let id = doc.id.clone();
        let source = doc.source.clone();
        let timestamp = doc.timestamp.clone();
        let content = doc.content.clone();

        // Insert into database
        self.conn.call(move |conn| {
            // Start a transaction
            let tx = conn.transaction()?;
            
            // Delete any existing document with same source to avoid duplicates
            tx.execute(
                "DELETE FROM documents WHERE source = ?1",
                [&source],
            )?;
            
            // Insert new document
            tx.execute(
                "INSERT INTO documents (id, source, timestamp, content) VALUES (?1, ?2, ?3, ?4)",
                [&id, &source, &timestamp, &content],
            )?;
            
            // Commit transaction
            tx.commit()?;
            Ok(())
        }).await?;

        // Add to vector store
        if let (Some(store), Some(model)) = (&self.store, &self.model) {
            info!("Generating embeddings for document: {}", doc.source);
            let embeddings = rig::embeddings::EmbeddingsBuilder::new(model.clone())
                .documents(vec![doc.clone()])?
                .build()
                .await?;
            
            info!("Adding document embeddings to vector store");
            store.add_rows(embeddings).await?;
            info!("Successfully added document embeddings");
        }

        Ok(doc)
    }

    pub async fn get_documents(&self) -> Result<Vec<Document>> {
        let docs = self.conn.call(|conn| {
            let mut stmt = conn.prepare("SELECT id, source, timestamp, content FROM documents")?;
            let rows = stmt.query_map([], |row| {
                Ok(Document {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    timestamp: row.get(2)?,
                    content: row.get(3)?,
                })
            })?;
            
            let mut docs = Vec::new();
            for row in rows {
                docs.push(row?);
            }
            Ok(docs)
        }).await?;
        
        Ok(docs)
    }

    pub async fn clear_documents(&self) -> Result<()> {
        // Clear documents table first
        self.conn.call(|conn| {
            // Start transaction
            let tx = conn.transaction()?;
            
            // Clear both tables
            tx.execute("DELETE FROM documents", [])?;
            tx.execute("DELETE FROM documents_embeddings", [])?;
            
            // Commit changes
            tx.commit()?;
            Ok(())
        }).await?;
        
        info!("Cleared all documents and embeddings");
        Ok(())
    }

    pub async fn initialize_tables(&self) -> Result<()> {
        self.conn.call(|conn| {
            // Drop existing tables if they exist
            conn.execute("DROP TABLE IF EXISTS documents", [])?;
            conn.execute("DROP TABLE IF EXISTS documents_embeddings", [])?;

            // Create new tables with correct schema
            conn.execute(
                "CREATE TABLE documents (
                    id TEXT PRIMARY KEY,
                    source TEXT NOT NULL,
                    timestamp TEXT NOT NULL,
                    content TEXT NOT NULL
                )",
                [],
            )?;

            // Create embeddings table with vector search support
            conn.execute(
                "CREATE VIRTUAL TABLE documents_embeddings USING vec0(
                    embedding FLOAT[1024]
                )",
                [],
            )?;

            Ok(())
        }).await?;
        Ok(())
    }

    pub async fn initialize_store_with_mode(&mut self, embedding_model: cohere::EmbeddingModel, persistent: bool) -> Result<()> {
        let store = SqliteVectorStore::new(self.conn.clone(), &embedding_model).await?;
        self.store = Some(store);
        self.model = Some(embedding_model.clone());
        
        if persistent {
            // Load existing documents into vector store
            info!("Loading existing documents from persistent storage...");
            let docs = self.get_documents().await?;
            if !docs.is_empty() {
                info!("Found {} existing documents", docs.len());
                if let Some(store) = &self.store {
                    let docs_count = docs.len(); // Store count before moving docs
                    let embeddings = rig::embeddings::EmbeddingsBuilder::new(embedding_model)
                        .documents(docs)?
                        .build()
                        .await?;
                    store.add_rows(embeddings).await?;
                    info!("Restored {} documents from persistent storage", docs_count);
                }
            }
        } else {
            // Clear any existing documents for fresh session
            info!("Starting fresh session, clearing existing documents...");
            self.clear_documents().await?;
        }
        
        Ok(())
    }

    pub async fn new_with_mode(persistent: bool) -> Result<Self> {
        // Use in-memory database for non-persistent mode
        let db_path = if persistent {
            "zoey.db".to_string()
        } else {
            ":memory:".to_string()  // SQLite in-memory database
        };
        
        let conn = Connection::open(&db_path).await?;
        Ok(Self {
            conn,
            store: None,
            model: None,
        })
    }
} 