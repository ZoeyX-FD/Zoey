mod error;
mod types;
mod client;

pub use error::ExaError;
pub use types::{ExaSearchResult, ExaSearchParams, ExaResponse, Contents, Highlights, Summary};
pub use client::ExaClient;