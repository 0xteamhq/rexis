//! # Rexis - Agentic AI Framework for Rust
//!
//! **Rexis** is a comprehensive AI framework that brings together:
//!
//! - **Rexis LLM**: Multi-provider LLM client (OpenAI, Claude, Ollama)
//! - **Rexis RAG**: Memory-first agents with vector search and retrieval
//! - **Rexis Graph**: Graph-based agent orchestration
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use rexis::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create LLM client
//!     let client = rexis_llm::Client::from_env()?;
//!
//!     // Build an agent with memory
//!     let agent = AgentBuilder::new()
//!         .with_llm(client)
//!         .stateful()
//!         .build()?;
//!
//!     // Run the agent
//!     let response = agent.run("What is Rust?").await?;
//!     println!("{}", response);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! ### Memory-First Agents
//!
//! Rexis agents come with built-in persistent memory:
//!
//! - **Working Memory**: Temporary scratchpad for current task
//! - **Semantic Memory**: Long-term knowledge with vector search
//! - **Episodic Memory**: Summarized conversation history with LLM
//! - **Shared Memory**: Cross-agent knowledge base
//!
//! ### Vector Search
//!
//! Enable semantic search with the `vector-search` feature:
//!
//! ```toml
//! [dependencies]
//! rexis = { version = "0.1", features = ["full"] }
//! ```
//!
//! ### Graph Orchestration
//!
//! Build complex multi-agent workflows with graph-based orchestration.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │             Rexis                       │
//! │  (Umbrella Crate - Rule Your Agents)   │
//! └──────────────┬──────────────────────────┘
//!                │
//!      ┌─────────┼─────────┐
//!      │         │         │
//!  ┌───▼──┐  ┌──▼───┐  ┌──▼────┐
//!  │ LLM  │  │ RAG  │  │ Graph │
//!  └──────┘  └──────┘  └───────┘
//! ```
//!
//! ## Tagline
//!
//! *"Rule your agents, connect your intelligence"*

#![doc(html_root_url = "https://docs.rs/rexis/0.1.0")]
#![warn(missing_docs)]

// Re-export sub-crates
#[cfg(feature = "llm")]
pub use rexis_llm as llm;

#[cfg(feature = "rag")]
pub use rexis_rag as rag;

#[cfg(feature = "graph")]
pub use rexis_graph as graph;

/// Commonly used types and traits
pub mod prelude {
    #[cfg(feature = "llm")]
    pub use crate::llm::{ChatMessage, ChatResponse, Client, MessageRole};

    #[cfg(feature = "rag")]
    pub use crate::rag::{
        agent::{Agent, AgentBuilder},
        error::{RragError, RragResult},
    };

    #[cfg(all(feature = "rag", feature = "llm"))]
    pub use crate::rag::agent::memory::{
        AgentMemoryManager, ConversationMemoryStore, Episode, EpisodicMemory, Fact, MemoryConfig,
        SemanticMemory, SharedKnowledgeBase, WorkingMemory,
    };

    #[cfg(feature = "graph")]
    pub use crate::graph::{
        core::{ExecutionContext, NodeId},
        nodes::{AgentNode, ConditionNode, ToolNode, TransformNode},
        state::GraphState,
    };
}
