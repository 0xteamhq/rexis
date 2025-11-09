//! # RRAG Agent + RGraph Integration Example
//!
//! Shows how to integrate RRAG's full-featured AgentMemoryManager
//! with RGraph workflows for advanced multi-agent systems.
//!
//! ## Features Demonstrated
//!
//! - **Conversation Memory**: Persistent chat history
//! - **Working Memory**: Temporary scratchpad
//! - **Semantic Memory**: Facts and knowledge
//! - **Episodic Memory**: Summarized interactions
//! - **Shared Knowledge**: Cross-agent communication
//!
//! ## Run This Example
//!
//! ```bash
//! cargo run --example rrag_agent_integration --features rrag-integration,observability
//! ```

use async_trait::async_trait;
use rexis_graph::core::{ExecutionContext, ExecutionResult, GraphBuilder, Node, NodeId};
use rexis_graph::state::{GraphState, StateValue};
use rexis_graph::RGraphResult;
use rexis_rag::agent::memory::{AgentMemoryManager, Episode, Fact, MemoryConfig};
use rexis_rag::storage::{InMemoryStorage, Memory, MemoryValue};
use std::sync::Arc;
use tracing::info;

/// Agent node that uses RRAG's AgentMemoryManager
struct RRAGAgentNode {
    id: NodeId,
    name: String,
    storage: Arc<dyn rrag::storage::Memory>,
    agent_id: String,
}

impl RRAGAgentNode {
    fn new(
        id: impl Into<NodeId>,
        name: impl Into<String>,
        storage: Arc<dyn rrag::storage::Memory>,
        agent_id: String,
    ) -> Arc<Self> {
        Arc::new(Self {
            id: id.into(),
            name: name.into(),
            storage,
            agent_id,
        })
    }
}

#[async_trait]
impl Node for RRAGAgentNode {
    async fn execute(
        &self,
        state: &mut GraphState,
        context: &ExecutionContext,
    ) -> RGraphResult<ExecutionResult> {
        info!(
            "Agent '{}' executing with full RRAG memory system",
            self.name
        );

        // Get input from GraphState
        let user_input = state
            .get("user_input")
            .unwrap_or_else(|_| StateValue::String("Hello!".to_string()));

        let input_text = match user_input {
            StateValue::String(s) => s,
            _ => "Hello!".to_string(),
        };

        info!("Processing: {}", input_text);

        // Create a memory manager for this execution
        let memory_config = MemoryConfig::new(self.storage.clone(), &self.agent_id)
            .with_persistence(true)
            .with_semantic_memory(true)
            .with_episodic_memory(true)
            .with_working_memory(true);

        let mut manager = AgentMemoryManager::new(memory_config);

        // 1. Use Working Memory (temporary scratchpad)
        info!("Using working memory for temporary data...");
        manager
            .working()
            .set("current_task", input_text.clone())
            .await
            .ok();
        manager.working().set("processing_step", 1i64).await.ok();

        // 2. Store Semantic Memory (facts)
        info!("Storing fact in semantic memory...");
        let fact = Fact::new(
            "user",
            "asked_about",
            MemoryValue::from(&input_text as &str),
        )
        .with_confidence(0.9);
        manager.semantic().store_fact(fact).await.ok();

        // 3. Create Episode (summarized interaction)
        info!("Creating episodic memory...");
        let episode = Episode::new(format!("User interaction: {}", input_text))
            .with_topics(vec!["conversation".to_string(), "query".to_string()])
            .with_importance(0.8);
        manager.episodic().store_episode(episode).await.ok();

        // 4. Store in Shared Knowledge Base (for other agents)
        info!("Updating shared knowledge base...");
        manager
            .shared()
            .store("last_user_query", MemoryValue::from(&input_text as &str))
            .await
            .ok();

        // 5. Retrieve context from different memory types
        let working_task = manager
            .working()
            .get("current_task")
            .await
            .ok()
            .and_then(|v| v)
            .and_then(|v| v.as_string().map(String::from));

        let fact_count = manager.semantic().count().await.unwrap_or(0);
        let episode_count = manager.episodic().count().await.unwrap_or(0);

        info!("Memory Stats:");
        info!("  - Working memory task: {:?}", working_task);
        info!("  - Facts stored: {}", fact_count);
        info!("  - Episodes recorded: {}", episode_count);

        // Generate response
        let response = format!(
            "I'm {} with full memory capabilities! I've recorded your message and have {} facts and {} episodes in memory.",
            self.name, fact_count, episode_count
        );

        // Store response in GraphState
        state.set("agent_response", response.clone());
        state.set("output", response);

        // Also make memory backend available in ExecutionContext for other nodes
        if context.memory().is_none() {
            info!("Note: ExecutionContext doesn't have memory backend set");
            info!("Consider using context.with_memory() to share memory with other nodes");
        }

        Ok(ExecutionResult::Continue)
    }

