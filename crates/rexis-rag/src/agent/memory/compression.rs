//! Memory compression and optimization strategies
//!
//! Provides utilities for compressing, archiving, and optimizing memory storage
//! to manage memory growth over long conversations and agent lifecycles.

use crate::error::RragResult;
use crate::storage::{Memory, MemoryValue};
use std::sync::Arc;

#[cfg(feature = "rexis-llm-client")]
use rexis_llm::{Client, ChatMessage};

/// Configuration for memory compression
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Maximum size in bytes before triggering compression
    pub max_size_bytes: usize,

    /// Maximum number of items before compression
    pub max_items: usize,

    /// Compression ratio target (0.0 to 1.0)
    pub compression_ratio: f64,

    /// Enable LLM-based intelligent compression
    pub use_llm_compression: bool,

    /// Minimum importance score to keep uncompressed
    pub min_importance_threshold: f64,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 10_000_000, // 10MB
            max_items: 10_000,
            compression_ratio: 0.5,
            use_llm_compression: true,
            min_importance_threshold: 0.7,
        }
    }
}

/// Memory compression strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionStrategy {
    /// Remove oldest entries
    RemoveOldest,

    /// Remove least important entries
    RemoveLeastImportant,

    /// Merge similar entries
    MergeSimilar,

    /// Summarize and archive
    SummarizeAndArchive,

    /// Binary compression (gzip/zstd)
    BinaryCompression,
}

/// Memory statistics for compression decisions
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// Total size in bytes
    pub total_bytes: usize,

    /// Number of items
    pub item_count: usize,

    /// Average item size
    pub avg_item_size: usize,

    /// Oldest item timestamp
    pub oldest_timestamp: Option<chrono::DateTime<chrono::Utc>>,

    /// Newest item timestamp
    pub newest_timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

/// Memory compressor
pub struct MemoryCompressor {
    storage: Arc<dyn Memory>,
    config: CompressionConfig,
}

impl MemoryCompressor {
    /// Create a new memory compressor
    pub fn new(storage: Arc<dyn Memory>, config: CompressionConfig) -> Self {
        Self { storage, config }
    }

    /// Check if compression is needed based on stats
    pub fn needs_compression(&self, stats: &MemoryStats) -> bool {
        stats.total_bytes > self.config.max_size_bytes
            || stats.item_count > self.config.max_items
    }

    /// Calculate memory statistics for a namespace
    pub async fn calculate_stats(&self, namespace: &str) -> RragResult<MemoryStats> {
        use crate::storage::MemoryQuery;

        let query = MemoryQuery::new().with_namespace(namespace.to_string());
        let keys = self.storage.keys(&query).await?;

        let mut total_bytes = 0;
        let mut oldest: Option<chrono::DateTime<chrono::Utc>> = None;
        let mut newest: Option<chrono::DateTime<chrono::Utc>> = None;

        for key in &keys {
            if let Some(value) = self.storage.get(key).await? {
                // Estimate size (rough approximation)
                total_bytes += match &value {
                    MemoryValue::String(s) => s.len(),
                    MemoryValue::Integer(_) => 8,
                    MemoryValue::Float(_) => 8,
                    MemoryValue::Boolean(_) => 1,
                    MemoryValue::Json(j) => j.to_string().len(),
                    MemoryValue::Bytes(b) => b.len(),
                    MemoryValue::List(items) => items.len() * 16, // rough estimate
                    MemoryValue::Map(m) => m.len() * 32,           // rough estimate
                };

                // Try to extract timestamp from JSON values
                if let MemoryValue::Json(json) = value {
                    if let Some(timestamp_str) = json.get("timestamp").and_then(|v| v.as_str()) {
                        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
                            let utc_ts = ts.with_timezone(&chrono::Utc);
                            oldest = Some(oldest.map_or(utc_ts, |o| o.min(utc_ts)));
                            newest = Some(newest.map_or(utc_ts, |n| n.max(utc_ts)));
                        }
                    } else if let Some(created_str) = json.get("created_at").and_then(|v| v.as_str())
                    {
                        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(created_str) {
                            let utc_ts = ts.with_timezone(&chrono::Utc);
                            oldest = Some(oldest.map_or(utc_ts, |o| o.min(utc_ts)));
                            newest = Some(newest.map_or(utc_ts, |n| n.max(utc_ts)));
                        }
                    }
                }
            }
        }

