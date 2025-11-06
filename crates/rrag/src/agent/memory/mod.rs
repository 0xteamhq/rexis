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
//! - **Semantic**: Facts and knowledge (future)
//! - **Episodic**: Summarized history (future)
//! - **Working**: Temporary scratchpad (future)
//! - **Shared**: Cross-agent knowledge base (future)
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
//!     .with_persistence(true);
//!
//! let manager = AgentMemoryManager::new(config);
//!
//! // Store agent-scoped data
//! manager.set_agent_memory("preferences", "value").await?;
//!
//! // Store session-scoped data
//! manager.set_session_memory("temp_data", 42i64).await?;
//! # Ok(())
//! # }
//! ```

mod config;
mod conversation;
mod manager;

pub use config::MemoryConfig;
pub use conversation::{generate_session_id, ConversationMemoryStore};
pub use manager::AgentMemoryManager;
