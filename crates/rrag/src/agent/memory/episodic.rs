//! Episodic memory - summarized conversation history
//!
//! Episodic memory stores summarized versions of past conversations and important
//! events. It's agent-scoped and provides long-term context without storing full
//! conversation transcripts.

use crate::error::RragResult;
use crate::storage::{Memory, MemoryValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
        let value = serde_json::to_value(&episode)
            .map_err(|e| crate::error::RragError::storage("serialize_episode", std::io::Error::new(std::io::ErrorKind::Other, e)))?;

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
                let episode = serde_json::from_value(json.clone())
                    .map_err(|e| crate::error::RragError::storage("deserialize_episode", std::io::Error::new(std::io::ErrorKind::Other, e)))?;
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
            .store_episode(
                Episode::new("Discussed Rust").with_topics(vec!["rust".to_string()])
            )
            .await
            .unwrap();
        episodic
            .store_episode(
                Episode::new("Talked about Python").with_topics(vec!["python".to_string()])
            )
            .await
            .unwrap();
        episodic
            .store_episode(
                Episode::new("Rust performance").with_topics(vec!["rust".to_string(), "performance".to_string()])
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