        let item_count = keys.len();
        let avg_item_size = if item_count > 0 {
            total_bytes / item_count
        } else {
            0
        };

        Ok(MemoryStats {
            total_bytes,
            item_count,
            avg_item_size,
            oldest_timestamp: oldest,
            newest_timestamp: newest,
        })
    }

    /// Compress conversation memory by summarizing old messages (requires 'rsllm-client' feature)
    #[cfg(feature = "rexis-llm-client")]
    pub async fn compress_conversation_memory(
        &self,
        namespace: &str,
        llm_client: &Client,
        keep_recent_count: usize,
    ) -> RragResult<usize> {
        use crate::storage::MemoryQuery;

        let query = MemoryQuery::new().with_namespace(namespace.to_string());
        let keys = self.storage.keys(&query).await?;

        if keys.len() <= keep_recent_count {
            return Ok(0); // Nothing to compress
        }

        // Get all messages with timestamps
        let mut messages: Vec<(String, serde_json::Value, chrono::DateTime<chrono::Utc>)> =
            Vec::new();

        for key in &keys {
            if let Some(value) = self.storage.get(key).await? {
                if let MemoryValue::Json(json) = value {
                    if let Some(timestamp_str) = json.get("timestamp").and_then(|v| v.as_str()) {
                        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
                            messages.push((key.clone(), json, ts.with_timezone(&chrono::Utc)));
                        }
                    }
                }
            }
        }

        // Sort by timestamp (oldest first)
        messages.sort_by(|a, b| a.2.cmp(&b.2));

        // Keep recent messages, compress old ones
        let to_compress = messages.len().saturating_sub(keep_recent_count);

        if to_compress == 0 {
            return Ok(0);
        }

        // Build text from old messages
        let mut old_messages_text = String::new();
        for (_, json, _) in messages.iter().take(to_compress) {
            if let Some(role) = json.get("role").and_then(|v| v.as_str()) {
                if let Some(content) = json.get("content").and_then(|v| v.as_str()) {
                    old_messages_text.push_str(&format!("{}: {}\n", role, content));
                }
            }
        }

        // Generate summary using LLM
        let summary_msg = ChatMessage::user(format!(
            "Summarize these conversation messages in 2-3 sentences:\n\n{}",
            old_messages_text
        ));

        let response = llm_client
            .chat_completion(vec![summary_msg])
            .await
            .map_err(|e| crate::error::RragError::rsllm_client("conversation_compression", e))?;

        let summary = response.content.trim().to_string();

        // Store summary
        let summary_key = format!("{}::summary::compressed", namespace);
        self.storage
            .set(
                &summary_key,
                MemoryValue::Json(serde_json::json!({
                    "summary": summary,
                    "compressed_count": to_compress,
                    "compressed_at": chrono::Utc::now().to_rfc3339(),
                })),
            )
            .await?;

        // Delete old messages
        let mut deleted = 0;
        for (key, _, _) in messages.iter().take(to_compress) {
            if self.storage.delete(key).await? {
                deleted += 1;
            }
        }

        tracing::info!(
            namespace = namespace,
            deleted = deleted,
            "Compressed conversation memory"
        );

        Ok(deleted)
    }

    /// Remove old items based on timestamp
    pub async fn remove_old_items(
        &self,
        namespace: &str,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> RragResult<usize> {
        use crate::storage::MemoryQuery;

        let query = MemoryQuery::new().with_namespace(namespace.to_string());
        let keys = self.storage.keys(&query).await?;

        let mut deleted = 0;

        for key in keys {
            if let Some(value) = self.storage.get(&key).await? {
                if let MemoryValue::Json(json) = value {
                    let should_delete = if let Some(timestamp_str) =
                        json.get("timestamp").and_then(|v| v.as_str())
                    {
                        chrono::DateTime::parse_from_rfc3339(timestamp_str)
                            .ok()
                            .map(|ts| ts.with_timezone(&chrono::Utc) < older_than)
                            .unwrap_or(false)
                    } else if let Some(created_str) = json.get("created_at").and_then(|v| v.as_str())
                    {
                        chrono::DateTime::parse_from_rfc3339(created_str)
                            .ok()
                            .map(|ts| ts.with_timezone(&chrono::Utc) < older_than)
                            .unwrap_or(false)
                    } else {
                        false
                    };

                    if should_delete && self.storage.delete(&key).await? {
                        deleted += 1;
                    }
                }
            }
        }

        tracing::info!(
            namespace = namespace,
            deleted = deleted,
            "Removed old items"
        );

        Ok(deleted)
    }

    /// Remove least important items
    pub async fn remove_least_important(
        &self,
        namespace: &str,
        min_importance: f64,
        max_to_remove: usize,
    ) -> RragResult<usize> {
        use crate::storage::MemoryQuery;

        let query = MemoryQuery::new().with_namespace(namespace.to_string());
        let keys = self.storage.keys(&query).await?;

        let mut items_with_importance: Vec<(String, f64)> = Vec::new();

        for key in keys {
            if let Some(value) = self.storage.get(&key).await? {
                if let MemoryValue::Json(json) = value {
                    if let Some(importance) = json.get("importance").and_then(|v| v.as_f64()) {
                        items_with_importance.push((key, importance));
                    }
                }
            }
        }

        // Sort by importance (ascending)
        items_with_importance.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let mut deleted = 0;

        for (key, importance) in items_with_importance.iter().take(max_to_remove) {
            if *importance < min_importance && self.storage.delete(key).await? {
                deleted += 1;
            }
        }

        tracing::info!(
            namespace = namespace,
            deleted = deleted,
            "Removed least important items"
        );

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryStorage;

    #[tokio::test]
    async fn test_memory_stats_calculation() {
        let storage = Arc::new(InMemoryStorage::new());
        let config = CompressionConfig::default();
        let compressor = MemoryCompressor::new(storage.clone(), config);

        // Store some test data
        let namespace = "test";
        for i in 0..5 {
            let key = format!("{}::item::{}", namespace, i);
            let value = MemoryValue::Json(serde_json::json!({
                "id": i,
                "content": "test data",
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }));
            storage.set(&key, value).await.unwrap();
        }

        let stats = compressor.calculate_stats(namespace).await.unwrap();

        assert_eq!(stats.item_count, 5);
        assert!(stats.total_bytes > 0);
        assert!(stats.avg_item_size > 0);
    }

    #[tokio::test]
    async fn test_remove_old_items() {
        let storage = Arc::new(InMemoryStorage::new());
        let config = CompressionConfig::default();
        let compressor = MemoryCompressor::new(storage.clone(), config);

        let namespace = "test";

        // Store old item
        let old_time = chrono::Utc::now() - chrono::Duration::days(10);
        storage
            .set(
                &format!("{}::old", namespace),
                MemoryValue::Json(serde_json::json!({
                    "timestamp": old_time.to_rfc3339(),
                })),
            )
            .await
            .unwrap();

        // Store recent item
        storage
            .set(
                &format!("{}::recent", namespace),
                MemoryValue::Json(serde_json::json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })),
            )
            .await
            .unwrap();

        // Remove items older than 5 days
        let cutoff = chrono::Utc::now() - chrono::Duration::days(5);
        let deleted = compressor.remove_old_items(namespace, cutoff).await.unwrap();

        assert_eq!(deleted, 1);
        assert_eq!(storage.count(Some(namespace)).await.unwrap(), 1);
    }
}
