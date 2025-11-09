//! Conversation memory storage with persistence

use crate::error::{RragError, RragResult};
use crate::storage::{Memory, MemoryValue};
use rexis_llm::{ChatMessage, MessageRole}; // Use re-exported rsllm types
use uuid::Uuid;

/// Conversation memory backed by persistent storage
pub struct ConversationMemoryStore {
    /// Storage backend
    storage: std::sync::Arc<dyn Memory>,

    /// Session namespace (session::{session_id}::conversation)
    namespace: String,

    /// Maximum number of messages to keep
    max_length: usize,

    /// Whether to persist messages
    persist: bool,
}

impl ConversationMemoryStore {
    /// Create a new conversation memory store
    pub fn new(
        storage: std::sync::Arc<dyn Memory>,
        session_id: String,
        max_length: usize,
        persist: bool,
    ) -> Self {
        let namespace = format!("session::{}::conversation", session_id);

        Self {
            storage,
            namespace,
            max_length,
            persist,
        }
    }

    /// Add a message to conversation history
    pub async fn add_message(&self, message: ChatMessage) -> RragResult<()> {
        if !self.persist {
            // TODO: Keep in-memory cache for non-persistent mode
            return Ok(());
        }

        // Get current message count
        let count = self.count().await?;

        // Store message
        let key = self.message_key(count);
        let value = self.message_to_value(&message)?;

        self.storage.set(&key, value).await?;

        // Prune if exceeded max length
        if count + 1 > self.max_length {
            self.prune_old_messages().await?;
        }

        Ok(())
    }

    /// Get all messages in order
    pub async fn get_messages(&self) -> RragResult<Vec<ChatMessage>> {
        if !self.persist {
            // TODO: Return in-memory cache
            return Ok(Vec::new());
        }

        let count = self.count().await?;
        let mut messages = Vec::with_capacity(count);

        for idx in 0..count {
            let key = self.message_key(idx);
            if let Some(value) = self.storage.get(&key).await? {
                let message = self.value_to_message(&value)?;
                messages.push(message);
            }
        }

        Ok(messages)
    }

    /// Get the number of messages
    pub async fn count(&self) -> RragResult<usize> {
        if !self.persist {
            return Ok(0);
        }

        let count_key = format!("{}::count", self.namespace);
        if let Some(value) = self.storage.get(&count_key).await? {
            if let Some(count) = value.as_integer() {
                return Ok(count as usize);
            }
        }

        Ok(0)
    }

    /// Clear all messages except system message
    pub async fn clear(&self) -> RragResult<()> {
        if !self.persist {
            return Ok(());
        }

        // Get system message if it exists
        let system_msg = if self.count().await? > 0 {
            let key = self.message_key(0);
            if let Some(value) = self.storage.get(&key).await? {
                let msg = self.value_to_message(&value)?;
                if matches!(msg.role, MessageRole::System) {
                    Some(msg)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Clear all messages
        self.storage.clear(Some(&self.namespace)).await?;

        // Restore system message if it existed
        if let Some(msg) = system_msg {
            self.add_message(msg).await?;
        }

        Ok(())
    }

    /// Generate message key
    fn message_key(&self, index: usize) -> String {
        format!("{}::msg_{}", self.namespace, index)
    }

    /// Convert ChatMessage to MemoryValue
    fn message_to_value(&self, message: &ChatMessage) -> RragResult<MemoryValue> {
        let json = serde_json::to_value(message).map_err(|e| {
            RragError::storage(
                "serialize_message",
                std::io::Error::new(std::io::ErrorKind::Other, e),
            )
        })?;

        Ok(MemoryValue::Json(json))
    }

    /// Convert MemoryValue to ChatMessage
    fn value_to_message(&self, value: &MemoryValue) -> RragResult<ChatMessage> {
        if let Some(json) = value.as_json() {
            let message = serde_json::from_value(json.clone()).map_err(|e| {
                RragError::storage(
                    "deserialize_message",
                    std::io::Error::new(std::io::ErrorKind::Other, e),
                )
            })?;

            Ok(message)
        } else {
            Err(RragError::storage(
                "invalid_message_type",
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Expected JSON value"),
            ))
        }
    }

    /// Prune old messages to maintain max_length
    async fn prune_old_messages(&self) -> RragResult<()> {
        let count = self.count().await?;

        if count <= self.max_length {
            return Ok(());
        }

        // Keep system message (index 0) if it exists
        let has_system = if let Some(value) = self.storage.get(&self.message_key(0)).await? {
            let msg = self.value_to_message(&value)?;
            matches!(msg.role, MessageRole::System)
        } else {
            false
        };

        let start_idx = if has_system { 1 } else { 0 };
        let to_remove = count - self.max_length;

        // Delete old messages
        let mut keys_to_delete = Vec::new();
        for idx in start_idx..(start_idx + to_remove) {
            keys_to_delete.push(self.message_key(idx));
        }

        self.storage.mdelete(&keys_to_delete).await?;

        // Shift remaining messages down
        for idx in (start_idx + to_remove)..count {
            let old_key = self.message_key(idx);
            let new_key = self.message_key(idx - to_remove);

            if let Some(value) = self.storage.get(&old_key).await? {
                self.storage.set(&new_key, value).await?;
                self.storage.delete(&old_key).await?;
            }
        }

        // Update count
        let count_key = format!("{}::count", self.namespace);
        self.storage
            .set(&count_key, MemoryValue::Integer((count - to_remove) as i64))
            .await?;

        Ok(())
    }

    /// Check if conversation is empty
    pub async fn is_empty(&self) -> RragResult<bool> {
        Ok(self.count().await? == 0)
    }
}

/// Generate a unique session ID
pub fn generate_session_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryStorage;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_conversation_memory_store() {
        let storage = Arc::new(InMemoryStorage::new());
        let session_id = generate_session_id();
        let store = ConversationMemoryStore::new(storage, session_id, 10, true);

        // Add messages
        store
            .add_message(ChatMessage::system("You are a helpful assistant"))
            .await
            .unwrap();
        store.add_message(ChatMessage::user("Hello")).await.unwrap();
        store
            .add_message(ChatMessage::assistant("Hi there!"))
            .await
            .unwrap();

        // Get messages
        let messages = store.get_messages().await.unwrap();
        assert_eq!(messages.len(), 3);

        // Check count
        assert_eq!(store.count().await.unwrap(), 3);

        // Clear
        store.clear().await.unwrap();

        // System message should remain
        let messages = store.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
        assert!(matches!(messages[0].role, rexis_llm::MessageRole::System));
    }
}
