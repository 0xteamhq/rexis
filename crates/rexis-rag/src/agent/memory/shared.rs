//! Shared knowledge base - cross-agent memory
//!
//! Shared knowledge allows multiple agents to read and write to a common memory space.
//! It's global-scoped and enables agent collaboration and information sharing.

use crate::error::RragResult;
use crate::storage::{Memory, MemoryValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A shared knowledge entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    /// Unique identifier
    pub id: String,

    /// Key for the knowledge
    pub key: String,

    /// The value/content
    pub value: MemoryValue,

    /// Agent that created this entry
    pub created_by: String,

    /// When it was created
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Agent that last updated this entry
    pub updated_by: String,

    /// When it was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,

    /// Tags for categorization
    pub tags: Vec<String>,

    /// Access control list (agent IDs that can access)
    pub acl: Option<Vec<String>>,

    /// Optional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl KnowledgeEntry {
    /// Create a new knowledge entry
    pub fn new(
        key: impl Into<String>,
        value: impl Into<MemoryValue>,
        created_by: impl Into<String>,
    ) -> Self {
        let now = chrono::Utc::now();
        let created_by = created_by.into();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            key: key.into(),
            value: value.into(),
            created_by: created_by.clone(),
            created_at: now,
            updated_by: created_by,
            updated_at: now,
            tags: Vec::new(),
            acl: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Set tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set access control list
    pub fn with_acl(mut self, acl: Vec<String>) -> Self {
        self.acl = Some(acl);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Check if an agent has access
    pub fn has_access(&self, agent_id: &str) -> bool {
        match &self.acl {
            None => true, // No ACL means public access
            Some(acl) => acl.contains(&agent_id.to_string()) || agent_id == self.created_by,
        }
    }
}

/// Shared knowledge base for cross-agent memory
pub struct SharedKnowledgeBase {
    /// Storage backend
    storage: Arc<dyn Memory>,

    /// Agent ID (for tracking who creates/updates entries)
    agent_id: String,

    /// Namespace (global::knowledge)
    namespace: String,
}

impl SharedKnowledgeBase {
    /// Create a new shared knowledge base
    pub fn new(storage: Arc<dyn Memory>, agent_id: String) -> Self {
        Self {
            storage,
            agent_id,
            namespace: "global::knowledge".to_string(),
        }
    }

    /// Store a knowledge entry
    pub async fn store(
        &self,
        key: impl Into<String>,
        value: impl Into<MemoryValue>,
    ) -> RragResult<KnowledgeEntry> {
        let entry = KnowledgeEntry::new(key, value, self.agent_id.clone());
        self.store_entry(entry.clone()).await?;
        Ok(entry)
    }

    /// Store a knowledge entry with tags
    pub async fn store_with_tags(
        &self,
        key: impl Into<String>,
        value: impl Into<MemoryValue>,
        tags: Vec<String>,
    ) -> RragResult<KnowledgeEntry> {
        let entry = KnowledgeEntry::new(key, value, self.agent_id.clone()).with_tags(tags);
        self.store_entry(entry.clone()).await?;
        Ok(entry)
    }

    /// Store a full knowledge entry
    pub async fn store_entry(&self, mut entry: KnowledgeEntry) -> RragResult<()> {
        // Update metadata
        entry.updated_by = self.agent_id.clone();
        entry.updated_at = chrono::Utc::now();

        let storage_key = self.entry_key(&entry.key);
        let value = serde_json::to_value(&entry).map_err(|e| {
            crate::error::RragError::storage(
                "serialize_entry",
                std::io::Error::new(std::io::ErrorKind::Other, e),
            )
        })?;

        self.storage
            .set(&storage_key, MemoryValue::Json(value))
            .await
    }

    /// Get a knowledge entry
    pub async fn get(&self, key: &str) -> RragResult<Option<KnowledgeEntry>> {
        let storage_key = self.entry_key(key);
        if let Some(value) = self.storage.get(&storage_key).await? {
            if let Some(json) = value.as_json() {
                let entry: KnowledgeEntry = serde_json::from_value(json.clone()).map_err(|e| {
                    crate::error::RragError::storage(
                        "deserialize_entry",
                        std::io::Error::new(std::io::ErrorKind::Other, e),
                    )
                })?;

                // Check ACL
                if entry.has_access(&self.agent_id) {
                    return Ok(Some(entry));
                }
            }
        }
        Ok(None)
    }

    /// Get just the value (without metadata)
    pub async fn get_value(&self, key: &str) -> RragResult<Option<MemoryValue>> {
        if let Some(entry) = self.get(key).await? {
            Ok(Some(entry.value))
        } else {
            Ok(None)
        }
    }

    /// Delete a knowledge entry
    pub async fn delete(&self, key: &str) -> RragResult<bool> {
        // Check if the current agent has permission to delete
        if let Some(entry) = self.get(key).await? {
            if entry.created_by != self.agent_id {
                // Only creator can delete (or implement more sophisticated permissions)
                return Ok(false);
            }
        }

        let storage_key = self.entry_key(key);
        self.storage.delete(&storage_key).await
    }

    /// Check if a key exists and is accessible
    pub async fn exists(&self, key: &str) -> RragResult<bool> {
        Ok(self.get(key).await?.is_some())
    }

    /// Find entries by tag
    pub async fn find_by_tag(&self, tag: &str) -> RragResult<Vec<KnowledgeEntry>> {
        let all_entries = self.get_all_entries().await?;

        let matching = all_entries
            .into_iter()
            .filter(|e| e.has_access(&self.agent_id) && e.tags.contains(&tag.to_string()))
            .collect();

        Ok(matching)
    }

    /// Find entries created by a specific agent
    pub async fn find_by_creator(&self, creator_agent_id: &str) -> RragResult<Vec<KnowledgeEntry>> {
        let all_entries = self.get_all_entries().await?;

        let matching = all_entries
            .into_iter()
            .filter(|e| e.has_access(&self.agent_id) && e.created_by == creator_agent_id)
            .collect();

        Ok(matching)
    }

    /// Get all accessible entries
    pub async fn get_all_entries(&self) -> RragResult<Vec<KnowledgeEntry>> {
        let all_keys = self.list_entry_keys().await?;
        let mut entries = Vec::new();

        for key in all_keys {
            if let Some(entry) = self.get(&key).await? {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Count accessible entries
    pub async fn count(&self) -> RragResult<usize> {
        // This counts all entries; for accurate count, filter by ACL
        self.storage.count(Some(&self.namespace)).await
    }

    /// Clear all entries (requires appropriate permissions)
    pub async fn clear(&self) -> RragResult<()> {
        self.storage.clear(Some(&self.namespace)).await
    }

    /// Generate entry key
    fn entry_key(&self, key: &str) -> String {
        format!("{}::{}", self.namespace, key)
    }

    /// List all entry keys
    async fn list_entry_keys(&self) -> RragResult<Vec<String>> {
        use crate::storage::MemoryQuery;

        let query = MemoryQuery::new().with_namespace(self.namespace.clone());
        let all_keys = self.storage.keys(&query).await?;

        // Extract entry keys
        let prefix = format!("{}::", self.namespace);
        let keys = all_keys
            .into_iter()
            .filter_map(|k| k.strip_prefix(&prefix).map(String::from))
            .collect();

        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryStorage;

    #[tokio::test]
    async fn test_shared_knowledge_store_and_retrieve() {
        let storage = Arc::new(InMemoryStorage::new());
        let kb = SharedKnowledgeBase::new(storage, "agent1".to_string());

        // Store and retrieve
        kb.store("api_key", MemoryValue::from("secret123"))
            .await
            .unwrap();

        let value = kb.get_value("api_key").await.unwrap().unwrap();
        assert_eq!(value.as_string(), Some("secret123"));
    }

    #[tokio::test]
    async fn test_shared_knowledge_cross_agent_access() {
        let storage = Arc::new(InMemoryStorage::new());
        let kb1 = SharedKnowledgeBase::new(storage.clone(), "agent1".to_string());
        let kb2 = SharedKnowledgeBase::new(storage.clone(), "agent2".to_string());

        // Agent1 stores data
        kb1.store("shared_config", MemoryValue::from("config_value"))
            .await
            .unwrap();

        // Agent2 can access it
        let value = kb2.get_value("shared_config").await.unwrap().unwrap();
        assert_eq!(value.as_string(), Some("config_value"));
    }

    #[tokio::test]
    async fn test_shared_knowledge_with_acl() {
        let storage = Arc::new(InMemoryStorage::new());
        let kb1 = SharedKnowledgeBase::new(storage.clone(), "agent1".to_string());
        let kb2 = SharedKnowledgeBase::new(storage.clone(), "agent2".to_string());
        let kb3 = SharedKnowledgeBase::new(storage.clone(), "agent3".to_string());

        // Agent1 stores with ACL (only agent1 and agent2)
        let entry = KnowledgeEntry::new("private_data", MemoryValue::from("secret"), "agent1")
            .with_acl(vec!["agent1".to_string(), "agent2".to_string()]);

        kb1.store_entry(entry).await.unwrap();

        // Agent2 can access
        assert!(kb2.get("private_data").await.unwrap().is_some());

        // Agent3 cannot access
        assert!(kb3.get("private_data").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_shared_knowledge_with_tags() {
        let storage = Arc::new(InMemoryStorage::new());
        let kb = SharedKnowledgeBase::new(storage, "agent1".to_string());

        // Store with tags
        kb.store_with_tags(
            "config1",
            MemoryValue::from("value1"),
            vec!["config".to_string(), "production".to_string()],
        )
        .await
        .unwrap();

        kb.store_with_tags(
            "config2",
            MemoryValue::from("value2"),
            vec!["config".to_string(), "development".to_string()],
        )
        .await
        .unwrap();

        // Find by tag
        let config_entries = kb.find_by_tag("config").await.unwrap();
        assert_eq!(config_entries.len(), 2);

        let prod_entries = kb.find_by_tag("production").await.unwrap();
        assert_eq!(prod_entries.len(), 1);
    }

    #[tokio::test]
    async fn test_shared_knowledge_delete_permissions() {
        let storage = Arc::new(InMemoryStorage::new());
        let kb1 = SharedKnowledgeBase::new(storage.clone(), "agent1".to_string());
        let kb2 = SharedKnowledgeBase::new(storage.clone(), "agent2".to_string());

        // Agent1 creates entry
        kb1.store("data", MemoryValue::from("value")).await.unwrap();

        // Agent2 cannot delete (not creator)
        let deleted = kb2.delete("data").await.unwrap();
        assert!(!deleted);

        // Agent1 can delete
        let deleted = kb1.delete("data").await.unwrap();
        assert!(deleted);
    }
}
