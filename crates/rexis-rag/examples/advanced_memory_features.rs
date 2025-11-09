//! # Advanced Memory Features Demo (Phase 4)
//!
//! Demonstrates the advanced memory capabilities added in Phase 4:
//!
//! ## Features Demonstrated
//!
//! ### 1. Vector Search (Semantic Memory)
//! - Embedding generation with multiple providers
//! - Cosine similarity search
//! - Finding semantically similar facts
//!
//! ### 2. LLM-Based Summarization (Episodic Memory)
//! - Automatic episode creation from conversations
//! - Comprehensive summary generation
//! - Insight extraction
//!
//! ### 3. Memory Compression
//! - Conversation memory compression
//! - Old item removal based on timestamp
//! - Least important item removal
//! - Memory statistics calculation
//!
//! ## Run This Example
//!
//! ```bash
//! # With all features
//! cargo run --example advanced_memory_features --features rexis-llm-client,vector-search
//!
//! # Just vector search (no LLM required)
//! cargo run --example advanced_memory_features --features vector-search --no-default-features
//! ```

use rexis_rag::agent::memory::{
    CompressionConfig, Episode, EpisodicMemory, Fact, MemoryCompressor, SemanticMemory,
};
use rexis_rag::storage::{InMemoryStorage, Memory, MemoryValue};
use std::sync::Arc;
use tracing::info;

#[cfg(feature = "vector-search")]
use rexis_rag::agent::memory::{Embedding, HashEmbeddingProvider};

#[cfg(feature = "rexis-llm-client")]
use rexis_llm::{ChatMessage, Client};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (simple fmt for examples)
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("=== Advanced Memory Features Demo ===\n");

    // Create shared storage
    let storage: Arc<dyn Memory> = Arc::new(InMemoryStorage::new());

    // ========================================
    // 1. VECTOR SEARCH (Semantic Memory)
    // ========================================
    #[cfg(feature = "vector-search")]
    {
        info!("=== 1. Vector Search Demo ===");
        demo_vector_search(storage.clone()).await?;
        info!("");
    }

    // ========================================
    // 2. LLM-BASED SUMMARIZATION
    // ========================================
    #[cfg(feature = "rexis-llm-client")]
    {
        info!("=== 2. LLM-Based Summarization Demo ===");
        demo_llm_summarization(storage.clone()).await?;
        info!("");
    }

    // ========================================
    // 3. MEMORY COMPRESSION
    // ========================================
    info!("=== 3. Memory Compression Demo ===");
    demo_memory_compression(storage.clone()).await?;

    info!("\n=== Demo Complete ===");
    info!("Phase 4 Features Summary:");
    info!("  ✓ Vector embeddings and similarity search");
    info!("  ✓ LLM-based automatic summarization");
    info!("  ✓ Memory compression and optimization");
    info!("  ✓ Cross-session memory tools");

    Ok(())
}

#[cfg(feature = "vector-search")]
async fn demo_vector_search(storage: Arc<dyn Memory>) -> Result<(), Box<dyn std::error::Error>> {
    use rexis_rag::agent::memory::EmbeddingProvider;

    info!("Creating semantic memory with vector search...");
    let semantic = SemanticMemory::new(storage.clone(), "vector-agent".to_string());

    // Create embedding provider (using hash-based for demo)
    let provider = HashEmbeddingProvider::new(128);

    info!("Storing facts with embeddings...");

    // Store facts with embeddings
    let facts = vec![
        ("user:alice", "prefers", "Rust programming"),
        ("user:alice", "enjoys", "functional programming"),
        ("user:bob", "likes", "Python scripting"),
        ("user:bob", "prefers", "object-oriented programming"),
        ("user:charlie", "loves", "systems programming in Rust"),
    ];

    for (subject, predicate, object) in facts {
        let fact = Fact::new(subject, predicate, MemoryValue::from(object));
        semantic.store_fact_with_embedding(fact, &provider).await?;
    }

    info!("Stored {} facts with embeddings", semantic.count().await?);

    // Search for similar facts
    info!("\nSearching for facts similar to 'Rust development'...");
    let results = semantic
        .find_similar("Rust development", &provider, 3, 0.0)
        .await?;

    for (i, result) in results.iter().enumerate() {
        info!(
            "  {}. [Score: {:.3}] {} {} {}",
            i + 1,
            result.score,
            result.item.subject,
            result.item.predicate,
            result.item.object.as_string().unwrap_or_default()
        );
    }

    // Test cosine similarity directly
    info!("\nTesting embedding similarity...");
    let emb1 = provider.embed("Rust programming").await?;
    let emb2 = provider.embed("Rust development").await?;
    let emb3 = provider.embed("Python scripting").await?;

    let sim_rust = emb1.cosine_similarity(&emb2)?;
    let sim_python = emb1.cosine_similarity(&emb3)?;

    info!(
        "  Similarity (Rust programming vs Rust development): {:.3}",
        sim_rust
    );
    info!(
        "  Similarity (Rust programming vs Python scripting): {:.3}",
        sim_python
    );

    Ok(())
}

