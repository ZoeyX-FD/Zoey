use rig::completion::{CompletionModel, AssistantContent};
use rig::message::Text;
use tracing::debug;

use crate::knowledge::{ChannelType, Source};
use std::collections::HashSet;

const RESPOND_COMMAND: &str = "[RESPOND]";
const IGNORE_COMMAND: &str = "[IGNORE]";
const STOP_COMMAND: &str = "[STOP]";

#[derive(Debug, PartialEq)]
pub enum AttentionCommand {
    Respond,
    Ignore,
    Stop,
}

#[derive(Debug)]
pub struct AttentionContext {
    pub message_content: String,
    pub mentioned_names: HashSet<String>,
    pub history: Vec<(String, String)>,
    pub channel_type: ChannelType,
    pub source: Source,
}

#[derive(Clone, Debug)]
pub struct AttentionConfig {
    pub bot_names: Vec<String>,
    pub reply_threshold: f32,
    pub max_history_messages: i64,
    pub cooldown_messages: i64,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            bot_names: vec!["AgentJoeyX".to_string(), "joey".to_string() , "@".to_string()],
            reply_threshold: 0.6,
            max_history_messages: 10,
            cooldown_messages: 3,
        }
    }
}

#[derive(Clone)]
pub struct Attention<M: CompletionModel> {
    config: AttentionConfig,
    completion_model: M,
}

impl<M: CompletionModel> Attention<M> {
    pub fn new(config: AttentionConfig, completion_model: M) -> Self {
        Self {
            config,
            completion_model,
        }
    }

    pub async fn should_reply(&self, context: &AttentionContext) -> AttentionCommand {
        let content = context.message_content.to_lowercase();

        // Always reply to DMs
        if context.channel_type == ChannelType::DirectMessage {
            return AttentionCommand::Respond;
        }

        // Check for mentions or name references
        for name in &self.config.bot_names {
            let mentioned = context.mentioned_names.contains(name);
            let name_in_content = content.contains(&name.to_lowercase());

            debug!(
                name = name,
                mentioned = mentioned,
                name_in_content = name_in_content,
                "Checking if bot name was mentioned"
            );

            if mentioned || name_in_content {
                debug!("Bot name {} was mentioned, will reply", name);
                return AttentionCommand::Respond;
            }
        }

        // Check for stop/disengage phrases
        let stop_phrases = [
            "shut up",
            "stop",
            "please shut up",
            "shut up please",
            "dont talk",
            "silence",
            "stop talking",
            "be quiet",
            "hush",
            "wtf",
            "stfu",
            "stupid bot",
            "dumb bot",
            "stop responding",
            "can you not",
            "can you stop",
            "be quiet",
        ];

        if stop_phrases.iter().any(|phrase| content.contains(phrase)) {
            return AttentionCommand::Stop;
        }

        // Ignore very short messages
        if content.len() < 4 {
            return AttentionCommand::Ignore;
        }

        // Improve the prompt to emphasize context awareness
        let prompt = format!(
            "You are in a conversation. You should respond while maintaining context of the discussion.\n\n\
            Recent conversation:\n{}\n\n\
            Latest message: {}\n\n\
            Consider:\n\
            1. The topic being discussed\n\
            2. The tone of the conversation\n\
            3. Previous messages for context\n\
            4. If your name was mentioned casually or needs a response\n\n\
            Response options:\n\
            {RESPOND_COMMAND} - Message is relevant and needs your response\n\
            {IGNORE_COMMAND} - Message just mentions you casually or isn't seeking response\n\
            {STOP_COMMAND} - User wants you to stop\n\n\
            Choose one response option:",
            context.history.iter()
                .map(|(_, msg)| format!("- {}", msg))
                .collect::<Vec<_>>()
                .join("\n"),
            context.message_content
        );

        let builder = self.completion_model.completion_request(Text::from(prompt.to_string()));

        match self.completion_model.completion(builder.build()).await {
            Ok(response) => {
                let text = match response.choice.first() {
                    AssistantContent::Text(text) => text.text.clone(),
                    _ => String::new()
                };
                let text = text.trim();
                if text.contains(RESPOND_COMMAND) {
                    AttentionCommand::Respond
                } else if text.contains(STOP_COMMAND) {
                    AttentionCommand::Stop
                } else {
                    AttentionCommand::Ignore
                }
            },
            Err(_) => AttentionCommand::Ignore,
        }
    }

    pub async fn should_like(&self, tweet_content: &str) -> bool {
        let prompt = format!(
            "You are deciding whether to like a tweet. Consider if the content is positive, interesting, or relevant.\n\n\
            Tweet: {}\n\n\
            Respond with only 'true' or 'false':",
            tweet_content
        );

        let builder = self.completion_model.completion_request(Text::from(prompt.to_string()));

        match self.completion_model.completion(builder.build()).await {
            Ok(response) => {
                let text = match response.choice.first() {
                    AssistantContent::Text(text) => text.text.clone(),
                    _ => String::new()
                };
                text.trim().to_lowercase() == "true"
            },
            Err(_) => false,
        }
    }

    pub async fn should_retweet(&self, tweet_content: &str) -> bool {
        let prompt = format!(
            "You are deciding whether to retweet. Only retweet if the content is highly valuable, interesting, or aligns with your values.\n\n\
            Tweet: {}\n\n\
            Respond with only 'true' or 'false':",
            tweet_content
        );

        let builder = self.completion_model.completion_request(Text::from(prompt.to_string()));

        match self.completion_model.completion(builder.build()).await {
            Ok(response) => {
                let text = match response.choice.first() {
                    AssistantContent::Text(text) => text.text.clone(),
                    _ => String::new()
                };
                text.trim().to_lowercase() == "true"
            },
            Err(_) => false,
        }
    }

    pub async fn should_quote(&self, tweet_content: &str) -> bool {
        let prompt = format!(
            "You are deciding whether to quote tweet. Quote tweet if the content deserves commentary, \
            could benefit from additional context, or warrants a thoughtful response.\n\n\
            Tweet: {}\n\n\
            Respond with only 'true' or 'false':",
            tweet_content
        );

        let builder = self.completion_model.completion_request(Text::from(prompt.to_string()));

        match self.completion_model.completion(builder.build()).await {
            Ok(response) => {
                let text = match response.choice.first() {
                    AssistantContent::Text(text) => text.text.clone(),
                    _ => String::new()
                };
                text.trim().to_lowercase() == "true"
            },
            Err(_) => false,
        }
    }
}
