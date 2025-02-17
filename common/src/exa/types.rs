use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Deserialize)]
pub struct ExaResponse {
    pub results: Vec<ExaSearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct ExaSearchResult {
    pub title: String,
    pub url: String,
    pub text: Option<String>,
    #[serde(default)]
    pub highlights: Vec<String>,
    #[serde(default)]
    pub highlight_scores: Vec<f64>,
    pub summary: Option<String>,
    pub published_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub relevance_score: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ExaSearchParams {
    pub query: String,
    pub num_results: i32,
    pub include_domains: Vec<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contents: Option<Contents>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Contents {
    pub text: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlights: Option<Highlights>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<Summary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Highlights {
    pub num_sentences: i32,
    pub highlights_per_result: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub max_sentences: i32,
}