#[cfg(feature = "rexis-llm-client")]
async fn demo_llm_summarization(
    storage: Arc<dyn Memory>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Setting up LLM client...");

    // Create LLM client (requires RSLLM_PROVIDER and RSLLM_MODEL env vars)
    let client = match Client::from_env() {
        Ok(c) => c,
        Err(e) => {
            info!("⚠️  LLM client not available: {}", e);
            info!("   Set RSLLM_PROVIDER=ollama and RSLLM_MODEL=llama3.2:3b to enable");
            return Ok(());
        }
    };

    let episodic = EpisodicMemory::new(storage.clone(), "llm-agent".to_string());

    info!("Creating episode from conversation messages...");

    // Simulate a conversation
    let messages = vec![
        ChatMessage::user("How do I use trait objects in Rust?"),
        ChatMessage::assistant(
            "Trait objects allow you to use dynamic dispatch in Rust. You create them using `dyn Trait` syntax.",
        ),
        ChatMessage::user("Can you show me an example?"),
        ChatMessage::assistant(
            "Sure! Here's an example: `let obj: Box<dyn MyTrait> = Box::new(MyStruct);`",
        ),
        ChatMessage::user("Thanks, that's helpful!"),
    ];

    // Create episode from messages using LLM summarization
    match episodic
        .create_episode_from_messages(&messages, &client)
        .await
    {
        Ok(episode) => {
            info!("✓ Generated episode summary:");
            info!("  Summary: {}", episode.summary);
            info!("  Topics: {:?}", episode.topics);
            info!("  Importance: {:.2}", episode.importance);

            // Store the episode
            episodic.store_episode(episode.clone()).await?;

            // Extract insights
            info!("\nExtracting insights from episode...");
            match episodic.extract_insights(&episode, &client).await {
                Ok(insights) => {
                    info!("✓ Extracted {} insights:", insights.len());
                    for (i, insight) in insights.iter().enumerate() {
                        info!("  {}. {}", i + 1, insight);
                    }
                }
                Err(e) => info!("⚠️  Could not extract insights: {}", e),
            }
        }
        Err(e) => {
            info!("⚠️  Could not create episode: {}", e);
            info!("   This requires a running LLM (e.g., ollama serve)");
        }
    }

    // Store a few more episodes manually
    for i in 1..=3 {
        let ep = Episode::new(format!("Discussion about topic {}", i))
            .with_topics(vec!["rust".to_string(), "programming".to_string()])
            .with_importance(0.5 + (i as f64 * 0.1));
        episodic.store_episode(ep).await?;
    }

    info!("\nTotal episodes stored: {}", episodic.count().await?);

    // Generate comprehensive summary
    info!("\nGenerating comprehensive summary of recent episodes...");
    match episodic.generate_llm_summary(5, &client).await {
        Ok(summary) => {
            info!("✓ Comprehensive summary:");
            info!("{}", summary);
        }
        Err(e) => info!("⚠️  Could not generate summary: {}", e),
    }

    Ok(())
}

async fn demo_memory_compression(
    storage: Arc<dyn Memory>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Setting up memory compression...");

    let config = CompressionConfig {
        max_size_bytes: 1_000_000,
        max_items: 100,
        compression_ratio: 0.5,
        use_llm_compression: true,
        min_importance_threshold: 0.6,
    };

    let compressor = MemoryCompressor::new(storage.clone(), config);

    // Create test namespace with data
    let namespace = "test-compression";

    info!("Creating test data...");
    for i in 0..10 {
        let days_ago = 10 - i;
        let timestamp = chrono::Utc::now() - chrono::Duration::days(days_ago);

        let key = format!("{}::item::{}", namespace, i);
        let value = MemoryValue::Json(serde_json::json!({
            "id": i,
            "content": format!("Test message {}", i),
            "importance": (i as f64) / 10.0,
            "timestamp": timestamp.to_rfc3339(),
        }));

        storage.set(&key, value).await?;
    }

    // Calculate statistics
    info!("\nCalculating memory statistics...");
    let stats = compressor.calculate_stats(namespace).await?;

    info!("  Total items: {}", stats.item_count);
    info!("  Total size: {} bytes", stats.total_bytes);
    info!("  Average item size: {} bytes", stats.avg_item_size);

    if let Some(oldest) = stats.oldest_timestamp {
        info!("  Oldest item: {}", oldest.format("%Y-%m-%d"));
    }
    if let Some(newest) = stats.newest_timestamp {
        info!("  Newest item: {}", newest.format("%Y-%m-%d"));
    }

    // Remove old items
    info!("\nRemoving items older than 7 days...");
    let cutoff = chrono::Utc::now() - chrono::Duration::days(7);
    let deleted = compressor.remove_old_items(namespace, cutoff).await?;
    info!("  Deleted {} old items", deleted);

    // Remove least important items
    info!("\nRemoving items with importance < 0.5...");
    let deleted = compressor
        .remove_least_important(namespace, 0.5, 10)
        .await?;
    info!("  Deleted {} low-importance items", deleted);

    // Final statistics
    let final_stats = compressor.calculate_stats(namespace).await?;
    info!("\nFinal statistics:");
    info!("  Remaining items: {}", final_stats.item_count);
    info!(
        "  Space saved: {} bytes",
        stats.total_bytes - final_stats.total_bytes
    );

    // Compression with LLM (if available)
    #[cfg(feature = "rexis-llm-client")]
    {
        info!("\nTesting LLM-based conversation compression...");
        if let Ok(client) = Client::from_env() {
            // Create some conversation messages
            let conv_namespace = "test-conversation";
            for i in 0..5 {
                let key = format!("{}::msg::{}", conv_namespace, i);
                let timestamp = chrono::Utc::now() - chrono::Duration::minutes((5 - i) * 10);
                let value = MemoryValue::Json(serde_json::json!({
                    "role": if i % 2 == 0 { "user" } else { "assistant" },
                    "content": format!("Message content {}", i),
                    "timestamp": timestamp.to_rfc3339(),
                }));
                storage.set(&key, value).await?;
            }

            match compressor
                .compress_conversation_memory(conv_namespace, &client, 2)
                .await
            {
                Ok(compressed) => {
                    info!("  ✓ Compressed {} old messages into summary", compressed);
                }
                Err(e) => {
                    info!("  ⚠️  Could not compress: {}", e);
                }
            }
        } else {
            info!("  ⚠️  LLM client not available for compression");
        }
    }

    Ok(())
}
