# Rexis - Agentic AI Framework for Rust

[![Crates.io](https://img.shields.io/crates/v/rexis.svg)](https://crates.io/crates/rexis)
[![Documentation](https://docs.rs/rexis/badge.svg)](https://docs.rs/rexis)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

> **"Rule your agents, connect your intelligence"**

**Rexis** is a production-ready Agentic AI framework for Rust, featuring multi-provider LLM support, memory-first agents with vector search, and graph-based orchestration.

## 🌟 Features

- **🤖 Multi-Provider LLM Support** - Unified interface for OpenAI, Anthropic Claude, and Ollama
- **🛠️ Type-Safe Tool Calling** - Automatic JSON schema generation with `#[tool]` macro
- **🧠 Intelligent Agents** - LangChain-style agents with conversation memory and tool execution
- **📊 Graph Orchestration** - Complex multi-agent workflows with `rexis-graph`
- **💾 Flexible Storage** - Multiple memory backends (in-memory, database with experimental support)
- **🔍 RAG Pipeline** - Document processing, retrieval, and context-aware generation
- **📝 Structured Logging** - Production-ready observability with `tracing`
- **⚡ High Performance** - Async/await throughout, zero-copy where possible

## 📦 Crates

This repository is organized as a workspace containing:

| Crate | Description | Version |
|-------|-------------|---------|
| [`rexis`](https://crates.io/crates/rexis) | Umbrella crate - All-in-one Rexis framework | [![Crates.io](https://img.shields.io/crates/v/rexis.svg)](https://crates.io/crates/rexis) |
| [`rexis-llm`](https://crates.io/crates/rexis-llm) | Multi-provider LLM client with tool calling | [![Crates.io](https://img.shields.io/crates/v/rexis-llm.svg)](https://crates.io/crates/rexis-llm) |
| [`rexis-rag`](https://crates.io/crates/rexis-rag) | RAG framework with memory-first agents | [![Crates.io](https://img.shields.io/crates/v/rexis-rag.svg)](https://crates.io/crates/rexis-rag) |
| [`rexis-graph`](https://crates.io/crates/rexis-graph) | Graph-based agent orchestration | [![Crates.io](https://img.shields.io/crates/v/rexis-graph.svg)](https://crates.io/crates/rexis-graph) |
| [`rexis-macros`](https://crates.io/crates/rexis-macros) | Procedural macros for `#[tool]` | [![Crates.io](https://img.shields.io/crates/v/rexis-macros.svg)](https://crates.io/crates/rexis-macros) |

## 🚀 Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
rexis = { version = "0.1", features = ["full"] }
# Or use individual crates:
# rexis-llm = "0.1"
# rexis-rag = "0.1"
# rexis-graph = "0.1"
```

### Basic Example

```rust
use rexis::prelude::*;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

// Define a tool with automatic schema generation
#[derive(JsonSchema, Serialize, Deserialize)]
pub struct CalculatorParams {
    /// First number to add
    pub a: f64,
    /// Second number to add
    pub b: f64,
}

#[tool(description = "Add two numbers together")]
fn calculator(params: CalculatorParams) -> Result<f64, String> {
    Ok(params.a + params.b)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create LLM client
    let client = Client::from_env()?;

    // Build agent with tools
    let agent = AgentBuilder::new()
        .with_llm(client)
        .with_tools(vec![Box::new(calculator)])
        .stateful()  // Maintains conversation history
        .verbose(true)
        .build()?;

    // Run agent
    let response = agent.run("What is 156 + 23?").await?;
    println!("{}", response);

    Ok(())
}
```

### Environment Configuration

```bash
# Provider selection
export RSLLM_PROVIDER=ollama  # or openai, claude
export RSLLM_MODEL=llama3.2:3b

# Provider-specific
export RSLLM_OLLAMA_BASE_URL=http://localhost:11434/api/
export RSLLM_OPENAI_API_KEY=your-api-key
export RSLLM_OPENAI_MODEL=gpt-4

# Optional settings
export RSLLM_TEMPERATURE=0.7
export RSLLM_MAX_TOKENS=2000
```

## 📚 Documentation

### Rexis LLM - LLM Client

The `rexis-llm` crate provides a unified interface for multiple LLM providers:

```rust
use rexis_llm::{LLMClient, ChatMessage, ChatRole};

let client = LLMClient::from_env()?;

let messages = vec![
    ChatMessage::new(ChatRole::System, "You are a helpful assistant"),
    ChatMessage::new(ChatRole::User, "Hello!"),
];

let response = client.chat_completion(messages).await?;
```

**Supported Providers:**
- OpenAI (GPT-3.5, GPT-4, etc.)
- Anthropic Claude
- Ollama (local models)

### Tool Calling

Three approaches for tool creation:

**1. `#[tool]` Macro (Recommended)**

```rust
#[derive(JsonSchema, Serialize, Deserialize)]
pub struct WeatherParams {
    /// City name to get weather for
    pub city: String,
    /// Temperature unit (celsius or fahrenheit)
    #[schemars(regex(pattern = "^(celsius|fahrenheit)$"))]
    pub unit: String,
}

#[tool(description = "Get current weather for a city")]
fn get_weather(params: WeatherParams) -> Result<String, String> {
    Ok(format!("Weather in {}: 72°{}", params.city, params.unit))
}
```

**2. SchemaBasedTool Trait**

```rust
use rexis_llm::tools::{SchemaBasedTool, ToolResult};

pub struct DatabaseTool {
    connection: DatabaseConnection,
}

impl SchemaBasedTool for DatabaseTool {
    fn name(&self) -> &str { "query_database" }
    fn description(&self) -> &str { "Query the database" }
    fn parameters_schema(&self) -> serde_json::Value { /* ... */ }
    async fn call(&self, args: serde_json::Value) -> ToolResult {
        // Execute with state
    }
}
```

### Agent System

**Stateful Agents** (Chat applications):

```rust
let agent = AgentBuilder::new()
    .with_llm(client)
    .with_tools(tools)
    .stateful()  // Maintains conversation history
    .with_system_prompt("You are a helpful assistant")
    .with_max_iterations(10)
    .verbose(true)
    .build()?;

// Multiple interactions maintain context
agent.run("My name is Alice").await?;
agent.run("What's my name?").await?;  // Remembers "Alice"
```

**Stateless Agents** (API endpoints):

```rust
let agent = AgentBuilder::new()
    .with_llm(client)
    .with_tools(tools)
    .stateless()  // Each call is independent
    .build()?;

// Each call is independent
agent.run("What is 2+2?").await?;
agent.run("What is 3+3?").await?;
```

### Memory Backends

**In-Memory Storage** (Production-ready):

```rust
use rexis::rag::storage::{InMemoryStorage, InMemoryConfig, Memory, MemoryValue};

let config = InMemoryConfig {
    max_keys: Some(100_000),
    max_memory_bytes: Some(1_000_000_000), // 1GB
    enable_eviction: false,
};

let storage = InMemoryStorage::with_config(config);

// Store and retrieve
storage.set("user:name", MemoryValue::from("Alice")).await?;
let name = storage.get("user:name").await?;
```

**Database Storage** (⚠️ Experimental):

```rust
use rexis::rag::storage::{DatabaseStorage, DatabaseConfig};

let config = DatabaseConfig {
    connection_string: "sqlite:memory.db".to_string(),
    max_connections: 10,
    ..Default::default()
};

let storage = DatabaseStorage::with_config(config).await?;
```

> **Note:** DatabaseStorage currently uses in-memory fallback due to Toasty ORM being experimental. For production persistence, use `InMemoryStorage` or integrate `sqlx`/`diesel` directly.

### Graph Orchestration

Build complex multi-agent workflows:

```rust
use rexis::graph::{Graph, Node, ExecutionContext};
use rexis::rag::storage::InMemoryStorage;

let storage = Arc::new(InMemoryStorage::new());
let context = ExecutionContext::new("graph-id", node_id)
    .with_memory(storage);

// Build workflow graph
let mut graph = Graph::new("workflow");
graph.add_node(agent_node);
graph.add_node(processing_node);
graph.add_edge("agent", "processing");

// Execute with persistent memory
graph.execute(&mut state, &context).await?;
```

## 🎯 Examples

Run the examples to see RRAG in action:

```bash
# Tool calling guide (comprehensive demo)
cargo run -p rsllm --example tool_calling_guide --all-features

# OpenAI compatibility verification
cargo run -p rsllm --example openai_compatibility_test --all-features

# Agent demo (stateful and stateless modes)
cargo run --bin agent_demo

# Simple agent prototype
cargo run --bin simple_agent

# Storage demo
cargo run -p rrag --example storage_demo --features rsllm-client

# Memory integration with agents
cargo run --example agent_memory_demo --features rrag-integration,observability
```

### Running Examples with Ollama

1. Install and start Ollama:
```bash
ollama serve
```

2. Pull a model with tool support:
```bash
ollama pull llama3.2:3b
```

3. Run examples:
```bash
export RSLLM_PROVIDER=ollama
export RSLLM_MODEL=llama3.2:3b
cargo run --bin agent_demo
```

## 🏗️ Architecture

### Key Design Principles

1. **Type Safety** - Leverage Rust's type system for correctness
2. **Zero-Cost Abstractions** - Performance without runtime overhead
3. **Async/Await** - Non-blocking I/O throughout
4. **Modular Design** - Use only what you need
5. **Production Ready** - Structured logging, error handling, observability

### Tool Schema Generation

RRAG uses a vendored, modified version of `schemars` configured for 100% OpenAI compatibility:

- Draft 7 JSON Schema (not 2020-12)
- Inline subschemas (no `$ref`, no `$defs`)
- No `$schema` field
- Verified with OpenAI compatibility tests

### Agent Loop

```
User Input
  ↓
Agent.run()
  ↓
LLM Call (with tool schemas)
  ↓
Tool Calls?
  Yes → Execute Tools → Add Results → Loop (max 10 iterations)
  No → Final Answer
```

### Why Rexis?

**Rexis** combines the power of Rust's performance and safety with modern RAG capabilities:

- **Performance**: Zero-cost abstractions and async I/O for high throughput
- **Safety**: Rust's type system prevents common bugs at compile time
- **Flexibility**: Use individual crates or the full framework
- **Production Ready**: Built-in observability, error handling, and testing

### Memory Architecture

```
┌─────────────────────────────────────┐
│      Application Layer              │
│  (Agents, Graphs, Custom Logic)     │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│       Memory Trait                  │
│  (Unified Storage Interface)        │
└──────────────┬──────────────────────┘
               │
       ┌───────┴────────┐
       ▼                ▼
┌─────────────┐  ┌──────────────┐
│ InMemory    │  │  Database    │
│  Storage    │  │  Storage     │
│ (Production)│  │(Experimental)│
└─────────────┘  └──────────────┘
```

## 🔧 Development

### Prerequisites

- Rust 1.70 or higher
- (Optional) Ollama for local LLM testing

### Building

```bash
# Build entire workspace
cargo build

# Build specific crate
cargo build -p rsllm
cargo build -p rrag

# Build with all features
cargo build --all-features
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Test specific crate
cargo test -p rsllm --lib
cargo test -p rrag --all-features

# Run with logging
RUST_LOG=debug cargo test
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Run clippy
cargo clippy --all-features --workspace -- -D warnings

# Check without building
cargo check --workspace
```

### Logging Policy

**Always use `tracing` for logging:**

```rust
use tracing::{debug, info, warn, error};

tracing::info!(user_id = %user.id, "User logged in");
tracing::error!(error = ?e, "Failed to process request");
```

**Never use `println!`, `eprintln!`, or `dbg!()` except in:**
- Example binaries for user-facing output
- Test output

## 🤝 Contributing

Contributions are welcome! Please follow these guidelines:

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Commit** your changes (follow existing commit style)
4. **Push** to the branch (`git push origin feature/amazing-feature`)
5. **Open** a Pull Request

### Commit Guidelines

- Use clear, descriptive commit messages
- Follow conventional commits format (e.g., `feat:`, `fix:`, `docs:`)
- Keep commits focused and atomic

### Code Style

- Run `cargo fmt` before committing
- Ensure `cargo clippy` passes with no warnings
- Add tests for new functionality
- Update documentation as needed

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Inspired by [LangChain](https://www.langchain.com/)
- Uses [Tokio](https://tokio.rs/) for async runtime
- JSON Schema generation via modified [schemars](https://github.com/GREsau/schemars)

## 📞 Contact

- **Author**: Vasanth
- **Email**: vasanth@0xteam.io
- **Repository**: https://github.com/0xteamhq/rexis
- **Issues**: https://github.com/0xteamhq/rexis/issues

## 🗺️ Roadmap

- [ ] Semantic search and vector embeddings
- [ ] Advanced RAG strategies (HyDE, Self-RAG)
- [ ] More LLM providers (Cohere, Gemini)
- [ ] Stable database storage backend
- [ ] Distributed agent orchestration
- [ ] Web UI for agent monitoring
- [ ] Benchmark suite and performance optimizations

---

**Star ⭐ this repository if you find it useful!**
