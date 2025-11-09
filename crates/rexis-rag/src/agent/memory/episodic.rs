//! Episodic memory - summarized conversation history
//!
//! Episodic memory stores summarized versions of past conversations and important
//! events. It's agent-scoped and provides long-term context without storing full
//! conversation transcripts.

use crate::error::RragResult;
use crate::storage::{Memory, MemoryValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[cfg(feature = "rexis-llm-client")]
use rexis_llm::{ChatMessage, Client, MessageRole};

/// An episode (summarized interaction or event)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    /// Unique identifier
    pub id: String,

    /// When this episode occurred
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Summary of the interaction/event
    pub summary: String,

    /// Key topics discussed
    pub topics: Vec<String>,

    /// Importance score (0.0 to 1.0)
    pub importance: f64,

    /// Optional session ID this episode is from
    pub session_id: Option<String>,

    /// Extracted facts or insights
    pub insights: Vec<String>,

    /// Optional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl Episode {
    /// Create a new episode
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            summary: summary.into(),
            topics: Vec::new(),
            importance: 0.5,
            session_id: None,
            insights: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Set topics
    pub fn with_topics(mut self, topics: Vec<String>) -> Self {
        self.topics = topics;
        self
    }

    /// Set importance
    pub fn with_importance(mut self, importance: f64) -> Self {
        self.importance = importance.clamp(0.0, 1.0);
        self
    }

    /// Set session ID
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add insights
    pub fn with_insights(mut self, insights: Vec<String>) -> Self {
        self.insights = insights;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Episodic memory for long-term context
pub struct EpisodicMemory {
    /// Storage backend
    storage: Arc<dyn Memory>,

    /// Namespace for this episodic memory (agent::{agent_id}::episodic)
    namespace: String,

    /// Maximum number of episodes to retain
    max_episodes: usize,
}

impl EpisodicMemory {
    /// Create a new episodic memory
    pub fn new(storage: Arc<dyn Memory>, agent_id: String) -> Self {
        let namespace = format!("agent::{}::episodic", agent_id);

        Self {
            storage,
            namespace,
            max_episodes: 1000,
        }
    }

    /// Create episodic memory with custom max episodes
    pub fn with_max_episodes(mut self, max: usize) -> Self {
        self.max_episodes = max;
        self
    }

    /// Store an episode
    pub async fn store_episode(&self, episode: Episode) -> RragResult<()> {
        let key = self.episode_key(&episode.id);
        let value = serde_json::to_value(&episode).map_err(|e| {
            crate::error::RragError::storage(
                "serialize_episode",
                std::io::Error::new(std::io::ErrorKind::Other, e),
            )
        })?;

        self.storage.set(&key, MemoryValue::Json(value)).await?;

        // Prune old episodes if exceeded max
        self.prune_if_needed().await?;

        Ok(())
    }

    /// Retrieve an episode by ID
    pub async fn get_episode(&self, episode_id: &str) -> RragResult<Option<Episode>> {
        let key = self.episode_key(episode_id);
        if let Some(value) = self.storage.get(&key).await? {
            if let Some(json) = value.as_json() {
                let episode = serde_json::from_value(json.clone()).map_err(|e| {
                    crate::error::RragError::storage(
                        "deserialize_episode",
                        std::io::Error::new(std::io::ErrorKind::Other, e),
                    )
                })?;
                return Ok(Some(episode));
            }
        }
        Ok(None)
    }

    /// Get recent episodes
    pub async fn get_recent_episodes(&self, limit: usize) -> RragResult<Vec<Episode>> {
        let mut all_episodes = self.get_all_episodes().await?;

        // Sort by timestamp descending
        all_episodes.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Take limit
        all_episodes.truncate(limit);

        Ok(all_episodes)
    }

    /// Find episodes by topic
    pub async fn find_by_topic(&self, topic: &str) -> RragResult<Vec<Episode>> {
        let all_episodes = self.get_all_episodes().await?;

        let matching = all_episodes
            .into_iter()
            .filter(|e| e.topics.iter().any(|t| t.contains(topic)))
            .collect();

        Ok(matching)
    }

    /// Find episodes by importance threshold
    pub async fn find_by_importance(&self, min_importance: f64) -> RragResult<Vec<Episode>> {
        let all_episodes = self.get_all_episodes().await?;

        let important = all_episodes
            .into_iter()
            .filter(|e| e.importance >= min_importance)
            .collect();

        Ok(important)
    }

    /// Find episodes within a date range
    pub async fn find_by_date_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> RragResult<Vec<Episode>> {
        let all_episodes = self.get_all_episodes().await?;

        let in_range = all_episodes
            .into_iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .collect();

        Ok(in_range)
    }

    /// Get all episodes
    pub async fn get_all_episodes(&self) -> RragResult<Vec<Episode>> {
        let all_keys = self.list_episode_keys().await?;
        let mut episodes = Vec::new();

        for key in all_keys {
            if let Some(episode) = self.get_episode(&key).await? {
                episodes.push(episode);
            }
        }

        Ok(episodes)
    }

    /// Delete an episode
    pub async fn delete_episode(&self, episode_id: &str) -> RragResult<bool> {
        let key = self.episode_key(episode_id);
        self.storage.delete(&key).await
    }

    /// Count episodes
    pub async fn count(&self) -> RragResult<usize> {
        self.storage.count(Some(&self.namespace)).await
    }

    /// Clear all episodes
    pub async fn clear(&self) -> RragResult<()> {
        self.storage.clear(Some(&self.namespace)).await
    }

    /// Generate a summary from recent episodes
    pub async fn generate_context_summary(&self, num_episodes: usize) -> RragResult<String> {
        let recent = self.get_recent_episodes(num_episodes).await?;

        if recent.is_empty() {
            return Ok(String::new());
        }

        let mut summary = String::from("Recent interaction history:\n");

        for episode in recent.iter() {
            summary.push_str(&format!(
                "- [{}] {}\n",
                episode.timestamp.format("%Y-%m-%d"),
                episode.summary
            ));

            if !episode.topics.is_empty() {
                summary.push_str(&format!("  Topics: {}\n", episode.topics.join(", ")));
            }
        }

        Ok(summary)
    }

    /// Prune old episodes if exceeded max_episodes
    async fn prune_if_needed(&self) -> RragResult<()> {
        let count = self.count().await?;

        if count <= self.max_episodes {
            return Ok(());
        }

        // Get all episodes and sort by importance and timestamp
        let mut all_episodes = self.get_all_episodes().await?;

        // Sort by importance (ascending) then timestamp (oldest first)
        all_episodes.sort_by(|a, b| {
            a.importance
                .partial_cmp(&b.importance)
                .unwrap()
                .then(a.timestamp.cmp(&b.timestamp))
        });

        // Delete least important/oldest episodes
        let to_delete = count - self.max_episodes;
        for episode in all_episodes.iter().take(to_delete) {
            self.delete_episode(&episode.id).await?;
        }

        Ok(())
    }

    /// Generate episode key
    fn episode_key(&self, episode_id: &str) -> String {
        format!("{}::episode::{}", self.namespace, episode_id)
    }

    /// List all episode keys (IDs)
    async fn list_episode_keys(&self) -> RragResult<Vec<String>> {
        use crate::storage::MemoryQuery;

        let query = MemoryQuery::new().with_namespace(self.namespace.clone());
        let all_keys = self.storage.keys(&query).await?;

        // Extract episode IDs from keys
        let prefix = format!("{}::episode::", self.namespace);
        let ids = all_keys
            .into_iter()
            .filter_map(|k| k.strip_prefix(&prefix).map(String::from))
            .collect();

        Ok(ids)
    }

    /// Create an episode from conversation messages using LLM summarization (requires 'rsllm-client' feature)
    #[cfg(feature = "rexis-llm-client")]
    pub async fn create_episode_from_messages(
        &self,
        messages: &[ChatMessage],
        llm_client: &Client,
    ) -> RragResult<Episode> {
        if messages.is_empty() {
            return Err(crate::error::RragError::validation(
                "messages",
                "must not be empty",
                "0 messages provided".to_string(),
            ));
        }

        // Build conversation text
        let mut conversation = String::new();
        for msg in messages {
            let content_text = match &msg.content {
                rexis_llm::MessageContent::Text(text) => text.clone(),
                rexis_llm::MessageContent::MultiModal { text, .. } => {
                    text.clone().unwrap_or_default()
                }
            };

            conversation.push_str(&format!(
                "{}: {}\n",
                match msg.role {
                    MessageRole::User => "User",
                    MessageRole::Assistant => "Assistant",
                    MessageRole::System => "System",
                    MessageRole::Tool => "Tool",
                },
                content_text
            ));
        }

        // Create summarization prompt
        let summary_prompt = format!(
            "Summarize this conversation in 2-3 sentences, focusing on key topics and outcomes:\n\n{}",
            conversation
        );

        let summary_msg = ChatMessage::user(summary_prompt);

        // Generate summary using LLM
        let response = llm_client
            .chat_completion(vec![summary_msg])
            .await
            .map_err(|e| crate::error::RragError::rsllm_client("summarization", e))?;

        let summary = response.content.trim().to_string();

        // Extract topics (simple keyword extraction from summary)
        let topics = self.extract_topics_from_text(&summary);

        // Calculate importance (based on message count and engagement)
        let importance = self.calculate_importance(messages.len(), &conversation);

        let episode = Episode::new(summary)
            .with_topics(topics)
            .with_importance(importance);

        Ok(episode)
    }

    /// Generate a comprehensive summary of recent episodes using LLM (requires 'rsllm-client' feature)
    #[cfg(feature = "rexis-llm-client")]
    pub async fn generate_llm_summary(
        &self,
        num_episodes: usize,
        llm_client: &Client,
    ) -> RragResult<String> {
        let recent = self.get_recent_episodes(num_episodes).await?;

        if recent.is_empty() {
            return Ok(String::from("No recent episodes to summarize."));
        }

        // Build context from episodes
        let mut episode_text = String::new();
        for (i, episode) in recent.iter().enumerate() {
            episode_text.push_str(&format!(
                "{}. [{}] {}\n",
                i + 1,
                episode.timestamp.format("%Y-%m-%d"),
                episode.summary
            ));
        }

        // Create summarization prompt
        let summary_prompt = format!(
            "Provide a coherent summary of these conversation episodes, highlighting key themes and progression:\n\n{}",
            episode_text
        );

        let msg = ChatMessage::user(summary_prompt);

        // Generate comprehensive summary
        let response = llm_client
            .chat_completion(vec![msg])
            .await
            .map_err(|e| crate::error::RragError::rsllm_client("episode_summary", e))?;

        Ok(response.content.trim().to_string())
    }

    /// Extract insights from an episode using LLM analysis (requires 'rsllm-client' feature)
    #[cfg(feature = "rexis-llm-client")]
    pub async fn extract_insights(
        &self,
        episode: &Episode,
        llm_client: &Client,
    ) -> RragResult<Vec<String>> {
        let insight_prompt = format!(
            "Extract 3-5 key insights or learnings from this conversation summary:\n\n{}",
            episode.summary
        );

        let msg = ChatMessage::user(insight_prompt);

        let response = llm_client
            .chat_completion(vec![msg])
            .await
            .map_err(|e| crate::error::RragError::rsllm_client("insight_extraction", e))?;

        // Parse insights (assuming one per line)
        let insights: Vec<String> = response
            .content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                // Remove leading numbers/bullets
                line.trim()
                    .trim_start_matches(|c: char| {
                        c.is_numeric() || c == '.' || c == '-' || c == '*'
                    })
                    .trim()
                    .to_string()
            })
            .filter(|s| !s.is_empty())
            .collect();

        Ok(insights)
    }

    /// Simple topic extraction from text (fallback when LLM not available)
    fn extract_topics_from_text(&self, text: &str) -> Vec<String> {
        // Simple keyword extraction - look for capitalized words and common programming terms
        let common_topics = [
            "rust",
            "python",
            "javascript",
            "programming",
            "coding",
            "algorithm",
            "database",
            "api",
            "frontend",
            "backend",
            "testing",
            "deployment",
            "performance",
            "security",
            "design",
            "architecture",
            "error",
            "debugging",
        ];

        let text_lower = text.to_lowercase();
        let mut topics = Vec::new();

        for topic in common_topics {
            if text_lower.contains(topic) {
                topics.push(topic.to_string());
            }
        }

        // Limit to top 5 topics
        topics.truncate(5);

        topics
    }

    /// Calculate importance score based on conversation characteristics
    fn calculate_importance(&self, message_count: usize, conversation: &str) -> f64 {
        let mut importance: f64 = 0.5; // Base importance

        // More messages = potentially more important
        if message_count > 10 {
            importance += 0.2;
        } else if message_count > 5 {
            importance += 0.1;
        }

        // Longer conversations = potentially more important
        let word_count = conversation.split_whitespace().count();
        if word_count > 500 {
            importance += 0.2;
        } else if word_count > 200 {
            importance += 0.1;
        }

        // Presence of key terms indicates importance
        let important_terms = [
            "important",
            "critical",
            "urgent",
            "key",
            "essential",
            "decision",
        ];
        let conv_lower = conversation.to_lowercase();
        for term in important_terms {
            if conv_lower.contains(term) {
                importance += 0.1;
                break;
            }
        }

        // Clamp to [0.0, 1.0]
        importance.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryStorage;

    #[tokio::test]
    async fn test_episodic_memory_store_and_retrieve() {
        let storage = Arc::new(InMemoryStorage::new());
        let episodic = EpisodicMemory::new(storage, "test-agent".to_string());

        // Store an episode
        let episode = Episode::new("User asked about Rust programming")
            .with_topics(vec!["rust".to_string(), "programming".to_string()])
            .with_importance(0.8);

        let episode_id = episode.id.clone();
        episodic.store_episode(episode).await.unwrap();

        // Retrieve it
        let retrieved = episodic.get_episode(&episode_id).await.unwrap().unwrap();
        assert_eq!(retrieved.summary, "User asked about Rust programming");
        assert_eq!(retrieved.topics.len(), 2);
        assert_eq!(retrieved.importance, 0.8);
    }

    #[tokio::test]
    async fn test_episodic_memory_recent_episodes() {
        let storage = Arc::new(InMemoryStorage::new());
        let episodic = EpisodicMemory::new(storage, "test-agent".to_string());

        // Store multiple episodes
        for i in 1..=5 {
            let episode = Episode::new(format!("Episode {}", i));
            episodic.store_episode(episode).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Get recent (should be in reverse order)
        let recent = episodic.get_recent_episodes(3).await.unwrap();
        assert_eq!(recent.len(), 3);
        assert!(recent[0].summary.contains("Episode 5"));
    }

    #[tokio::test]
    async fn test_episodic_memory_find_by_topic() {
        let storage = Arc::new(InMemoryStorage::new());
        let episodic = EpisodicMemory::new(storage, "test-agent".to_string());

        // Store episodes with different topics
        episodic
            .store_episode(Episode::new("Discussed Rust").with_topics(vec!["rust".to_string()]))
            .await
            .unwrap();
        episodic
            .store_episode(
                Episode::new("Talked about Python").with_topics(vec!["python".to_string()]),
            )
            .await
            .unwrap();
        episodic
            .store_episode(
                Episode::new("Rust performance")
                    .with_topics(vec!["rust".to_string(), "performance".to_string()]),
            )
            .await
            .unwrap();

        // Find by topic
        let rust_episodes = episodic.find_by_topic("rust").await.unwrap();
        assert_eq!(rust_episodes.len(), 2);
    }

    #[tokio::test]
    async fn test_episodic_memory_context_summary() {
        let storage = Arc::new(InMemoryStorage::new());
        let episodic = EpisodicMemory::new(storage, "test-agent".to_string());

        // Store some episodes
        episodic
            .store_episode(Episode::new("User asked about Rust"))
            .await
            .unwrap();
        episodic
            .store_episode(Episode::new("Discussed error handling"))
            .await
            .unwrap();

        // Generate summary
        let summary = episodic.generate_context_summary(5).await.unwrap();
        assert!(summary.contains("Recent interaction history"));
        assert!(summary.contains("User asked about Rust"));
    }
}
