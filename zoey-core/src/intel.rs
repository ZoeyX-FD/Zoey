use serde::{Deserialize, Serialize};
use std::path::Path;
use csv::Reader;
use tokio::fs;
use chrono::DateTime;
use chrono_tz::Asia::Bangkok;
use tracing::{info, debug, error};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub(crate) static ref PROCESSED_FILES: Arc<Mutex<HashMap<String, DateTime<chrono::Utc>>>> = 
        Arc::new(Mutex::new(HashMap::new()));
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoIntel {
    pub source: IntelSource,
    pub content: String,
    pub timestamp: DateTime<chrono::Utc>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntelSource {
    Market,
    Airdrop,
    News,
    Security,
    Other(String),
}

impl CryptoIntel {
    pub fn get_file_id(path: &Path) -> Option<String> {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    }

    pub async fn from_csv(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let file_id = Self::get_file_id(path)
            .ok_or("Could not get file ID")?;

        let mut processed = PROCESSED_FILES.lock().await;
        if let Some(timestamp) = processed.get(&file_id) {
            if chrono::Utc::now().signed_duration_since(*timestamp).num_minutes() < 30 {
                return Err("File processed too recently".into());
            }
        }

        let content = fs::read_to_string(path).await?;
        let mut rdr = Reader::from_reader(content.as_bytes());
        
        let record = rdr.records().next()
            .ok_or("No records found")??;

        // Parse timestamp and convert to Bangkok time
        let utc_time = DateTime::parse_from_str(
            record.get(0).ok_or("Missing timestamp")?,
            "%Y-%m-%d %H:%M:%S %z"
        )?;
        let bkk_time = utc_time.with_timezone(&Bangkok);
        
        // Get fields directly from CSV
        let symbol = record.get(1).ok_or("Missing symbol")?;
        let price = record.get(2).ok_or("Missing price")?;
        let change = record.get(3).ok_or("Missing price change")?;
        let analysis = record.get(10).ok_or("Missing analysis")?;
        let outlook = record.get(8).ok_or("Missing outlook")?;
        let risk = record.get(9).ok_or("Missing risk")?;

        // Create a summary with Bangkok timestamp
        let summary = format!(
            "[{}] ${} at ${} ({:+}%) - {} outlook with {} risk. Key points: {}",
            bkk_time.format("%Y-%m-%d %H:%M ICT"),
            symbol,
            price,
            change,
            outlook.to_lowercase(),
            risk.to_lowercase(),
            analysis.split('\n').next().unwrap_or(analysis)
        );

        // Mark as processed with current timestamp
        processed.insert(file_id, chrono::Utc::now());

        Ok(CryptoIntel {
            source: IntelSource::Market,
            content: summary,
            timestamp: utc_time.into(),
            tags: vec![
                symbol.to_string(),
                outlook.to_string(),
                risk.to_string()
            ],
        })
    }

    pub fn get_context(&self) -> String {
        format!("Latest {} intel: {}", 
            match self.source {
                IntelSource::Market => "market",
                IntelSource::Airdrop => "airdrop",
                IntelSource::News => "news",
                IntelSource::Security => "security",
                IntelSource::Other(ref s) => s,
            },
            self.content
        )
    }
}

pub async fn scan_intel_folder(folder_path: &str) -> Result<Vec<CryptoIntel>, Box<dyn std::error::Error>> {
    info!("Scanning folder: {}", folder_path);
    let mut intel_list = Vec::new();
    
    // Check if folder exists
    if !Path::new(folder_path).exists() {
        error!("Intel folder does not exist: {}", folder_path);
        return Ok(vec![]);
    }

    let mut entries = fs::read_dir(folder_path).await?;
    debug!("Reading directory entries");

    // Collect all CSV files first
    let mut csv_files = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("csv") {
            csv_files.push(path);
        }
    }

    // Sort files by name to ensure sequential processing
    csv_files.sort();
    debug!("Found {} CSV files to process", csv_files.len());

    // Process each file in order
    for path in csv_files {
        debug!("Processing file: {}", path.display());
        
        // Check if we've already processed this file
        if let Some(file_id) = CryptoIntel::get_file_id(&path) {
            let processed = PROCESSED_FILES.lock().await;
            if processed.contains_key(&file_id) {
                debug!("Skipping already processed file: {}", file_id);
                continue;
            }
            drop(processed); // Release the lock
        }

        match CryptoIntel::from_csv(&path).await {
            Ok(intel) => {
                debug!("Successfully parsed intel from {}", path.display());
                intel_list.push(intel);
            },
            Err(e) => {
                if e.to_string().contains("too recently") {
                    debug!("File {} was processed too recently", path.display());
                } else {
                    error!("Error processing {}: {}", path.display(), e);
                }
            }
        }
    }

    // Sort by timestamp (newest first) as a secondary sort
    intel_list.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    info!("Found {} processable intel files", intel_list.len());
    debug!("Intel order: {:?}", intel_list.iter().map(|i| i.tags.first()).collect::<Vec<_>>());
    
    Ok(intel_list)
}

pub async fn cleanup_processed_files() {
    let mut processed = PROCESSED_FILES.lock().await;
    let now = chrono::Utc::now();
    processed.retain(|_, timestamp| {
        now.signed_duration_since(*timestamp).num_hours() < 1
    });
} 