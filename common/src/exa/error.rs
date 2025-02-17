#[derive(Debug, thiserror::Error)]
pub enum ExaError {
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Rate limit exceeded")]
    RateLimit,
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
}
