use reqwest::Client;
use chrono::{Utc, Duration};
use anyhow::Result;

use super::types::{ExaSearchResult, ExaSearchParams, ExaResponse, Contents, Highlights, Summary};
use super::error::ExaError;

const EXA_API_URL: &str = "https://api.exa.ai/search";

pub struct ExaClient {
    client: Client,
    api_key: String,
}

impl ExaClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
        }
    }

    pub async fn search_crypto(&self, mut params: ExaSearchParams) -> Result<Vec<ExaSearchResult>> {
        // Add content parameters if not already set
        if params.contents.is_none() {
            params.contents = Some(Contents {
                text: true,
                highlights: Some(Highlights {
                    num_sentences: 3,
                    highlights_per_result: 3,
                }),
                summary: Some(Summary {
                    max_sentences: 5,
                }),
            });
        }

        let response = self.client
            .post(EXA_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&params)
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::OK => {
                let exa_response: ExaResponse = response.json().await?;
                Ok(exa_response.results)
            },
            reqwest::StatusCode::TOO_MANY_REQUESTS => {
                Err(ExaError::RateLimit.into())
            },
            _ => {
                let error_text = response.text().await?;
                Err(ExaError::ApiError(error_text).into())
            }
        }
    }

    pub async fn search_project_news(&self, symbol: &str) -> Result<Vec<ExaSearchResult>> {
        let params = ExaSearchParams {
            query: format!("cryptocurrency {} latest news developments", symbol),
            num_results: 5,
            include_domains: vec![
                "cointelegraph.com".to_string(),
                "coindesk.com".to_string(),
                "theblock.co".to_string(),
                "decrypt.co".to_string(),
            ],
            start_date: Some(Utc::now() - Duration::days(7)),
            end_date: None,
            contents: Some(Contents {
                text: true,
                highlights: Some(Highlights {
                    num_sentences: 3,
                    highlights_per_result: 3,
                }),
                summary: Some(Summary {
                    max_sentences: 5,
                }),
            }),
        };
        self.search_crypto(params).await
    }

    pub async fn search_market_analysis(&self, symbol: &str) -> Result<Vec<ExaSearchResult>> {
        let params = ExaSearchParams {
            query: format!("{} price analysis market prediction", symbol),
            num_results: 5,
            include_domains: vec![
                "tradingview.com".to_string(),
                "seekingalpha.com".to_string(),
                "investing.com".to_string(),
            ],
            start_date: Some(Utc::now() - Duration::days(2)),
            end_date: None,
            contents: Some(Contents {
                text: true,
                highlights: Some(Highlights {
                    num_sentences: 3,
                    highlights_per_result: 3,
                }),
                summary: Some(Summary {
                    max_sentences: 5,
                }),
            }),
        };
        self.search_crypto(params).await
    }
}