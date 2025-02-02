use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Failed to process document: {0}")]
    DocumentError(String),
    
    #[error("Failed to communicate with AI: {0}")]
    AIError(String),
    
    #[error("Failed to communicate with external API: {0}")]
    ExternalApiError(String),
    
    #[error("Failed to parse response: {0}")]
    ParseError(String),
    
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}