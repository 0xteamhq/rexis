# Rexis

> **Rule your agents, connect your intelligence**

**Rexis** is a comprehensive Agentic AI framework for Rust that combines:

- 🤖 **Multi-provider LLM client** (OpenAI, Claude, Ollama)
- 🧠 **Memory-first AI agents** with persistent knowledge
- 🔍 **Vector search** and semantic retrieval
- 📊 **Graph-based orchestration** for multi-agent workflows

## Quick Start

```rust
use rexis::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create LLM client
    let client = rexis_llm::Client::from_env()?;

    // Build an agent with memory
    let agent = AgentBuilder::new()
        .with_llm(client)
        .stateful() // Persistent conversation memory
        .build()?;

    // Run the agent
    let response = agent.run("What is Rust?").await?;
    println!("{}", response);

    Ok(())
}
```

## Features

| Feature | Description |
|---------|-------------|
| `llm` | Multi-provider LLM client with streaming and tool calling |
| `rag` | RAG framework with agents and memory systems |
| `graph` | Graph-based agent orchestration |
| `full` | All features enabled (recommended) |

## Installation

```toml
[dependencies]
rexis = { version = "0.1", features = ["full"] }
```

## Architecture

Rexis is built from three core crates:

### 1. Rexis LLM (`rexis-llm`)

Multi-provider LLM client with:
- OpenAI, Claude, Ollama support
- Streaming responses
- Tool calling with JSON schema
- Automatic retry and error handling

### 2. Rexis RAG (`rexis-rag`)

Memory-first agents with:
- **Working Memory**: Temporary task context
- **Semantic Memory**: Knowledge graph with vector search
- **Episodic Memory**: LLM-summarized conversation history
- **Shared Memory**: Cross-agent knowledge base

### 3. Rexis Graph (`rexis-graph`)

Graph-based orchestration with:
- Hybrid state management (fast + persistent)
- Agent node integration
- Conditional branching
- Parallel execution

## Advanced Features

### Vector Search

Enable semantic search in semantic memory:

```rust
use rexis::rag::agent::memory::{SemanticMemory, HashEmbeddingProvider};

let semantic = SemanticMemory::new(storage, "agent-id".to_string());
let provider = HashEmbeddingProvider::new(128);

// Find similar facts
let results = semantic
    .find_similar("Rust programming", &provider, 5, 0.7)
    .await?;
```

### Memory Compression

Automatically compress old memories:

```rust
use rexis::rag::agent::memory::{MemoryCompressor, CompressionConfig};

let compressor = MemoryCompressor::new(
    storage,
    CompressionConfig::default()
);

// Compress old conversations
compressor
    .compress_conversation_memory(namespace, &llm_client, 10)
    .await?;
```

### Graph Workflows

Build multi-agent workflows:

```rust
use rexis::graph::prelude::*;

let workflow = GraphWorkflow::new("research-workflow")
    .add_node(research_agent)
    .add_node(summarizer_agent)
    .add_edge("research", "summarizer")
    .build()?;

workflow.execute(initial_state).await?;
```

## Examples

See [`examples/`](../../examples/) for:
- Basic agent usage
- Advanced memory features
- Multi-agent orchestration
- Vector search integration

## Documentation

- [API Docs](https://docs.rs/rexis)
- [Guide](https://github.com/0xteamhq/rexis#readme)
- [Examples](https://github.com/0xteamhq/rexis/tree/main/examples)

## License

MIT License - see [LICENSE](../../LICENSE) for details.

## Contributing

Contributions welcome! See [CONTRIBUTING.md](../../CONTRIBUTING.md).
