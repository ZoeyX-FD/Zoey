use anyhow::Result;
use chrono;
use rig::{
    providers::cohere,
    Embed,
};
use rig_sqlite::{Column, ColumnValue, SqliteVectorStore, SqliteVectorStoreTable};
use tokio_rusqlite::Connection;
use uuid;

// Document struct for storing loaded content
#[derive(Debug, Clone, Embed)]
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
                let embeddings = rig::embeddings::EmbeddingsBuilder::new(embedding_model)
                    .documents(docs)?
                    .build()
                    .await?;
                store.add_rows(embeddings).await?;
            }
        }
        
        Ok(())
    }

    pub async fn add_document(&self, source: &str, content: &str) -> Result<Document> {
        let now = chrono::Local::now();
        let uuid = uuid::Uuid::new_v4().to_string(); // Generate unique ID
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
            conn.execute(
                "INSERT OR IGNORE INTO documents (id, source, timestamp, content) VALUES (?1, ?2, ?3, ?4)",
                [&id, &source, &timestamp, &content],
            )?;
            Ok(())
        }).await?;

        // Add to vector store
        if let (Some(store), Some(model)) = (&self.store, &self.model) {
            let embeddings = rig::embeddings::EmbeddingsBuilder::new(model.clone())
                .documents(vec![doc.clone()])?
                .build()
                .await?;
            store.add_rows(embeddings).await?;
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
        self.conn.call(|conn| {
            conn.execute("DELETE FROM documents", [])?;
            conn.execute("DELETE FROM documents_embeddings", [])?;
            Ok(())
        }).await?;
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
            // Note: vec0 extension automatically adds rowid and distance columns
            // Using 1024 dimensions for Cohere's model
            conn.execute(
                "CREATE VIRTUAL TABLE documents_embeddings USING vec0(embedding FLOAT[1024])",
                [],
            )?;

            Ok(())
        }).await?;
        Ok(())
    }
} 