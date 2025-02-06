pub mod coingecko;
pub mod social_media;

pub use coingecko::CoinGeckoClient;
pub use social_media::SocialMediaClient;

// Re-export commonly used types
pub use coingecko::DetailedCoinData;
pub use social_media::SocialMediaPost; 