//! # RRAG Agent Module
//!
//! LangChain-style agent framework for Rust with tool calling support.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use rrag::agent::{Agent, AgentBuilder};
//!
//! let agent = AgentBuilder::new()
//!     .with_llm(client)
//!     .with_tools(tools)
//!     .build()?;
//!
//! let response = agent.run("What's 2 + 2?").await?;
//! ```

mod agent;
mod builder;
mod config;
mod executor;
mod legacy_memory;
pub mod memory; // New memory system

pub use agent::Agent;
pub use builder::AgentBuilder;
pub use config::{AgentConfig, ConversationMode};
pub use executor::ToolExecutor;
pub use legacy_memory::ConversationMemory; // Keep for backward compatibility
