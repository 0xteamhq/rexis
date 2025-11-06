//! Core Agent implementation

use super::{AgentConfig, ConversationMemory, ConversationMode, ToolExecutor};
use super::memory::AgentMemoryManager;
use crate::error::RragResult;
use rsllm::{ChatMessage, ChatResponse, Client};
use tracing::{debug, error, info};

/// Agent that can use tools and maintain conversation
pub struct Agent {
    /// LLM client
    llm_client: Client,

    /// Tool executor
    tool_executor: ToolExecutor,

    /// Legacy conversation memory (for backward compatibility)
    legacy_memory: ConversationMemory,

    /// New persistent memory manager (optional)
    memory_manager: Option<AgentMemoryManager>,

    /// Agent configuration
    config: AgentConfig,
}

impl Agent {
    /// Create a new agent (legacy constructor - uses in-memory ConversationMemory)
    pub fn new(
        llm_client: Client,
        tool_executor: ToolExecutor,
        config: AgentConfig,
    ) -> RragResult<Self> {
        let mut legacy_memory = ConversationMemory::with_max_length(config.max_conversation_length);

        // Add system prompt
        legacy_memory.add_message(ChatMessage::system(config.system_prompt.clone()));

        Ok(Self {
            llm_client,
            tool_executor,
            legacy_memory,
            memory_manager: None,
            config,
        })
    }

    /// Create a new agent with persistent memory
    pub fn new_with_memory(
        llm_client: Client,
        tool_executor: ToolExecutor,
        memory_manager: AgentMemoryManager,
        config: AgentConfig,
    ) -> RragResult<Self> {
        let mut legacy_memory = ConversationMemory::with_max_length(config.max_conversation_length);

        // Add system prompt to legacy memory (fallback)
        legacy_memory.add_message(ChatMessage::system(config.system_prompt.clone()));

        Ok(Self {
            llm_client,
            tool_executor,
            legacy_memory,
            memory_manager: Some(memory_manager),
            config,
        })
    }

    /// Run the agent with a user query
    ///
    /// In stateless mode: Creates fresh conversation for each call
    /// In stateful mode: Continues previous conversation
    pub async fn run(&mut self, user_input: impl Into<String>) -> RragResult<String> {
        let input = user_input.into();

        info!(user_input = %input, "Agent received user input");

        if self.config.verbose {
            debug!(input = %input, "Processing user query");
        }

        // Prepare conversation based on mode and memory system
        let mut conversation = match self.config.conversation_mode {
            ConversationMode::Stateless => {
                // Fresh conversation: system prompt + user message
                vec![
                    ChatMessage::system(self.config.system_prompt.clone()),
                    ChatMessage::user(input.clone()),
                ]
            }
            ConversationMode::Stateful => {
                // Use new memory system if available, otherwise legacy
                if let Some(ref memory_manager) = self.memory_manager {
                    // Add user message to persistent memory
                    memory_manager
                        .add_conversation_message(ChatMessage::user(input.clone()))
                        .await?;

                    // Get full conversation history
                    memory_manager.get_conversation_messages().await?
                } else {
                    // Legacy in-memory conversation
                    self.legacy_memory.add_message(ChatMessage::user(input.clone()));
                    self.legacy_memory.to_messages()
                }
            }
        };

        // Agent loop: iterate until we get a final answer
        for iteration in 1..=self.config.max_iterations {
            debug!(iteration, max_iterations = self.config.max_iterations, "Agent iteration");

            // Call LLM with tools
            let response = self.llm_step(&conversation).await?;

            // Check for tool calls
            if let Some(tool_calls) = &response.tool_calls {
                if !tool_calls.is_empty() {
                    info!(
                        tool_count = tool_calls.len(),
                        tools = ?tool_calls.iter().map(|t| &t.function.name).collect::<Vec<_>>(),
                        "Agent requesting tool calls"
                    );

                    // Add assistant message with tool calls to conversation
                    let mut assistant_msg = ChatMessage::assistant(response.content.clone());
                    assistant_msg.tool_calls = Some(tool_calls.clone());
                    conversation.push(assistant_msg);

                    // Execute all tool calls
                    let tool_results = self.tool_executor.execute_tool_calls(tool_calls);

                    // Add tool results to conversation
                    for result in tool_results {
                        if let crate::rsllm::MessageContent::Text(ref content) = result.content {
                            debug!(tool_result = %content, "Tool execution completed");
                        }
                        conversation.push(result);
                    }

                    // Continue loop to let LLM process results
                    continue;
                }
            }

            // No tool calls - this is the final answer
            info!(
                response = %response.content,
                iterations = iteration,
                "Agent generated final answer"
            );

            // Update memory in stateful mode
            if self.config.conversation_mode == ConversationMode::Stateful {
                if let Some(ref memory_manager) = self.memory_manager {
                    // Persist to new memory system
                    memory_manager
                        .add_conversation_message(ChatMessage::assistant(response.content.clone()))
                        .await?;
                } else {
                    // Legacy in-memory
                    self.legacy_memory.add_message(ChatMessage::assistant(response.content.clone()));
                }
            }

            return Ok(response.content);
        }

        // Exceeded max iterations
        error!(
            max_iterations = self.config.max_iterations,
            "Agent exceeded maximum iterations without reaching final answer"
        );

        Err(crate::error::RragError::Agent {
            agent_id: "default".to_string(),
            message: format!("Agent exceeded maximum iterations ({})", self.config.max_iterations),
            source: None,
        })
    }

    /// Single LLM call with tools
    async fn llm_step(&self, conversation: &[ChatMessage]) -> RragResult<ChatResponse> {
        // Get tool definitions
        let tools = self.tool_executor.registry().tool_definitions();

        debug!(
            tool_count = tools.len(),
            message_count = conversation.len(),
            "Calling LLM with tools"
        );

        // Call LLM
        let response = self
            .llm_client
            .chat_completion_with_tools(conversation.to_vec(), tools)
            .await?;

        debug!(
            content_length = response.content.len(),
            has_tool_calls = response.tool_calls.is_some(),
            tool_call_count = response.tool_calls.as_ref().map(|t| t.len()).unwrap_or(0),
            "LLM response received"
        );

        Ok(response)
    }

    /// Reset conversation (clears history, keeps system prompt)
    pub async fn reset(&mut self) -> RragResult<()> {
        if let Some(ref memory_manager) = self.memory_manager {
            memory_manager.clear_conversation().await?;
        } else {
            self.legacy_memory.clear();
        }
        Ok(())
    }

    /// Get conversation history (legacy - uses in-memory only)
    pub fn get_conversation(&self) -> &[ChatMessage] {
        self.legacy_memory.get_messages()
    }

    /// Get conversation history from persistent memory (async)
    pub async fn get_conversation_async(&self) -> RragResult<Vec<ChatMessage>> {
        if let Some(ref memory_manager) = self.memory_manager {
            memory_manager.get_conversation_messages().await
        } else {
            Ok(self.legacy_memory.to_messages())
        }
    }

    /// Get agent configuration
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut AgentConfig {
        &mut self.config
    }

    /// Get access to the memory manager (if using persistent memory)
    pub fn memory(&self) -> Option<&AgentMemoryManager> {
        self.memory_manager.as_ref()
    }

    /// Get mutable access to the memory manager (if using persistent memory)
    pub fn memory_mut(&mut self) -> Option<&mut AgentMemoryManager> {
        self.memory_manager.as_mut()
    }
}
