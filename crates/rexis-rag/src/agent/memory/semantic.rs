//! Semantic memory - facts and knowledge storage
//!
//! Semantic memory stores facts, preferences, and learned information about users,
//! entities, and concepts. It's agent-scoped and persists across sessions.
//!
//! Supports optional vector embeddings for semantic similarity search.

use crate::error::RragResult;
use crate::storage::{Memory, MemoryValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[cfg(feature = "vector-search")]
use super::vector::{Embedding, EmbeddingProvider, SearchResult};

/// A semantic fact stored in memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    /// Unique identifier for the fact
    pub id: String,

    /// The subject of the fact (e.g., "user:123", "product:456")
    pub subject: String,

    /// The predicate/relation (e.g., "prefers", "is_located_in", "purchased")
    pub predicate: String,

    /// The object/value (can be any type)
    pub object: MemoryValue,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,

    /// When the fact was created
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// When the fact was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,

    /// Optional metadata
    pub metadata: std::collections::HashMap<String, String>,

    /// Optional vector embedding for similarity search
    #[cfg(feature = "vector-search")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Embedding>,
}

impl Fact {
    /// Create a new fact
    pub fn new(
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<MemoryValue>,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            confidence: 1.0,
            created_at: now,
            updated_at: now,
            metadata: std::collections::HashMap::new(),
            #[cfg(feature = "vector-search")]
            embedding: None,
        }
    }

    /// Set the embedding for this fact
    #[cfg(feature = "vector-search")]
    pub fn with_embedding(mut self, embedding: Embedding) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Set confidence score
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Semantic memory for agent knowledge
pub struct SemanticMemory {
    /// Storage backend
    storage: Arc<dyn Memory>,

    /// Namespace for this semantic memory (agent::{agent_id}::semantic)
    namespace: String,
}

impl SemanticMemory {
    /// Create a new semantic memory
    pub fn new(storage: Arc<dyn Memory>, agent_id: String) -> Self {
        let namespace = format!("agent::{}::semantic", agent_id);

        Self { storage, namespace }
    }

