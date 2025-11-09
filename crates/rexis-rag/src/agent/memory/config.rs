//! Memory configuration for agents

use crate::storage::Memory;
use std::sync::Arc;

/// Configuration for agent memory system
#[derive(Clone)]
pub struct MemoryConfig {
    /// Storage backend (InMemoryStorage, DatabaseStorage, etc.)
    pub backend: Arc<dyn Memory>,

    /// Unique agent identifier (for agent-scoped memory)
    pub agent_id: String,

    /// Optional session identifier (for session-scoped memory)
    pub session_id: Option<String>,

    /// Whether to persist conversation history to storage
    pub persist_conversations: bool,

    /// Enable semantic memory (facts/knowledge storage)
    pub enable_semantic: bool,

    /// Enable episodic memory (summarized history)
    pub enable_episodic: bool,

    /// Enable working memory (temporary scratchpad)
    pub enable_working: bool,

    /// Maximum conversation length before pruning
    pub max_conversation_length: usize,

    /// Auto-generate session IDs if not provided
    pub auto_generate_session_id: bool,
}

impl MemoryConfig {
    /// Create a new memory configuration
    pub fn new(backend: Arc<dyn Memory>, agent_id: impl Into<String>) -> Self {
        Self {
            backend,
            agent_id: agent_id.into(),
            session_id: None,
            persist_conversations: false,
            enable_semantic: false,
            enable_episodic: false,
            enable_working: false,
            max_conversation_length: 50,
            auto_generate_session_id: true,
        }
    }

    /// Set session ID
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Enable conversation persistence
    pub fn with_persistence(mut self, persist: bool) -> Self {
        self.persist_conversations = persist;
        self
    }

    /// Enable semantic memory
    pub fn with_semantic_memory(mut self, enable: bool) -> Self {
        self.enable_semantic = enable;
        self
    }

    /// Enable episodic memory
    pub fn with_episodic_memory(mut self, enable: bool) -> Self {
        self.enable_episodic = enable;
        self
    }

    /// Enable working memory
    pub fn with_working_memory(mut self, enable: bool) -> Self {
        self.enable_working = enable;
        self
    }

    /// Set max conversation length
    pub fn with_max_conversation_length(mut self, length: usize) -> Self {
        self.max_conversation_length = length;
        self
    }

    /// Enable/disable auto session ID generation
    pub fn with_auto_session_id(mut self, auto: bool) -> Self {
        self.auto_generate_session_id = auto;
        self
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        use crate::storage::InMemoryStorage;

        Self {
            backend: Arc::new(InMemoryStorage::new()),
            agent_id: "default".to_string(),
            session_id: None,
            persist_conversations: false,
            enable_semantic: false,
            enable_episodic: false,
            enable_working: false,
            max_conversation_length: 50,
            auto_generate_session_id: true,
        }
    }
}
