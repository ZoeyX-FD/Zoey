use rig::{
    embeddings::EmbeddingsBuilder,
    vector_store::VectorStoreError,
};
use rig::embeddings::embedding::EmbeddingModel;
use tokio_rusqlite::Connection;
use tracing::{debug, info, error};

use super::models::{Account, Channel, Document, Message, TradeAction, Trade};
use rig_sqlite::{SqliteError, SqliteVectorIndex, SqliteVectorStore};
use rusqlite::OptionalExtension;

#[derive(Clone)]
pub struct KnowledgeBase<E: EmbeddingModel + Clone + 'static> {
    conn: Connection,
    document_store: SqliteVectorStore<E, Document>,
    message_store: SqliteVectorStore<E, Message>,
    embedding_model: E,
}

impl<E: EmbeddingModel> KnowledgeBase<E> {
    pub async fn new(conn: Connection, embedding_model: E) -> Result<Self, VectorStoreError> {
        info!("Initializing KnowledgeBase with vector stores");
        let document_store = SqliteVectorStore::new(conn.clone(), &embedding_model).await?;
        let message_store = SqliteVectorStore::new(conn.clone(), &embedding_model).await?;

        debug!("Creating database schema");
        conn.call(|conn| {
            conn.execute_batch(
                "BEGIN;
                -- User management tables
                CREATE TABLE IF NOT EXISTS accounts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    source_id TEXT NOT NULL UNIQUE,
                    source TEXT NOT NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX IF NOT EXISTS idx_source_id_source ON accounts(source_id, source);

                -- Channel tables
                CREATE TABLE IF NOT EXISTS channels (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    channel_id TEXT NOT NULL UNIQUE,
                    channel_type TEXT NOT NULL,
                    source TEXT NOT NULL,
                    name TEXT,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX IF NOT EXISTS idx_channel_id_type ON channels(channel_id, channel_type);

                -- Trade table
                CREATE TABLE IF NOT EXISTS trade (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    wallet_address TEXT NOT NULL,
                    action TEXT NOT NULL,
                    token_address TEXT NOT NULL,
                    amount REAL NOT NULL,
                    reason TEXT NOT NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    signature TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_wallet_created_at 
                ON trade(wallet_address, created_at);

                COMMIT;"
            )
            .map_err(tokio_rusqlite::Error::from)
        })
        .await
        .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

        info!("KnowledgeBase initialized successfully");
        Ok(Self {
            conn,
            document_store,
            message_store,
            embedding_model,
        })
    }

    pub async fn create_user(&self, name: String, source: String) -> Result<i64, SqliteError> {
        info!(name = %name, source = %source, "Creating new user");
        let result = self.conn
            .call(move |conn| {
                conn.query_row(
                    "INSERT INTO accounts (name, source, created_at, updated_at)
                     VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                     ON CONFLICT(name) DO UPDATE SET 
                         updated_at = CURRENT_TIMESTAMP
                     RETURNING id",
                    rusqlite::params![name, source],
                    |row| row.get(0),
                )
                .map_err(tokio_rusqlite::Error::from)
            })
            .await
            .map_err(|e| SqliteError::DatabaseError(Box::new(e)));

        match &result {
            Ok(id) => debug!(user_id = %id, "User created/updated successfully"),
            Err(e) => error!(error = ?e, "Failed to create/update user"),
        }

        result
    }

    pub fn document_index(self) -> SqliteVectorIndex<E, Document> {
        SqliteVectorIndex::new(self.embedding_model, self.document_store)
    }

    pub fn message_index(self) -> SqliteVectorIndex<E, Message> {
        SqliteVectorIndex::new(self.embedding_model, self.message_store)
    }

    pub async fn get_user_by_source(&self, source: String) -> Result<Option<Account>, SqliteError> {
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, source, created_at, updated_at FROM accounts WHERE source = ?1"
                )?;

                let account = stmt.query_row(rusqlite::params![source], |row| {
                    Account::try_from(row)
                }).optional()?;

                Ok(account)
            })
            .await
            .map_err(|e| SqliteError::DatabaseError(Box::new(e)))
    }

    pub async fn create_channel(
        &self,
        channel_id: String,
        channel_type: String,
        name: Option<String>,
    ) -> Result<i64, SqliteError> {
        self.conn
            .call(move |conn| {
                conn.query_row(
                    "INSERT INTO channels (channel_id, channel_type, name, created_at, updated_at)
                     VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                     ON CONFLICT(channel_id) DO UPDATE SET 
                         name = COALESCE(?3, name),
                         updated_at = CURRENT_TIMESTAMP
                     RETURNING id",
                    rusqlite::params![channel_id, channel_type, name],
                    |row| row.get(0),
                )
                .map_err(tokio_rusqlite::Error::from)
            })
            .await
            .map_err(|e| SqliteError::DatabaseError(Box::new(e)))
    }

    pub async fn get_channel(&self, id: i64) -> Result<Option<Channel>, SqliteError> {
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, source, created_at, updated_at FROM channels WHERE id = ?1",
                )?;

                let channel = stmt
                    .query_row(rusqlite::params![id], |row| Channel::try_from(row))
                    .optional()?;

                Ok(channel)
            })
            .await
            .map_err(|e| SqliteError::DatabaseError(Box::new(e)))
    }

    pub async fn get_channels_by_source(
        &self,
        source: String,
    ) -> Result<Vec<Channel>, SqliteError> {
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, source, created_at, updated_at FROM channels WHERE source = ?1"
                )?;

                let channels = stmt.query_map(rusqlite::params![source], |row| {
                    Channel::try_from(row)
                }).and_then(|mapped_rows| {
                    mapped_rows.collect::<Result<Vec<Channel>, _>>()
                })?;

                Ok(channels)
            })
            .await
            .map_err(|e| SqliteError::DatabaseError(Box::new(e)))
    }

    pub async fn create_message(&self, msg: Message) -> anyhow::Result<i64> {
        info!(
            source = %msg.source.as_str(),
            channel_type = %msg.channel_type.as_str(),
            channel_id = %msg.channel_id,
            "Creating new message"
        );

        debug!("Generating embeddings for message");
        let embeddings = EmbeddingsBuilder::new(self.embedding_model.clone())
            .documents(vec![msg.clone()])?
            .build()
            .await?;

        let store = self.message_store.clone();

        let result = self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;

                debug!("Upserting channel information");
                tx.execute(
                    "INSERT INTO channels (channel_id, channel_type, source, name, created_at, updated_at) 
                     VALUES (?1, ?2, ?3, NULL, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                     ON CONFLICT (channel_id) DO UPDATE SET 
                     updated_at = CURRENT_TIMESTAMP",
                    [
                        &msg.channel_id,
                        &msg.channel_type.as_str().to_string(),
                        &msg.source.as_str().to_string(),
                    ],
                )?;

                debug!("Storing message with embeddings");
                let id = store
                    .add_rows_with_txn(&tx, embeddings)
                    .map_err(tokio_rusqlite::Error::from)?;

                tx.commit()?;
                
                debug!(message_id = %id, "Message stored successfully");
                Ok(id)
            })
            .await
            .map_err(anyhow::Error::from);

        if let Err(ref e) = result {
            error!(error = %e, "Failed to store message");
        }

        result
    }

    pub async fn get_message(&self, id: i64) -> Result<Option<Message>, SqliteError> {
        debug!(message_id = %id, "Fetching message");
        let result = self.conn
            .call(move |conn| {
                Ok(conn.prepare("SELECT id, source, source_id, channel_type, channel_id, account_id, role, content, created_at FROM messages WHERE id = ?1")?
                    .query_row(rusqlite::params![id], |row| {
                        Message::try_from(row)
                    }).optional()?)
            })
            .await
            .map_err(|e| SqliteError::DatabaseError(Box::new(e)));

        match &result {
            Ok(Some(_)) => debug!(message_id = %id, "Message retrieved successfully"),
            Ok(None) => debug!(message_id = %id, "Message not found"),
            Err(e) => error!(message_id = %id, error = ?e, "Failed to retrieve message"),
        }

        result
    }

    pub async fn get_recent_messages(
        &self,
        channel_id: i64,
        limit: usize,
    ) -> Result<Vec<Message>, SqliteError> {
        debug!(channel_id = %channel_id, limit = %limit, "Fetching recent messages");
        let result = self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, source, source_id, channel_type, channel_id, account_id, role, content, created_at 
                     FROM messages 
                     WHERE channel_id = ?1 
                     ORDER BY created_at DESC 
                     LIMIT ?2",
                )?;

                let messages = stmt.query_map([channel_id, limit as i64], |row| {
                    Message::try_from(row)
                })?.collect::<Result<Vec<_>, _>>()?;

                debug!(
                    channel_id = %channel_id,
                    message_count = %messages.len(),
                    "Retrieved recent messages"
                );

                Ok(messages)
            })
            .await
            .map_err(|e| SqliteError::DatabaseError(Box::new(e)));

        if let Err(ref e) = result {
            error!(
                channel_id = %channel_id,
                error = ?e,
                "Failed to retrieve recent messages"
            );
        }

        result
    }

    pub async fn channel_messages(
        &self,
        channel_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<(String, String)>> {
        let channel_id = channel_id.to_string();

        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT source_id, content 
                     FROM messages 
                     WHERE channel_id = ?1
                     ORDER BY created_at DESC 
                     LIMIT ?2",
                )?;
                let messages = stmt
                    .query_map([&channel_id, &limit.to_string()], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(messages)
            })
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub async fn add_documents<'a, I>(&mut self, documents: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = Document>,
    {
        info!("Adding documents to KnowledgeBase");
        let embeddings = EmbeddingsBuilder::new(self.embedding_model.clone())
            .documents(documents)?
            .build()
            .await?;

        debug!("Adding embeddings to document store");
        self.document_store.add_rows(embeddings).await?;

        info!("Successfully added documents to KnowledgeBase");
        Ok(())
    }

    pub async fn store_trade_recommendation(
        &self,
        wallet_address: &str,
        action: TradeAction,
        token_address: &str,
        amount: f64,
        reason: &str,
        signature: &str,
    ) -> Result<i64, SqliteError> {
        let wallet = wallet_address.to_string();
        let token = token_address.to_string();
        let action_str = action.as_str().to_string();
        let reason_text = reason.to_string();
        let signature_str = signature.to_string();
        self.conn
            .call(move |conn| {
                conn.query_row(
                    "INSERT INTO trade 
                     (wallet_address, action, token_address, amount, reason, created_at, signature)
                     VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP, ?6)
                     RETURNING id",
                    rusqlite::params![wallet, action_str, token, amount, reason_text, signature_str],
                    |row| row.get(0),
                )
                .map_err(tokio_rusqlite::Error::from)
            })
            .await
            .map_err(|e| SqliteError::DatabaseError(Box::new(e)))
    }

    pub async fn get_recent_trades(
        &self,
        wallet_address: &str,
        limit: i64,
    ) -> Result<Vec<Trade>, SqliteError> {
        let wallet = wallet_address.to_string();

        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, wallet_address, action, token_address, amount, reason, created_at, signature 
                     FROM trade 
                     WHERE wallet_address = ?1 
                     ORDER BY created_at DESC 
                     LIMIT ?2"
                )?;

                let trades = stmt
                    .query_map([wallet, limit.to_string()], |row| {
                        Ok(Trade {
                            id: row.get(0)?,
                            wallet_address: row.get(1)?,
                            action: TradeAction::from_str(&row.get::<_, String>(2)?)
                                .unwrap_or(TradeAction::Hold),
                            token_address: row.get(3)?,
                            amount: row.get(4)?,
                            reason: row.get(5)?,
                            created_at: row.get(6)?,
                            signature: row.get(7)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(trades)
            })
            .await
            .map_err(|e| SqliteError::DatabaseError(Box::new(e)))
    }
}