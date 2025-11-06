//! Agent memory manager - coordinates all memory types

use super::config::MemoryConfig;
use super::conversation::{generate_session_id, ConversationMemoryStore};
use crate::error::RragResult;
use crate::storage::Memory;
use crate::rsllm::ChatMessage; // Use re-exported rsllm type
use std::sync::Arc;

/// Manages all memory types for an agent
pub struct AgentMemoryManager {
    /// Storage backend
    storage: Arc<dyn Memory>,

    /// Agent identifier
    agent_id: String,

    /// Current session identifier
    session_id: String,

    /// Conversation memory
    conversation: ConversationMemoryStore,

    /// Configuration
    config: MemoryConfig,
}

impl AgentMemoryManager {
    /// Create a new agent memory manager
    pub fn new(mut config: MemoryConfig) -> Self {
        // Auto-generate session ID if needed
        if config.session_id.is_none() && config.auto_generate_session_id {
            config.session_id = Some(generate_session_id());
        }

        let session_id = config
            .session_id
            .clone()
            .unwrap_or_else(|| "default".to_string());

        let conversation = ConversationMemoryStore::new(
            config.backend.clone(),
            session_id.clone(),
            config.max_conversation_length,
            config.persist_conversations,
        );

        Self {
            storage: config.backend.clone(),
            agent_id: config.agent_id.clone(),
            session_id,
            conversation,
            config,
        }
    }

    /// Get agent ID
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get conversation memory
    pub fn conversation(&self) -> &ConversationMemoryStore {
        &self.conversation
    }

    /// Add a message to conversation history
    pub async fn add_conversation_message(&self, message: ChatMessage) -> RragResult<()> {
        self.conversation.add_message(message).await
    }

    /// Get all conversation messages
    pub async fn get_conversation_messages(&self) -> RragResult<Vec<ChatMessage>> {
        self.conversation.get_messages().await
    }

    /// Clear conversation (keeps system message)
    pub async fn clear_conversation(&self) -> RragResult<()> {
        self.conversation.clear().await
    }

    /// Get the underlying storage backend
    pub fn storage(&self) -> Arc<dyn Memory> {
        self.storage.clone()
    }

    /// Generate a namespace key for agent-scoped memory
    pub fn agent_key(&self, key: &str) -> String {
        format!("agent::{}::{}", self.agent_id, key)
    }

    /// Generate a namespace key for session-scoped memory
    pub fn session_key(&self, key: &str) -> String {
        format!("session::{}::{}", self.session_id, key)
    }

    /// Generate a namespace key for global memory
    pub fn global_key(key: &str) -> String {
        format!("global::{}", key)
    }

    /// Store a value in agent-scoped memory
    pub async fn set_agent_memory(&self, key: &str, value: impl Into<crate::storage::MemoryValue>) -> RragResult<()> {
        let full_key = self.agent_key(key);
        self.storage.set(&full_key, value.into()).await
    }

    /// Get a value from agent-scoped memory
    pub async fn get_agent_memory(&self, key: &str) -> RragResult<Option<crate::storage::MemoryValue>> {
        let full_key = self.agent_key(key);
        self.storage.get(&full_key).await
    }

    /// Store a value in session-scoped memory
    pub async fn set_session_memory(&self, key: &str, value: impl Into<crate::storage::MemoryValue>) -> RragResult<()> {
        let full_key = self.session_key(key);
        self.storage.set(&full_key, value.into()).await
    }

    /// Get a value from session-scoped memory
    pub async fn get_session_memory(&self, key: &str) -> RragResult<Option<crate::storage::MemoryValue>> {
        let full_key = self.session_key(key);
        self.storage.get(&full_key).await
    }

    /// Store a value in global memory
    pub async fn set_global_memory(&self, key: &str, value: impl Into<crate::storage::MemoryValue>) -> RragResult<()> {
        let full_key = Self::global_key(key);
        self.storage.set(&full_key, value.into()).await
    }

    /// Get a value from global memory
    pub async fn get_global_memory(&self, key: &str) -> RragResult<Option<crate::storage::MemoryValue>> {
        let full_key = Self::global_key(key);
        self.storage.get(&full_key).await
    }

    /// Get configuration
    pub fn config(&self) -> &MemoryConfig {
        &self.config
    }
}

impl Clone for AgentMemoryManager {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            agent_id: self.agent_id.clone(),
            session_id: self.session_id.clone(),
            conversation: ConversationMemoryStore::new(
                self.storage.clone(),
                self.session_id.clone(),
                self.config.max_conversation_length,
                self.config.persist_conversations,
            ),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{InMemoryStorage, MemoryValue};

    #[tokio::test]
    async fn test_memory_manager_namespacing() {
        let storage = Arc::new(InMemoryStorage::new());
        let config = MemoryConfig::new(storage, "test-agent")
            .with_session_id("test-session");

        let manager = AgentMemoryManager::new(config);

        // Test namespacing
        assert_eq!(
            manager.agent_key("preferences"),
            "agent::test-agent::preferences"
        );
        assert_eq!(
            manager.session_key("working_data"),
            "session::test-session::working_data"
        );
        assert_eq!(
            AgentMemoryManager::global_key("shared_config"),
            "global::shared_config"
        );
    }

    #[tokio::test]
    async fn test_memory_manager_scoped_storage() {
        let storage = Arc::new(InMemoryStorage::new());
        let config = MemoryConfig::new(storage.clone(), "test-agent");

        let manager = AgentMemoryManager::new(config);

        // Store in different scopes
        manager
            .set_agent_memory("profile::name", MemoryValue::from("Alice"))
            .await
            .unwrap();

        manager
            .set_session_memory("temp::data", MemoryValue::from(42i64))
            .await
            .unwrap();

        manager
            .set_global_memory("config::setting", MemoryValue::from(true))
            .await
            .unwrap();

        // Retrieve from different scopes
        let name = manager.get_agent_memory("profile::name").await.unwrap();
        assert_eq!(name.unwrap().as_string(), Some("Alice"));

        let data = manager.get_session_memory("temp::data").await.unwrap();
        assert_eq!(data.unwrap().as_integer(), Some(42));

        let setting = manager.get_global_memory("config::setting").await.unwrap();
        assert_eq!(setting.unwrap().as_boolean(), Some(true));
    }

    #[tokio::test]
    async fn test_conversation_integration() {
        let storage = Arc::new(InMemoryStorage::new());
        let config = MemoryConfig::new(storage, "test-agent")
            .with_persistence(true);

        let manager = AgentMemoryManager::new(config);

        // Add messages
        manager
            .add_conversation_message(ChatMessage::system("System prompt"))
            .await
            .unwrap();

        manager
            .add_conversation_message(ChatMessage::user("Hello"))
            .await
            .unwrap();

        // Get messages
        let messages = manager.get_conversation_messages().await.unwrap();
        assert_eq!(messages.len(), 2);
    }
}