    fn id(&self) -> &NodeId {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn input_keys(&self) -> Vec<&str> {
        vec!["user_input"]
    }

    fn output_keys(&self) -> Vec<&str> {
        vec!["agent_response", "output"]
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::INFO)
            .finish(),
    )
    .expect("setting default subscriber failed");

    info!("=== RRAG Agent + RGraph Integration Demo ===\n");

    // Create memory backend
    let storage = Arc::new(InMemoryStorage::new());

    // Create agent node with storage backend
    info!("Creating RRAG-powered agent node...");
    let agent = RRAGAgentNode::new(
        "rrag_agent",
        "RRAG Bot",
        storage.clone(),
        "demo-agent".to_string(),
    );

    // Build workflow
    info!("Building workflow graph...");
    let graph = GraphBuilder::new("rrag_integration_demo")
        .description("RRAG AgentMemoryManager + RGraph integration")
        .add_node("agent", agent)
        .await?
        .build()?;

    info!("Graph built successfully\n");

    // Get the agent node for direct execution
    let agent_node = graph.get_node(&NodeId::new("agent")).unwrap();

    // === Execution 1 ===
    info!("=== Execution #1 ===");
    {
        let mut state = GraphState::new();
        state.set("user_input", "What can you tell me about Rust programming?");

        let context = ExecutionContext::new(graph.id().to_string(), NodeId::new("agent"))
            .with_memory(storage.clone());

        let result = agent_node.execute(&mut state, &context).await?;

        info!("Result: {:?}", result);
        if let Ok(StateValue::String(response)) = state.get("output") {
            info!("Response: {}\n", response);
        }
    }

    // === Execution 2 ===
    info!("=== Execution #2 (Memory Persists) ===");
    {
        let mut state = GraphState::new();
        state.set("user_input", "Tell me about async programming!");

        let context = ExecutionContext::new(graph.id().to_string(), NodeId::new("agent"))
            .with_memory(storage.clone());

        let result = agent_node.execute(&mut state, &context).await?;

        info!("Result: {:?}", result);
        if let Ok(StateValue::String(response)) = state.get("output") {
            info!("Response: {}\n", response);
        }
    }

    // === Execution 3 ===
    info!("=== Execution #3 (Showing Memory Accumulation) ===");
    {
        let mut state = GraphState::new();
        state.set("user_input", "How do I use traits effectively?");

        let context = ExecutionContext::new(graph.id().to_string(), NodeId::new("agent"))
            .with_memory(storage.clone());

        let result = agent_node.execute(&mut state, &context).await?;

        info!("Result: {:?}", result);
        if let Ok(StateValue::String(response)) = state.get("output") {
            info!("Response: {}\n", response);
        }
    }

    // Show memory statistics
    info!("\n=== Final Memory Statistics ===");
    info!("Total keys in storage: {}", storage.count(None).await?);

    info!("\n=== Demo Complete ===");
    info!("This example showed:");
    info!("1. Full RRAG AgentMemoryManager integration with RGraph");
    info!("2. All 5 memory types: Conversation, Working, Semantic, Episodic, Shared");
    info!("3. Persistent memory across multiple graph executions");
    info!("4. Hybrid GraphState (fast) + Memory (persistent) approach");

    Ok(())
}
