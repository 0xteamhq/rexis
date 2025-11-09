//! Working memory - temporary scratchpad for agent reasoning
//!
//! Working memory provides a temporary space for agents to store intermediate
//! results, thoughts, and data during execution. It's session-scoped and typically
//! cleared when the session ends.

use crate::error::RragResult;
use crate::storage::{Memory, MemoryValue};
use std::sync::Arc;

/// Working memory for temporary agent data
pub struct WorkingMemory {
    /// Storage backend
    storage: Arc<dyn Memory>,

    /// Namespace for this working memory (session::{session_id}::working)
    namespace: String,

    /// Whether to auto-clear on drop
    auto_clear: bool,
}

impl WorkingMemory {
    /// Create a new working memory
    pub fn new(storage: Arc<dyn Memory>, session_id: String) -> Self {
        let namespace = format!("session::{}::working", session_id);

        Self {
            storage,
            namespace,
            auto_clear: true,
        }
    }

    /// Create working memory without auto-clear
    pub fn new_persistent(storage: Arc<dyn Memory>, session_id: String) -> Self {
        let namespace = format!("session::{}::working", session_id);

        Self {
            storage,
            namespace,
            auto_clear: false,
        }
    }

    /// Set a value in working memory
    pub async fn set(&self, key: &str, value: impl Into<MemoryValue>) -> RragResult<()> {
        let full_key = self.make_key(key);
        self.storage.set(&full_key, value.into()).await
    }

    /// Get a value from working memory
    pub async fn get(&self, key: &str) -> RragResult<Option<MemoryValue>> {
        let full_key = self.make_key(key);
        self.storage.get(&full_key).await
    }

    /// Delete a value from working memory
    pub async fn delete(&self, key: &str) -> RragResult<bool> {
        let full_key = self.make_key(key);
        self.storage.delete(&full_key).await
    }

    /// Check if a key exists in working memory
    pub async fn exists(&self, key: &str) -> RragResult<bool> {
        let full_key = self.make_key(key);
        self.storage.exists(&full_key).await
    }

    /// Clear all working memory
    pub async fn clear(&self) -> RragResult<()> {
        self.storage.clear(Some(&self.namespace)).await
    }

    /// Get all keys in working memory
    pub async fn keys(&self) -> RragResult<Vec<String>> {
        use crate::storage::MemoryQuery;

        let query = MemoryQuery::new().with_namespace(self.namespace.clone());
        let all_keys = self.storage.keys(&query).await?;

        // Strip namespace prefix
        let prefix = format!("{}::", self.namespace);
        let keys = all_keys
            .into_iter()
            .filter_map(|k| k.strip_prefix(&prefix).map(String::from))
            .collect();

        Ok(keys)
    }

    /// Set multiple values at once
    pub async fn set_many(&self, pairs: &[(&str, MemoryValue)]) -> RragResult<()> {
        let full_pairs: Vec<(String, MemoryValue)> = pairs
            .iter()
            .map(|(k, v)| (self.make_key(k), v.clone()))
            .collect();

        self.storage.mset(&full_pairs).await
    }

    /// Get multiple values at once
    pub async fn get_many(&self, keys: &[&str]) -> RragResult<Vec<Option<MemoryValue>>> {
        let full_keys: Vec<String> = keys.iter().map(|k| self.make_key(k)).collect();
        self.storage.mget(&full_keys).await
    }

    /// Get count of items in working memory
    pub async fn count(&self) -> RragResult<usize> {
        self.storage.count(Some(&self.namespace)).await
    }

    /// Make a fully qualified key
    fn make_key(&self, key: &str) -> String {
        format!("{}::{}", self.namespace, key)
    }

    /// Disable auto-clear on drop
    pub fn disable_auto_clear(&mut self) {
        self.auto_clear = false;
    }

    /// Enable auto-clear on drop
    pub fn enable_auto_clear(&mut self) {
        self.auto_clear = true;
    }
}

impl Drop for WorkingMemory {
    fn drop(&mut self) {
        if self.auto_clear {
            // Best effort cleanup - we can't await in Drop
            // In production, consider using a cleanup task
            tracing::debug!(
                namespace = %self.namespace,
                "WorkingMemory dropped with auto_clear enabled - cleanup deferred"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryStorage;

    #[tokio::test]
    async fn test_working_memory_basic_operations() {
        let storage = Arc::new(InMemoryStorage::new());
        let working = WorkingMemory::new(storage, "test-session".to_string());

        // Set and get
        working.set("temp_result", MemoryValue::from(42i64)).await.unwrap();
        let value = working.get("temp_result").await.unwrap();
        assert_eq!(value.unwrap().as_integer(), Some(42));

        // Exists
        assert!(working.exists("temp_result").await.unwrap());
        assert!(!working.exists("nonexistent").await.unwrap());

        // Delete
        assert!(working.delete("temp_result").await.unwrap());
        assert!(!working.exists("temp_result").await.unwrap());
    }

    #[tokio::test]
    async fn test_working_memory_multiple_operations() {
        let storage = Arc::new(InMemoryStorage::new());
        let working = WorkingMemory::new(storage, "test-session".to_string());

        // Set multiple
        let pairs = [
            ("key1", MemoryValue::from("value1")),
            ("key2", MemoryValue::from(100i64)),
            ("key3", MemoryValue::from(true)),
        ];
        working.set_many(&pairs).await.unwrap();

        // Get multiple
        let keys = ["key1", "key2", "key3"];
        let values = working.get_many(&keys).await.unwrap();

        assert_eq!(values[0].as_ref().unwrap().as_string(), Some("value1"));
        assert_eq!(values[1].as_ref().unwrap().as_integer(), Some(100));
        assert_eq!(values[2].as_ref().unwrap().as_boolean(), Some(true));

        // Count
        assert_eq!(working.count().await.unwrap(), 3);

        // Keys
        let all_keys = working.keys().await.unwrap();
        assert_eq!(all_keys.len(), 3);
    }

    #[tokio::test]
    async fn test_working_memory_clear() {
        let storage = Arc::new(InMemoryStorage::new());
        let working = WorkingMemory::new(storage, "test-session".to_string());

        // Add some data
        working.set("key1", MemoryValue::from("value1")).await.unwrap();
        working.set("key2", MemoryValue::from("value2")).await.unwrap();

        assert_eq!(working.count().await.unwrap(), 2);

        // Clear
        working.clear().await.unwrap();
        assert_eq!(working.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_working_memory_namespace_isolation() {
        let storage = Arc::new(InMemoryStorage::new());
        let working1 = WorkingMemory::new(storage.clone(), "session1".to_string());
        let working2 = WorkingMemory::new(storage.clone(), "session2".to_string());

        // Set in different sessions
        working1.set("data", MemoryValue::from("session1-data")).await.unwrap();
        working2.set("data", MemoryValue::from("session2-data")).await.unwrap();

        // Verify isolation
        let value1 = working1.get("data").await.unwrap();
        let value2 = working2.get("data").await.unwrap();

        assert_eq!(value1.unwrap().as_string(), Some("session1-data"));
        assert_eq!(value2.unwrap().as_string(), Some("session2-data"));
    }
}
