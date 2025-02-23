use rig::{
    agent::AgentBuilder, 
    completion::{CompletionModel, Prompt},
    embeddings::embedding::EmbeddingModel
};
use tracing::{info, debug, error};
use crate::{character::Character, knowledge::KnowledgeBase, intel::CryptoIntel};
use rig::message::Text;
use crate::interaction_history::InteractionHistory;

#[derive(Clone)]
pub struct Agent<M: CompletionModel, E: EmbeddingModel + 'static> {
    pub character: Character,
    completion_model: M,
    knowledge: KnowledgeBase<E>,
    pub interaction_history: InteractionHistory,
}

impl<M: CompletionModel, E: EmbeddingModel> Agent<M, E> {
    pub fn new(character: Character, completion_model: M, knowledge: KnowledgeBase<E>, interaction_history: InteractionHistory) -> Self {
        info!(name = character.name, "Creating new agent");

        Self {
            character,
            completion_model,
            knowledge,
            interaction_history,
        }
    }

    pub fn builder(&self) -> AgentBuilder<M> {
        let mut builder = AgentBuilder::new(self.completion_model.clone());

        // Add performance insights to context
        if let Ok(insights) = futures::executor::block_on(self.interaction_history.generate_performance_insights()) {
            builder = builder.context(&insights);
        }

        // Build character context
        let character_context = format!(
            "Your name is: {}
            
            Your identity and expertise:
            Topics of expertise: {}
            
            Example messages for reference:
            {}",
            self.character.name,
            self.character.topics.join(", "),
            self.character.message_examples.join("\n")
        );

        // Build style context
        let style_context = format!(
            "Your personality and communication style:
            
            Core traits and behaviors:
            {}
            
            Communication style:
            - In chat: {}
            - In posts: {}
            
            Expression elements:
            - Common adjectives: {}
            
            Personal elements:
            - Key interests: {}
            - Meme-related phrases: {}",
            self.character.style.all.join("\n"),
            self.character.style.chat.join("\n"),
            self.character.style.post.join("\n"),
            self.character.style.adjectives.join(", "),
            self.character.style.interests.join("\n"),
            self.character.style.meme_phrases.join("\n")
        );

        builder
            .preamble(&self.character.preamble)
            .context(&character_context)
            .context(&style_context)
            .dynamic_context(2, self.knowledge.clone().document_index())
    }

    pub fn knowledge(&self) -> &KnowledgeBase<E> {
        &self.knowledge
    }

    pub async fn process_market_data(&self, intel: &CryptoIntel) -> Result<String, Box<dyn std::error::Error>> {
        let mut retries = 3;
        
        loop {
            info!("Processing market data for agent response (retries left: {})", retries);
            debug!("Input content: {}", intel.content);
            
            let prompt = format!(
                "You have received this market data: {}

                As a chef who loves explaining crypto through cooking metaphors:
                1. review this market data
                2. If you find it interesting or important, create a tweet about it ,Keep under 260 characters (MUST)
                3. If not interesting enough, respond with 'NO_POST'
                
                Rules for tweets:
                - Keep your chef personality
                - Include price and percentage only (no timestamp)
                - Focus on key insights and actions
                - Use cooking metaphors naturally
                - Use max 2 hashtags
                - Only use these special characters: $ % . , ! ?
                - Only use these emojis: 👨‍🍳 🔥 🌶️ 🍳 🥘 🥗 
                - No asterisks, ellipsis, or other special formatting
                - Format numbers with standard notation{}",
                intel.content,
                if retries < 3 { "\n- Make it more concise than before, keep it under 260 characters" } else { "" }
            );

            info!("Sending prompt to completion model");
            let response = self.builder()
                .context(&prompt)
                .build()
                .prompt(Text::from(intel.content.to_string()))
                .await?;
            
            info!("Received response from model: {}", response);
            
            // Clean up the response
            let cleaned = response
                .trim()
                .replace("\n\n", "\n")
                .replace("  ", " ")
                .replace("...", ".")
                .replace("*", "");
                
            if cleaned.len() <= 240 {
                return Ok(cleaned);
            }
            
            retries -= 1;
            if retries == 0 {
                error!("Failed to generate tweet under 240 characters after all retries");
                return Ok("NO_POST".to_string());
            }
            
            info!("Response too long ({}), trying again with {} retries left", cleaned.len(), retries);
        }
    }
}