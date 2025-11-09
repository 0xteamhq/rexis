//! # Agent Memory Integration Example
//!
//! Demonstrates how agents in RGraph workflows can use persistent memory
//! alongside GraphState for hybrid in-memory + persistent storage.
//!
//! ## What This Example Shows
//!
//! - **GraphState**: Fast, in-memory workflow data (temporary)
//! - **Memory**: Persistent agent memory (survives across executions)
//! - **Hybrid Approach**: Combining both for optimal performance
//!
//! ## Run This Example
//!
//! ```bash
//! cargo run --example agent_memory_demo --features rrag-integration,observability
//! ```

use rrag::storage::{InMemoryStorage, Memory};
use rrag_graph::core::{ExecutionContext, ExecutionResult, GraphBuilder, Node, NodeId};
use rrag_graph::state::{GraphState, StateValue};
use rrag_graph::RGraphResult;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::info;

/// A simple agent node that uses both GraphState and Memory
struct MemoryAwareAgent {
    id: NodeId,
    name: String,
}

impl MemoryAwareAgent {
    fn new(id: impl Into<NodeId>, name: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            id: id.into(),
            name: name.into(),
        })
    }
}

#[async_trait]
impl Node for MemoryAwareAgent {
    async fn execute(
        &self,
        state: &mut GraphState,
        context: &ExecutionContext,
    ) -> RGraphResult<ExecutionResult> {
        info!("Agent '{}' executing", self.name);

        // 1. Read input from GraphState (fast, in-memory)
        let user_input = state
            .get("user_input")
            .unwrap_or_else(|_| StateValue::String("Hello!".to_string()));

        let input_text = match user_input {
            StateValue::String(s) => s,
            _ => "Hello!".to_string(),
        };

        info!("User input from GraphState: {}", input_text);

        // 2. Access persistent memory if available
        if let Some(memory) = context.memory() {
            info!("Persistent memory is available!");

            // Check conversation count
            let count_key = format!("agent::{}::conversation_count", self.id.as_str());
            let current_count = match memory.get(&count_key).await {
                Ok(Some(value)) => value.as_integer().unwrap_or(0),
                _ => 0,
            };

            info!("This is conversation #{}", current_count + 1);

            // Update count
            memory
                .set(
                    &count_key,
                    rrag::storage::MemoryValue::from(current_count + 1),
                )
                .await
                .ok();

            // Store user preference (example of semantic memory)
            let pref_key = format!("agent::{}::user_preferences", self.id.as_str());
            memory
                .set(&pref_key, rrag::storage::MemoryValue::from("friendly_tone"))
                .await
                .ok();

            // Store episodic memory (summary of this interaction)
            let episode_key = format!(
                "agent::{}::episode::{}",
                self.id.as_str(),
                uuid::Uuid::new_v4()
            );
            let episode_data = serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "summary": format!("User said: {}", input_text),
                "importance": 0.7
            });
            memory
                .set(&episode_key, rrag::storage::MemoryValue::Json(episode_data))
                .await
                .ok();

            info!("Saved interaction to persistent memory");
        } else {
            info!("No persistent memory available (running in stateless mode)");
        }

        // 3. Generate response (would call LLM in real implementation)
        let response = format!(
            "Hello! I'm {}, and I processed your message: '{}'",
            self.name, input_text
        );

        // 4. Store response in GraphState (for next nodes in workflow)
        state.set("agent_response", response.clone());
        state.set("output", response);

        info!("Stored response in GraphState");

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
    // Initialize tracing (simple console output)
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::INFO)
            .finish(),
    )
    .expect("setting default subscriber failed");

    info!("=== RGraph Agent Memory Demo ===\n");

    // Create persistent memory backend
    let memory_storage = Arc::new(InMemoryStorage::new());

    // Build workflow graph with memory-aware agent
    info!("Building workflow graph...");
    let agent = MemoryAwareAgent::new("memory_agent", "MemoryBot");

    let graph = GraphBuilder::new("agent_memory_demo")
        .description("Demonstrates hybrid GraphState + Memory approach")
        .add_node("agent", agent)
        .await?
        .build()?;

    info!("Graph built successfully\n");

    // Get the agent node for direct execution
    let agent_node = graph.get_node(&NodeId::new("agent")).unwrap();

    // === Execution 1: With Persistent Memory ===
    info!("=== Execution #1: WITH PERSISTENT MEMORY ===");
    {
        let mut state = GraphState::new();
        state.set("user_input", "Tell me about Rust!");

        let context = ExecutionContext::new(graph.id().to_string(), NodeId::new("agent"))
            .with_memory(memory_storage.clone());

        info!("Executing agent with persistent memory...");
        let result = agent_node.execute(&mut state, &context).await?;

        info!("Execution result: {:?}", result);
        if let Ok(StateValue::String(response)) = state.get("output") {
            info!("Agent response: {}\n", response);
        }
    }

    // === Execution 2: Another run with same memory (shows persistence) ===
    info!("=== Execution #2: SECOND RUN (Same Memory) ===");
    {
        let mut state = GraphState::new();
        state.set("user_input", "What's your favorite programming language?");

        let context = ExecutionContext::new(graph.id().to_string(), NodeId::new("agent"))
            .with_memory(memory_storage.clone());

        info!("Executing agent again with same memory backend...");
        let result = agent_node.execute(&mut state, &context).await?;

        info!("Execution result: {:?}", result);
        if let Ok(StateValue::String(response)) = state.get("output") {
            info!("Agent response: {}\n", response);
        }

        // Show what's in persistent memory
        info!("\n=== Persistent Memory Contents ===");
        let count_key = "agent::memory_agent::conversation_count";
        if let Ok(Some(count)) = memory_storage.get(count_key).await {
            info!("Total conversations: {}", count.as_integer().unwrap_or(0));
        }

        let pref_key = "agent::memory_agent::user_preferences";
        if let Ok(Some(pref)) = memory_storage.get(pref_key).await {
            info!("User preferences: {}", pref.as_string().unwrap_or("none"));
        }

        info!("Total memory keys: {}", memory_storage.count(None).await?);
    }

    info!("\n=== Demo Complete ===");
    info!("Key Takeaways:");
    info!("1. GraphState: Fast, temporary workflow data (cleared each run)");
    info!("2. Memory: Persistent across executions (conversation count increased)");
    info!("3. Hybrid: Use both for optimal agent performance");

    Ok(())
}