    /// Store a fact
    pub async fn store_fact(&self, fact: Fact) -> RragResult<()> {
        let key = self.fact_key(&fact.id);
        let value = serde_json::to_value(&fact)
            .map_err(|e| crate::error::RragError::storage("serialize_fact", std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        self.storage.set(&key, MemoryValue::Json(value)).await
    }

    /// Retrieve a fact by ID
    pub async fn get_fact(&self, fact_id: &str) -> RragResult<Option<Fact>> {
        let key = self.fact_key(fact_id);
        if let Some(value) = self.storage.get(&key).await? {
            if let Some(json) = value.as_json() {
                let fact = serde_json::from_value(json.clone())
                    .map_err(|e| crate::error::RragError::storage("deserialize_fact", std::io::Error::new(std::io::ErrorKind::Other, e)))?;
                return Ok(Some(fact));
            }
        }
        Ok(None)
    }

    /// Delete a fact
    pub async fn delete_fact(&self, fact_id: &str) -> RragResult<bool> {
        let key = self.fact_key(fact_id);
        self.storage.delete(&key).await
    }

    /// Find facts by subject
    pub async fn find_by_subject(&self, subject: &str) -> RragResult<Vec<Fact>> {
        // This is a simplified implementation
        // In a production system, you'd want indexing or vector search
        let all_keys = self.list_fact_keys().await?;
        let mut matching_facts = Vec::new();

        for key in all_keys {
            if let Some(fact) = self.get_fact(&key).await? {
                if fact.subject == subject {
                    matching_facts.push(fact);
                }
            }
        }

        Ok(matching_facts)
    }

    /// Find facts by predicate
    pub async fn find_by_predicate(&self, predicate: &str) -> RragResult<Vec<Fact>> {
        let all_keys = self.list_fact_keys().await?;
        let mut matching_facts = Vec::new();

        for key in all_keys {
            if let Some(fact) = self.get_fact(&key).await? {
                if fact.predicate == predicate {
                    matching_facts.push(fact);
                }
            }
        }

        Ok(matching_facts)
    }

    /// Find facts by subject and predicate
    pub async fn find_by_subject_and_predicate(
        &self,
        subject: &str,
        predicate: &str,
    ) -> RragResult<Vec<Fact>> {
        let all_keys = self.list_fact_keys().await?;
        let mut matching_facts = Vec::new();

        for key in all_keys {
            if let Some(fact) = self.get_fact(&key).await? {
                if fact.subject == subject && fact.predicate == predicate {
                    matching_facts.push(fact);
                }
            }
        }

        Ok(matching_facts)
    }

    /// Get all facts
    pub async fn get_all_facts(&self) -> RragResult<Vec<Fact>> {
        let all_keys = self.list_fact_keys().await?;
        let mut facts = Vec::new();

        for key in all_keys {
            if let Some(fact) = self.get_fact(&key).await? {
                facts.push(fact);
            }
        }

        Ok(facts)
    }

    /// Count facts
    pub async fn count(&self) -> RragResult<usize> {
        self.storage.count(Some(&self.namespace)).await
    }

    /// Clear all facts
    pub async fn clear(&self) -> RragResult<()> {
        self.storage.clear(Some(&self.namespace)).await
    }

    /// Generate fact key
    fn fact_key(&self, fact_id: &str) -> String {
        format!("{}::fact::{}", self.namespace, fact_id)
    }

    /// List all fact keys (IDs)
    async fn list_fact_keys(&self) -> RragResult<Vec<String>> {
        use crate::storage::MemoryQuery;

        let query = MemoryQuery::new().with_namespace(self.namespace.clone());
        let all_keys = self.storage.keys(&query).await?;

        // Extract fact IDs from keys
        let prefix = format!("{}::fact::", self.namespace);
        let ids = all_keys
            .into_iter()
            .filter_map(|k| k.strip_prefix(&prefix).map(String::from))
            .collect();

        Ok(ids)
    }

    /// Search for facts using vector similarity (requires 'vector-search' feature)
    #[cfg(feature = "vector-search")]
    pub async fn vector_search(
        &self,
        query_embedding: &Embedding,
        limit: usize,
        min_similarity: f32,
    ) -> RragResult<Vec<SearchResult<Fact>>> {
        let all_facts = self.get_all_facts().await?;
        let mut results = Vec::new();

        for fact in all_facts {
            if let Some(fact_embedding) = &fact.embedding {
                match query_embedding.cosine_similarity(fact_embedding) {
                    Ok(similarity) => {
                        if similarity >= min_similarity {
                            results.push(SearchResult::new(fact, similarity));
                        }
                    }
                    Err(_) => continue, // Skip facts with incompatible embeddings
                }
            }
        }

        // Sort by similarity (highest first)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Take top N results
        results.truncate(limit);

        Ok(results)
    }

    /// Store a fact with automatic embedding generation (requires 'vector-search' feature)
    #[cfg(feature = "vector-search")]
    pub async fn store_fact_with_embedding<P>(
        &self,
        mut fact: Fact,
        provider: &P,
    ) -> RragResult<()>
    where
        P: EmbeddingProvider,
    {
        // Generate text representation for embedding
        let text = format!(
            "{} {} {}",
            fact.subject,
            fact.predicate,
            fact.object.as_string().unwrap_or_default()
        );

        // Generate embedding
        let embedding = provider.embed(&text).await?;
        fact.embedding = Some(embedding);

        // Store the fact
        self.store_fact(fact).await
    }

    /// Find similar facts to a query text (requires 'vector-search' feature)
    #[cfg(feature = "vector-search")]
    pub async fn find_similar<P>(
        &self,
        query: &str,
        provider: &P,
        limit: usize,
        min_similarity: f32,
    ) -> RragResult<Vec<SearchResult<Fact>>>
    where
        P: EmbeddingProvider,
    {
        // Generate embedding for query
        let query_embedding = provider.embed(query).await?;

        // Search using embedding
        self.vector_search(&query_embedding, limit, min_similarity).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryStorage;

    #[tokio::test]
    async fn test_semantic_memory_store_and_retrieve() {
        let storage = Arc::new(InMemoryStorage::new());
        let semantic = SemanticMemory::new(storage, "test-agent".to_string());

        // Store a fact
        let fact = Fact::new("user:alice", "prefers", MemoryValue::from("dark_mode"))
            .with_confidence(0.9);

        let fact_id = fact.id.clone();
        semantic.store_fact(fact).await.unwrap();

        // Retrieve it
        let retrieved = semantic.get_fact(&fact_id).await.unwrap().unwrap();
        assert_eq!(retrieved.subject, "user:alice");
        assert_eq!(retrieved.predicate, "prefers");
        assert_eq!(retrieved.object.as_string(), Some("dark_mode"));
        assert_eq!(retrieved.confidence, 0.9);
    }

    #[tokio::test]
    async fn test_semantic_memory_find_by_subject() {
        let storage = Arc::new(InMemoryStorage::new());
        let semantic = SemanticMemory::new(storage, "test-agent".to_string());

        // Store multiple facts
        semantic
            .store_fact(Fact::new("user:alice", "prefers", MemoryValue::from("dark_mode")))
            .await
            .unwrap();
        semantic
            .store_fact(Fact::new("user:alice", "likes", MemoryValue::from("coffee")))
            .await
            .unwrap();
        semantic
            .store_fact(Fact::new("user:bob", "prefers", MemoryValue::from("light_mode")))
            .await
            .unwrap();

        // Find by subject
        let alice_facts = semantic.find_by_subject("user:alice").await.unwrap();
        assert_eq!(alice_facts.len(), 2);

        let bob_facts = semantic.find_by_subject("user:bob").await.unwrap();
        assert_eq!(bob_facts.len(), 1);
    }

    #[tokio::test]
    async fn test_semantic_memory_find_by_predicate() {
        let storage = Arc::new(InMemoryStorage::new());
        let semantic = SemanticMemory::new(storage, "test-agent".to_string());

        // Store facts
        semantic
            .store_fact(Fact::new("user:alice", "prefers", MemoryValue::from("dark_mode")))
            .await
            .unwrap();
        semantic
            .store_fact(Fact::new("user:bob", "prefers", MemoryValue::from("light_mode")))
            .await
            .unwrap();
        semantic
            .store_fact(Fact::new("user:alice", "likes", MemoryValue::from("coffee")))
            .await
            .unwrap();

        // Find by predicate
        let prefer_facts = semantic.find_by_predicate("prefers").await.unwrap();
        assert_eq!(prefer_facts.len(), 2);

        let like_facts = semantic.find_by_predicate("likes").await.unwrap();
        assert_eq!(like_facts.len(), 1);
    }

    #[tokio::test]
    async fn test_semantic_memory_delete() {
        let storage = Arc::new(InMemoryStorage::new());
        let semantic = SemanticMemory::new(storage, "test-agent".to_string());

        // Store and delete
        let fact = Fact::new("user:alice", "prefers", MemoryValue::from("dark_mode"));
        let fact_id = fact.id.clone();
        semantic.store_fact(fact).await.unwrap();

        assert_eq!(semantic.count().await.unwrap(), 1);

        semantic.delete_fact(&fact_id).await.unwrap();
        assert_eq!(semantic.count().await.unwrap(), 0);
    }
}
