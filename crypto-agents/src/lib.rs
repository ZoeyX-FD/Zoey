pub mod agents;
pub mod api;
pub mod models;
pub mod system;

// Re-export main components
pub use agents::{
    Agent,
    TechnicalAgent,
    FundamentalAgent,
    TokenExtractor,
    NewTopAgent,
    ModelProvider,
    SentimentAgent,
    SynopsisAgent,
};
pub use api::coingecko::CoinGeckoClient;
pub use models::{MarketData, GlobalData, CoinData, TrendingCoin};
pub use system::MultiAgentSystem; 