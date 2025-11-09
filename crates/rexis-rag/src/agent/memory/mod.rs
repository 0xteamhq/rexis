//! # Agent Memory System
//!
//! Provides persistent, hierarchical memory management for conversational agents.
//!
//! ## Memory Scopes
//!
//! - **Global**: `global::key` - Shared across all agents
//! - **Agent**: `agent::<agent_id>::key` - Agent-specific persistent memory
//! - **Session**: `session::<session_id>::key` - Session-scoped temporary memory
//!
//! ## Memory Types
//!
//! - **Conversation**: Chat message history with persistence
//! - **Working**: Temporary scratchpad for agent reasoning
//! - **Semantic**: Facts and knowledge storage
//! - **Episodic**: Summarized conversation history
//! - **Shared**: Cross-agent knowledge base
//!
//! ## Example
//!
//! ```rust,no_run
//! use rrag::agent::memory::{MemoryConfig, AgentMemoryManager};
//! use rrag::storage::InMemoryStorage;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let storage = Arc::new(InMemoryStorage::new());
//! let config = MemoryConfig::new(storage, "my-agent")
//!     .with_persistence(true)
//!     .with_working_memory(true)
//!     .with_semantic_memory(true);
//!
//! let mut manager = AgentMemoryManager::new(config);
//!
//! // Use working memory (scratchpad)
//! manager.working().set("temp_result", 42i64).await?;
//!
//! // Use semantic memory (facts)
//! use rrag::agent::memory::Fact;
//! use rrag::storage::MemoryValue;
//! let fact = Fact::new("user:alice", "prefers", MemoryValue::from("dark_mode"));
//! manager.semantic().store_fact(fact).await?;
//!
//! // Use episodic memory (summaries)
//! use rrag::agent::memory::Episode;
//! let episode = Episode::new("User asked about Rust programming");
//! manager.episodic().store_episode(episode).await?;
//!
//! // Use shared knowledge base (cross-agent)
//! manager.shared().store("api_endpoint", MemoryValue::from("https://api.example.com")).await?;
//! # Ok(())
//! # }
//! ```

mod compression;
mod config;
mod conversation;
mod episodic;
mod manager;
mod semantic;
mod shared;
mod working;

#[cfg(feature = "vector-search")]
pub mod vector;

pub use compression::{CompressionConfig, CompressionStrategy, MemoryCompressor, MemoryStats};
pub use config::MemoryConfig;
pub use conversation::{generate_session_id, ConversationMemoryStore};
pub use episodic::{Episode, EpisodicMemory};
pub use manager::AgentMemoryManager;
pub use semantic::{Fact, SemanticMemory};
pub use shared::{KnowledgeEntry, SharedKnowledgeBase};
pub use working::WorkingMemory;

#[cfg(feature = "vector-search")]
pub use vector::{Embedding, EmbeddingProvider, HashEmbeddingProvider, SearchResult};
