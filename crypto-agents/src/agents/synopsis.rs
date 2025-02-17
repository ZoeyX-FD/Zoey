use async_trait::async_trait;
use anyhow::Result;
use std::path::PathBuf;

use crate::models::MarketData;
use super::{
    Agent, BaseAgent, ModelProvider,
    TechnicalAgent, FundamentalAgent, TopicAgent, SentimentAgent
};

const SYNOPSIS_SYSTEM_PROMPT: &str = r#"
You are the Round Synopsis Agent 📊
Your role is to facilitate discussion between agents and create clear consensus summaries.

Guidelines:
- Use existing analyses from other agents
- Guide discussion between agents about their findings
- Find common ground and points of agreement
- Highlight key disagreements that need resolution
- Create actionable consensus summaries

Format your response like this:

📊 Discussion Summary:
[Overview of agent perspectives]

🤝 Points of Agreement:
- [List key points where agents agree]

⚖️ Different Viewpoints:
- [List areas where agents have different perspectives]

📋 Consensus:
- [Final agreed conclusions]
- [Actionable insights]

⚠️ Key Risks:
- [Shared risk concerns]

🎯 Recommended Actions:
[Clear next steps based on consensus]
"#;

#[derive(Debug)]
pub struct AgentAnalysis {
    agent_name: String,
    analysis: String,
}

pub struct SynopsisAgent {
    base: BaseAgent,
    technical_agent: TechnicalAgent,
    fundamental_agent: FundamentalAgent,
    topic_agent: TopicAgent,
    sentiment_agent: SentimentAgent,
}

impl SynopsisAgent {
    pub async fn new(model: String, provider: ModelProvider) -> Result<Self> {
        Ok(Self {
            base: BaseAgent::new(
                "Synopsis Agent".to_string(),
                model.clone(),
                SYNOPSIS_SYSTEM_PROMPT.to_string(),
                provider.clone()
            ).await?.with_temperature(0.3),
            technical_agent: TechnicalAgent::new(model.clone(), provider.clone()).await?,
            fundamental_agent: FundamentalAgent::new(model.clone(), provider.clone()).await?,
            topic_agent: TopicAgent::new(model.clone(), provider.clone()).await?,
            sentiment_agent: SentimentAgent::new(model.clone(), provider).await?,
        })
    }

    pub async fn generate_synopsis(
        &mut self,
        agent_one_response: &str,
        agent_two_response: &str,
        sentiment_response: Option<&str>,
        topic_analysis: Option<&str>,
    ) -> Result<String> {
        let mut analyses = Vec::new();
        
        // Add technical analysis
        analyses.push(AgentAnalysis {
            agent_name: "Technical".to_string(),
            analysis: agent_one_response.to_string(),
        });

        // Add fundamental analysis
        analyses.push(AgentAnalysis {
            agent_name: "Fundamental".to_string(),
            analysis: agent_two_response.to_string(),
        });

        // Add sentiment analysis if available
        if let Some(sentiment) = sentiment_response {
            analyses.push(AgentAnalysis {
                agent_name: "Sentiment".to_string(),
                analysis: sentiment.to_string(),
            });
        }

        // Add topic analysis if available
        if let Some(topic) = topic_analysis {
            analyses.push(AgentAnalysis {
                agent_name: "Topic".to_string(),
                analysis: topic.to_string(),
            });
        }

        // Facilitate discussion and generate consensus
        let discussion = self.facilitate_discussion(analyses).await?;
        self.generate_consensus(&discussion).await
    }

    // Facilitate discussion between agents using their analyses
    async fn facilitate_discussion(&self, analyses: Vec<AgentAnalysis>) -> Result<String> {
        let mut discussion = String::new();
        
        // Format analyses for discussion
        for analysis in &analyses {
            discussion.push_str(&format!("\n🔹 {} Agent's Analysis:\n{}\n", 
                analysis.agent_name, analysis.analysis));
        }

        // Generate discussion synthesis
        let prompt = format!(
            "Based on these agent analyses, facilitate a discussion to find common ground and create consensus:\n{}",
            discussion
        );

        self.base.generate_response(&prompt, None).await
    }

    // Create final consensus from discussion
    async fn generate_consensus(&self, discussion: &str) -> Result<String> {
        let prompt = format!(
            "Review this agent discussion and create a final consensus summary:\n{}",
            discussion
        );

        self.base.generate_response(&prompt, None).await
    }
}

#[async_trait]
impl Agent for SynopsisAgent {
    fn name(&self) -> &str {
        "Synopsis Agent"
    }

    fn model(&self) -> &str {
        &self.base.model
    }

    async fn think(&mut self, market_data: &MarketData, _previous: Option<String>) -> Result<String> {
        // 1. Collect existing analyses from all agents using the provided market data
        let mut analyses = Vec::new();
        
        // Get latest analysis from each agent's think() method
        let technical = self.technical_agent.think(market_data, None).await?;
        analyses.push(AgentAnalysis {
            agent_name: "Technical".to_string(),
            analysis: technical,
        });
        
        let fundamental = self.fundamental_agent.think(market_data, None).await?;
        analyses.push(AgentAnalysis {
            agent_name: "Fundamental".to_string(),
            analysis: fundamental,
        });

        let sentiment = self.sentiment_agent.think(market_data, None).await?;
        analyses.push(AgentAnalysis {
            agent_name: "Sentiment".to_string(),
            analysis: sentiment,
        });

        let topic = self.topic_agent.think(market_data, None).await?;
        analyses.push(AgentAnalysis {
            agent_name: "Topic".to_string(),
            analysis: topic,
        });
        
        // 2. Facilitate discussion between agents
        let discussion = self.facilitate_discussion(analyses).await?;
        
        // 3. Generate final consensus
        self.generate_consensus(&discussion).await
    }

    async fn save_memory(&self) -> Result<()> {
        Ok(())
    }

    async fn load_memory(&mut self) -> Result<()> {
        Ok(())
    }

    fn memory_file(&self) -> PathBuf {
        PathBuf::from("synopsis_memory.json")
    }
} 